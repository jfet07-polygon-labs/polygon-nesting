//! The deterministic contact corpus and its **independent** score.
//!
//! Gate 0's numeric-soundness cell asks four questions that Φ cannot be allowed
//! to answer about itself, so everything in this file is a second
//! implementation on purpose:
//!
//! * it measures **whole transformed rings**, never the convex cells Φ uses, so
//!   a decomposition bug cannot hide;
//! * it detects overlap by ray-cast containment and segment crossing, not by
//!   SAT, so a sign error in the hot path cannot cancel;
//! * it measures penetration as the deepest interior vertex's distance to the
//!   other boundary, which is a different quantity from the minimum translation
//!   vector and is therefore not a copy of the thing it audits.
//!
//! It is **diagnostic only**. Nothing here is the optimizer's objective, no
//! acceptance path calls it, and it is compiled into the same feature the
//! prototype is - it has no route into a shipped build at all.

use crate::search::general_fast::{GeneralFastPiece, GeneralFastSettings};
use crate::validation::round_envelope::{
    boundary_admissible, certifies, critical_boundary_radius_micron, critical_two_r_micron,
    pair_admissible, GridSet,
};

use super::descent::counter_hash;
use super::energy::{fold, rebuild_all, rebuild_piece_rows};
use super::diagnostics::WorkVector;
use super::state::{
    build_geometry, pair_count, transform_piece, Contract, EdgeRow, Geometry, IcsState, PairRow,
    PieceSource, Pose,
};

/// The independent measurement of one layout.
#[derive(Clone, Copy, Debug, Default)]
pub struct IndependentScore {
    /// `sum over pairs of [c_pair - d]_+  +  sum over piece-edges of deficit`.
    pub total_violation_mm: f64,
    /// The largest single pair or boundary violation.
    pub max_violation_mm: f64,
    /// `true` when some ring is wholly inside another - the case a pure
    /// boundary-distance measure calls feasible.
    pub containment: bool,
}

/// Whether a point is strictly inside a ring, by ray casting.
fn point_inside(ring: &[[f64; 2]], point: [f64; 2]) -> bool {
    let mut inside = false;
    let count = ring.len();
    for index in 0..count {
        let a = ring[index];
        let b = ring[(index + 1) % count];
        if (a[1] > point[1]) != (b[1] > point[1]) {
            let t = (point[1] - a[1]) / (b[1] - a[1]);
            if point[0] < a[0] + t * (b[0] - a[0]) {
                inside = !inside;
            }
        }
    }
    inside
}

fn segment_point_distance(a: [f64; 2], b: [f64; 2], point: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length_squared = dx * dx + dy * dy;
    let t = if length_squared > 0.0 {
        (((point[0] - a[0]) * dx + (point[1] - a[1]) * dy) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    libm::hypot(point[0] - (a[0] + t * dx), point[1] - (a[1] + t * dy))
}

fn boundary_distance(ring: &[[f64; 2]], point: [f64; 2]) -> f64 {
    let count = ring.len();
    let mut best = f64::INFINITY;
    for index in 0..count {
        best = best.min(segment_point_distance(
            ring[index],
            ring[(index + 1) % count],
            point,
        ));
    }
    best
}

fn segments_cross(a0: [f64; 2], a1: [f64; 2], b0: [f64; 2], b1: [f64; 2]) -> bool {
    let orient = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
        (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])
    };
    let d1 = orient(a0, a1, b0);
    let d2 = orient(a0, a1, b1);
    let d3 = orient(b0, b1, a0);
    let d4 = orient(b0, b1, a1);
    ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0))
}

fn ring_min_distance(first: &[[f64; 2]], second: &[[f64; 2]]) -> f64 {
    let mut best = f64::INFINITY;
    for index in 0..first.len() {
        let a0 = first[index];
        let a1 = first[(index + 1) % first.len()];
        for other in 0..second.len() {
            let b0 = second[other];
            let b1 = second[(other + 1) % second.len()];
            best = best
                .min(segment_point_distance(a0, a1, b0))
                .min(segment_point_distance(a0, a1, b1))
                .min(segment_point_distance(b0, b1, a0))
                .min(segment_point_distance(b0, b1, a1));
        }
    }
    best
}

/// The independent signed gap between two whole transformed rings.
///
/// Positive is the minimum boundary distance. Negative is the deepest interior
/// vertex's distance to the other boundary - a different quantity from the
/// minimum translation vector, deliberately, so that agreeing with Φ means
/// something.
pub fn independent_ring_gap(first: &[[f64; 2]], second: &[[f64; 2]]) -> (f64, bool) {
    let mut deepest = 0.0f64;
    let mut any_inside_first = false;
    let mut all_inside_first = true;
    for point in second {
        if point_inside(first, *point) {
            any_inside_first = true;
            deepest = deepest.max(boundary_distance(first, *point));
        } else {
            all_inside_first = false;
        }
    }
    let mut any_inside_second = false;
    let mut all_inside_second = true;
    for point in first {
        if point_inside(second, *point) {
            any_inside_second = true;
            deepest = deepest.max(boundary_distance(second, *point));
        } else {
            all_inside_second = false;
        }
    }
    let mut crossing = false;
    'outer: for index in 0..first.len() {
        let a0 = first[index];
        let a1 = first[(index + 1) % first.len()];
        for other in 0..second.len() {
            let b0 = second[other];
            let b1 = second[(other + 1) % second.len()];
            if segments_cross(a0, a1, b0, b1) {
                crossing = true;
                break 'outer;
            }
        }
    }
    let containment = (all_inside_first && !second.is_empty() && !crossing)
        || (all_inside_second && !first.is_empty() && !crossing);
    if any_inside_first || any_inside_second || crossing {
        (-deepest, containment)
    } else {
        (ring_min_distance(first, second), false)
    }
}

/// The independent score of the current state.
pub fn independent_score(
    geometry: &Geometry,
    contract: &Contract,
    target_depth_mm: f64,
) -> IndependentScore {
    let count = geometry.piece_rings.len();
    let clearance = contract.pair_clearance_mm();
    let mut score = IndependentScore::default();
    for first in 0..count {
        for second in (first + 1)..count {
            let (gap, containment) = independent_ring_gap(
                geometry.ring_slice(first),
                geometry.ring_slice(second),
            );
            score.containment |= containment;
            let violation = (clearance - gap).max(0.0);
            score.total_violation_mm += violation;
            score.max_violation_mm = score.max_violation_mm.max(violation);
        }
    }
    for piece in 0..count {
        let box_mm = geometry.piece_bounds[piece];
        // The same L/R/B-physical, top-inset split Phi uses. This is the one
        // place the "independent" oracle is not independent - it shares
        // `boundary_residuals` - and that is exactly why it could not have
        // caught the phantom top (Sol review 15 §D). The conventions are kept
        // identical on purpose rather than diverging silently; the share is
        // recorded in the round's README as a limit of the oracle.
        for deficit in super::broad_phase::boundary_residuals(box_mm, contract, target_depth_mm) {
            score.total_violation_mm += deficit;
            score.max_violation_mm = score.max_violation_mm.max(deficit);
        }
    }
    score
}

/// The independent active violation incident on one piece.
pub fn independent_incident(
    geometry: &Geometry,
    contract: &Contract,
    target_depth_mm: f64,
    piece: usize,
) -> f64 {
    let count = geometry.piece_rings.len();
    let clearance = contract.pair_clearance_mm();
    let mut total = 0.0;
    for other in 0..count {
        if other == piece {
            continue;
        }
        let (gap, _) =
            independent_ring_gap(geometry.ring_slice(piece), geometry.ring_slice(other));
        total += (clearance - gap).max(0.0);
    }
    for deficit in
        super::broad_phase::boundary_residuals(geometry.piece_bounds[piece], contract, target_depth_mm)
    {
        total += deficit;
    }
    total
}

/// The exact dual verdict on one pose set, plus the worst deficit the round
/// kernel's own critical radius reports when it refuses.
pub struct ExactVerdict {
    pub kernel_valid: bool,
    pub worst_pair_shortfall_micron: i64,
    pub worst_boundary_shortfall_micron: i64,
    pub failed: bool,
}

/// Runs the round kernel over a pose set and measures how far outside it is.
pub fn kernel_verdict(
    pieces: &[GeneralFastPiece<'_>],
    poses: &[Pose],
    sources: &[PieceSource],
    contract: &Contract,
) -> Option<ExactVerdict> {
    let radius = crate::canonical_grid::to_grid_mm(contract.expansion_mm())? as i64;
    let two_r = 2 * radius;
    if !certifies(two_r) {
        return None;
    }
    let inset_mm = contract.sheet_inset_mm();
    let inset = [
        crate::canonical_grid::to_grid_mm(inset_mm)? as i64,
        crate::canonical_grid::to_grid_mm(inset_mm)? as i64,
        crate::canonical_grid::to_grid_mm(contract.sheet_short_axis_mm - inset_mm)? as i64,
        crate::canonical_grid::to_grid_mm(contract.sheet_long_axis_mm - inset_mm)? as i64,
    ];
    let placements = super::publish::placements_of(sources, poses);
    let mut sets = Vec::with_capacity(placements.len());
    for (piece, placement) in pieces.iter().zip(&placements) {
        let polygon = piece
            .polygon
            .transformed(
                placement.rotation_deg,
                placement.mirrored,
                placement.translate_short_axis,
                placement.translate_long_axis,
            )
            .ok()?;
        sets.push(GridSet::of(&polygon)?);
    }
    let mut verdict = ExactVerdict {
        kernel_valid: true,
        worst_pair_shortfall_micron: 0,
        worst_boundary_shortfall_micron: 0,
        failed: false,
    };
    let ceiling = 8 * two_r.max(1);
    for set in &sets {
        if !boundary_admissible(set, radius, inset[0], inset[1], inset[2], inset[3]) {
            verdict.kernel_valid = false;
            match critical_boundary_radius_micron(set, inset[0], inset[1], inset[2], inset[3]) {
                Some(critical) => {
                    verdict.worst_boundary_shortfall_micron =
                        verdict.worst_boundary_shortfall_micron.max(radius - critical);
                }
                None => verdict.failed = true,
            }
        }
    }
    for first in 0..sets.len() {
        for second in (first + 1)..sets.len() {
            if pair_admissible(&sets[first], &sets[second], two_r) {
                continue;
            }
            verdict.kernel_valid = false;
            match critical_two_r_micron(&sets[first], &sets[second], ceiling) {
                Some((critical, _)) => {
                    verdict.worst_pair_shortfall_micron =
                        verdict.worst_pair_shortfall_micron.max(two_r - critical);
                }
                None => verdict.failed = true,
            }
        }
    }
    Some(verdict)
}

/// The corpus's per-state findings.
#[derive(Clone, Copy, Debug, Default)]
pub struct CorpusReport {
    pub states: u64,
    /// States whose Φ says feasible.
    pub proxy_feasible: u64,
    /// Proxy-feasible states the exact gates refused.
    pub proxy_feasible_exact_invalid: u64,
    /// Of those, the ones whose deficit left the 4 µm band. **Must be zero.**
    pub outside_band: u64,
    /// The largest deficit, in micrometres, seen on a proxy-feasible state.
    pub worst_band_micron: i64,
    /// States with a containment that Φ called feasible. **Must be zero.**
    pub containment_false_feasible: u64,
    pub containment_states: u64,
    /// Incremental-row rebuilds that did not reproduce a cold rebuild bit for
    /// bit. **Must be zero.**
    pub incremental_mismatches: u64,
    /// Force-correlation denominators and numerators.
    pub force_steps: u64,
    pub force_active_improved: u64,
    pub force_total_not_worse: u64,
    /// Kernel refusals with no critical radius at all: a hard fail-closed.
    pub kernel_unmeasurable: u64,
    pub compressed_states: u64,
    pub grazing_states: u64,
    pub containment_family_states: u64,
    /// Force correlation, split by family, because a single rate cannot say
    /// whether the field is wrong or one adversarial family is hard.
    pub force_steps_by_family: [u64; 3],
    pub force_active_improved_by_family: [u64; 3],
    pub force_total_not_worse_by_family: [u64; 3],
}

/// One force-correlation step that did **not** improve the independent active
/// violation, kept so a rate can be read as a mechanism rather than a number.
#[derive(Clone, Copy, Debug)]
pub struct ForceMiss {
    pub ordinal: u64,
    pub family: Family,
    pub piece: usize,
    pub scale_mm: f64,
    pub before_active_mm: f64,
    pub after_active_mm: f64,
    pub before_total_mm: f64,
    pub after_total_mm: f64,
    pub phi_before: f64,
    pub phi_after: f64,
    pub step_mm: f64,
}

/// Which question a corpus state is there to ask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// Compressed to a 1 %, 3 % or 10 % residual and shaken: deeply infeasible.
    /// This is the family the force-correlation clauses are measured on.
    Compressed,
    /// The parent layout shaken by micrometres. This is the family that
    /// actually straddles feasibility, and therefore the only one that can
    /// exercise "no proxy-feasible state is exact-invalid outside the 4 um
    /// band". A corpus made only of `Compressed` states passes that clause
    /// vacuously, with `proxyFeasible = 0`, which is how a soundness battery
    /// fakes itself.
    Grazing,
    /// One piece translated inside another. A pure boundary-distance measure
    /// calls these feasible; Phi must not.
    Containment,
}

impl Family {
    pub fn label(self) -> &'static str {
        match self {
            Family::Compressed => "compressed",
            Family::Grazing => "grazing",
            Family::Containment => "containment",
        }
    }
}

/// One deterministic state of the corpus.
pub struct CorpusState {
    pub poses: Vec<Pose>,
    pub family: Family,
    pub residual: f64,
    pub scale_mm: f64,
    pub ordinal: u64,
}

/// Generates the corpus.
///
/// Deterministic in the strong sense: the same `(parent, seed, count)` produces
/// the same states on every run and in every process, because the only source
/// of variation is [`counter_hash`] over an explicit key. Nothing here reads a
/// clock, an address, or an iteration count.
///
/// The families rotate on `ordinal % 6`: three compressed, two grazing, one
/// containment, so a 1,000-state corpus is 500 / 334 / 166 and every clause has
/// a population to be measured on.
pub fn generate(
    sources: &[PieceSource],
    parent: &[Pose],
    contract: &Contract,
    parent_depth_mm: f64,
    lower_scale_mm: f64,
    states: u64,
    seed: u64,
) -> Vec<CorpusState> {
    let residuals = [0.01f64, 0.03, 0.10];
    let mut out = Vec::with_capacity(states as usize);
    for ordinal in 0..states {
        let slot = ordinal % 6;
        let (family, residual) = match slot {
            0..=2 => (Family::Compressed, residuals[(ordinal / 6 % 3) as usize]),
            3 | 4 => (Family::Grazing, 0.0),
            _ => (Family::Containment, 0.0),
        };
        let (mut poses, scale) = match family {
            Family::Compressed => {
                let target =
                    parent_depth_mm - residual * (parent_depth_mm - lower_scale_mm);
                let factor =
                    super::homotopy::affine_compression_factor(sources, parent, contract, target);
                (
                    super::homotopy::compressed(sources, parent, contract, factor),
                    0.25 + 20.0 * residual,
                )
            }
            // A geometric ladder from 20 um down to 0.16 um, straddling the
            // 4 um canonicalization guard from both sides.
            Family::Grazing => (
                parent.to_vec(),
                0.020 / (1u64 << (ordinal / 6 % 8)) as f64,
            ),
            Family::Containment => (parent.to_vec(), 0.0),
        };
        for (index, pose) in poses.iter_mut().enumerate() {
            if scale <= 0.0 {
                continue;
            }
            let key = counter_hash(&[seed, ordinal, index as u64]);
            pose.tx_mm += (unit(key) * 2.0 - 1.0) * scale;
            pose.ty_mm += (unit(key >> 17) * 2.0 - 1.0) * scale;
            pose.theta_deg += (unit(key >> 34) * 2.0 - 1.0) * scale * 4.0;
        }
        if family == Family::Containment && sources.len() >= 2 {
            // The smallest piece, translated onto another piece's centroid. Its
            // boundary distance to the host stays large and positive; only a
            // measure that sees containment can call this infeasible.
            let key = counter_hash(&[seed, ordinal, 0xC047]);
            let smallest = index_of_extreme(sources, false);
            let host = (key % sources.len() as u64) as usize;
            let host = if host == smallest {
                index_of_extreme(sources, true)
            } else {
                host
            };
            if host != smallest {
                let (sin, cos) = super::state::pose_sin_cos(poses[host].theta_deg);
                let host_centre = super::state::apply_pose(
                    sources[host].centroid,
                    poses[host].mirrored,
                    sin,
                    cos,
                    poses[host].tx_mm,
                    poses[host].ty_mm,
                );
                let (sin, cos) = super::state::pose_sin_cos(poses[smallest].theta_deg);
                let rotated = super::state::apply_pose(
                    sources[smallest].centroid,
                    poses[smallest].mirrored,
                    sin,
                    cos,
                    0.0,
                    0.0,
                );
                poses[smallest].tx_mm = host_centre[0] - rotated[0];
                poses[smallest].ty_mm = host_centre[1] - rotated[1];
            }
        }
        out.push(CorpusState {
            poses,
            family,
            residual,
            scale_mm: scale,
            ordinal,
        });
    }
    out
}

fn index_of_extreme(sources: &[PieceSource], largest: bool) -> usize {
    let mut best = 0usize;
    for index in 1..sources.len() {
        let better = if largest {
            sources[index].area_mm2 > sources[best].area_mm2
        } else {
            sources[index].area_mm2 < sources[best].area_mm2
        };
        if better {
            best = index;
        }
    }
    best
}

fn unit(key: u64) -> f64 {
    ((key >> 11) as f64) / ((1u64 << 53) as f64)
}

/// Runs the corpus and returns its report.
///
/// The four Gate-0 questions, in order: false feasibility outside the band,
/// containment, incremental-versus-cold, and force correlation.
#[allow(clippy::too_many_arguments)]
pub fn run(
    pieces: &[GeneralFastPiece<'_>],
    sources: &[PieceSource],
    settings: GeneralFastSettings,
    contract: &Contract,
    parent: &[Pose],
    parent_depth_mm: f64,
    lower_scale_mm: f64,
    states: u64,
    seed: u64,
    target_depth_mm: f64,
) -> (CorpusReport, Vec<ForceMiss>) {
    let _ = settings;
    let mut misses: Vec<ForceMiss> = Vec::new();
    let mut report = CorpusReport::default();
    let generated = generate(
        sources,
        parent,
        contract,
        parent_depth_mm,
        lower_scale_mm,
        states,
        seed,
    );
    let count = sources.len();
    for entry in &generated {
        report.states += 1;
        match entry.family {
            Family::Compressed => report.compressed_states += 1,
            Family::Grazing => report.grazing_states += 1,
            Family::Containment => report.containment_family_states += 1,
        }
        let geometry = build_geometry(sources, &entry.poses);
        let mut state = IcsState {
            poses: entry.poses.clone(),
            geometry,
            pair_rows: vec![PairRow::default(); pair_count(count)],
            edge_rows: vec![[EdgeRow::default(); 4]; count],
            target_depth_mm,
        };
        let mut work = WorkVector::default();
        rebuild_all(&mut state, contract, &mut work);
        let totals = fold(&state);
        let independent = independent_score(&state.geometry, contract, target_depth_mm);
        if independent.containment {
            report.containment_states += 1;
            if totals.max_violation_mm <= 0.0 {
                report.containment_false_feasible += 1;
            }
        }
        if totals.max_violation_mm <= 0.0 {
            report.proxy_feasible += 1;
            match kernel_verdict(pieces, &entry.poses, sources, contract) {
                Some(verdict) => {
                    if verdict.failed {
                        report.kernel_unmeasurable += 1;
                        report.proxy_feasible_exact_invalid += 1;
                        report.outside_band += 1;
                    } else if !verdict.kernel_valid {
                        report.proxy_feasible_exact_invalid += 1;
                        let worst = verdict
                            .worst_pair_shortfall_micron
                            .max(verdict.worst_boundary_shortfall_micron);
                        report.worst_band_micron = report.worst_band_micron.max(worst);
                        if worst > 4 {
                            report.outside_band += 1;
                        }
                    }
                }
                None => {
                    report.kernel_unmeasurable += 1;
                    report.outside_band += 1;
                }
            }
        }
        // Incremental versus cold, on the piece the descent would move first.
        let piece = super::energy::highest_pressure_piece(&state);
        let mut moved = state.clone();
        moved.poses[piece].tx_mm += 0.37;
        moved.poses[piece].ty_mm -= 0.21;
        moved.poses[piece].theta_deg += 1.7;
        transform_piece(sources, &mut moved.geometry, &moved.poses, piece);
        rebuild_piece_rows(&mut moved, contract, piece, &mut work);
        let incremental = fold(&moved);
        let mut cold = moved.clone();
        rebuild_all(&mut cold, contract, &mut work);
        let cold_totals = fold(&cold);
        if incremental.raw.to_bits() != cold_totals.raw.to_bits()
            || incremental.guided.to_bits() != cold_totals.guided.to_bits()
        {
            report.incremental_mismatches += 1;
        }
        // Force correlation: one accepted step from the descent's own ladder.
        let before_active = independent_incident(&state.geometry, contract, target_depth_mm, piece);
        let before_total = independent.total_violation_mm;
        let mut stepped = state.clone();
        let config = super::descent::DescentConfig::derive(contract, sources, seed);
        let mut descent = super::descent::Descent::new(
            config,
            pieces.iter().map(|piece| piece.allow_rotation).collect(),
        );
        if descent.propose(&mut stepped, sources, contract, piece, &mut work) {
            let slot = match entry.family {
                Family::Compressed => 0,
                Family::Grazing => 1,
                Family::Containment => 2,
            };
            report.force_steps += 1;
            report.force_steps_by_family[slot] += 1;
            let after_active =
                independent_incident(&stepped.geometry, contract, target_depth_mm, piece);
            let after_total =
                independent_score(&stepped.geometry, contract, target_depth_mm).total_violation_mm;
            if after_active < before_active {
                report.force_active_improved += 1;
                report.force_active_improved_by_family[slot] += 1;
            } else if misses.len() < 64 {
                misses.push(ForceMiss {
                    ordinal: entry.ordinal,
                    family: entry.family,
                    piece,
                    scale_mm: entry.scale_mm,
                    before_active_mm: before_active,
                    after_active_mm: after_active,
                    before_total_mm: before_total,
                    after_total_mm: after_total,
                    phi_before: totals.raw,
                    phi_after: fold(&stepped).raw,
                    step_mm: libm::hypot(
                        stepped.poses[piece].tx_mm - state.poses[piece].tx_mm,
                        stepped.poses[piece].ty_mm - state.poses[piece].ty_mm,
                    ),
                });
            }
            if after_total <= before_total {
                report.force_total_not_worse += 1;
                report.force_total_not_worse_by_family[slot] += 1;
            }
        }
    }
    (report, misses)
}
