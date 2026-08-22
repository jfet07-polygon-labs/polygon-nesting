//! Publication: the only place in this engine where exact geometry is asked
//! anything, and the only place `best_exact` is written.
//!
//! ```text
//! continuous pose  (never snapped)
//!   -> transformed source rings
//!   -> GridSet::of                       the sole 1 µm canonicalization
//!   -> round kernel, request-scoped, Exclusive semantics, r = 2.500, allowance 0
//!   -> frozen-theta, same-strip, <= 4n row micro-repair, <= 16 µm per piece
//!   -> untouched validate_placements_against_contract
//!   -> best_exact
//! ```
//!
//! Four refusals both designers insisted on, all of them enforced below:
//!
//! * **no pose pre-snap.** `tx`, `ty`, `theta` reach `PolygonSet::transformed`
//!   as they are; a second quantization on top of `GridSet::of` would be a
//!   rounding nobody validates.
//! * **allowance zero.** `search_offset_allowance_mm` is search-only and never
//!   part of publication legality, so this module builds its settings with it
//!   forced to `0.0` whatever the caller passed.
//! * **pure predicates, request-scoped.** `pair_admissible` / `boundary_admissible`
//!   at the request's own radius. Not `KernelMode::Union`, not the process-global
//!   arm, not 2.502.
//! * **no millimetre-scale legalization.** `epsilon_grid = 2*ceil(sqrt(2) * 1 µm)
//!   = 4 µm` is the whole band, 16 µm the whole per-piece cap. A source-faithful
//!   Φ at zero may disagree with the exact geometry at grid scale and nowhere
//!   else; a repair that returns half a millimetre is a broken proxy wearing a
//!   legalizer's coat, and the checkpoint is discarded instead.

use crate::canonical_grid::to_grid_mm;
use crate::geometry::general_polygon::PolygonSet;
use crate::search::general_fast::{
    validate_placements_against_contract, GeneralFastPiece, GeneralFastPlacement,
    GeneralFastSettings,
};
use crate::validation::round_envelope::{
    boundary_admissible, certifies, critical_boundary_radius_micron, critical_two_r_micron,
    pair_admissible, GridSet,
};
use sha2::{Digest, Sha256};

use super::diagnostics::{ExactCheckpoint, WorkVector};
use super::state::{Contract, IcsState, PieceSource, Pose};

/// `epsilon_grid = 2 * ceil(sqrt(2) * 1 µm) = 4 µm`, in millimetres.
///
/// Derived, not chosen: `GridSet::of` moves a vertex by at most half a grid
/// step in each axis, so two canonicalized rings can approach each other by at
/// most `sqrt(2)` grid steps more than their continuous originals, and the
/// guard has to cover both sides of one pair.
pub const EPSILON_GRID_MM: f64 = 0.004;

/// The publication band and the repair caps. Frozen Round-1 knobs.
#[derive(Clone, Copy, Debug)]
pub struct PublicationLimits {
    /// Attempt only when `max_g` is inside this band.
    pub band_mm: f64,
    pub epsilon_grid_mm: f64,
    /// Cumulative displacement cap per piece: `4 * epsilon_grid`.
    pub max_piece_displacement_mm: f64,
    /// Row corrections allowed, as a multiple of `n`.
    pub repair_rows_per_piece: usize,
    /// The smallest depth gain that counts as an improvement: 1 µm.
    pub minimum_improvement_mm: f64,
}

impl Default for PublicationLimits {
    fn default() -> Self {
        Self {
            band_mm: EPSILON_GRID_MM,
            epsilon_grid_mm: EPSILON_GRID_MM,
            max_piece_displacement_mm: 4.0 * EPSILON_GRID_MM,
            repair_rows_per_piece: 4,
            minimum_improvement_mm: 0.001,
        }
    }
}

/// A dual-valid layout, and what it cost to get there.
#[derive(Clone, Debug)]
pub struct Publication {
    pub placements: Vec<GeneralFastPlacement>,
    pub poses: Vec<Pose>,
    pub raw_source_depth_mm: f64,
    pub placement_fingerprint: String,
    pub repair_rows: u64,
    pub repair_max_displacement_mm: f64,
    pub repair_depth_giveback_mm: f64,
}

/// The result of one attempt: always a checkpoint row, sometimes a publication.
#[derive(Clone, Debug)]
pub struct Attempt {
    pub checkpoint: ExactCheckpoint,
    pub publication: Option<Publication>,
}

/// The placements a set of continuous poses denotes. No rounding of any kind.
pub fn placements_of(sources: &[PieceSource], poses: &[Pose]) -> Vec<GeneralFastPlacement> {
    sources
        .iter()
        .zip(poses)
        .map(|(source, pose)| GeneralFastPlacement {
            piece_id: source.id.clone(),
            rotation_deg: pose.rotation_deg(),
            mirrored: pose.mirrored,
            translate_short_axis: pose.tx_mm,
            translate_long_axis: pose.ty_mm,
        })
        .collect()
}

/// This module's own placement digest.
///
/// Deliberately not `general_relaxed::general_placement_fingerprint`: the
/// converged spec's "do not use" list names that module, and importing one
/// hash out of it would make the overlap-ICS tree depend on the relaxed lane
/// for no reason other than a digest. The construction is the same - piece IDs
/// sorted, angle keyed at 1e-6 degrees, translations keyed on the canonical
/// 1 µm grid - so a fingerprint from here is comparable with one from there,
/// which is what "the constructor fingerprint is never a child" needs.
pub fn placement_fingerprint(placements: &[GeneralFastPlacement]) -> String {
    let mut canonical = placements.iter().collect::<Vec<_>>();
    canonical.sort_by(|first, second| first.piece_id.cmp(&second.piece_id));
    let mut digest = Sha256::new();
    for placement in canonical {
        digest.update((placement.piece_id.len() as u64).to_le_bytes());
        digest.update(placement.piece_id.as_bytes());
        digest.update(angle_key(placement.rotation_deg).to_le_bytes());
        digest.update([u8::from(placement.mirrored)]);
        digest.update(grid_key(placement.translate_short_axis).to_le_bytes());
        digest.update(grid_key(placement.translate_long_axis).to_le_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn angle_key(angle_deg: f64) -> i64 {
    (angle_deg.rem_euclid(360.0) * 1_000_000.0).round() as i64
}

fn grid_key(value: f64) -> i64 {
    to_grid_mm(value).map(|value| value as i64).unwrap_or(i64::MAX)
}

/// The publication settings: the caller's contract with the search allowance
/// forced to zero.
pub fn publication_settings(settings: GeneralFastSettings) -> GeneralFastSettings {
    let mut settings = settings;
    settings.search_offset_allowance_mm = 0.0;
    settings
}

fn transformed(
    piece: &GeneralFastPiece<'_>,
    placement: &GeneralFastPlacement,
) -> Result<PolygonSet, String> {
    piece
        .polygon
        .transformed(
            placement.rotation_deg,
            placement.mirrored,
            placement.translate_short_axis,
            placement.translate_long_axis,
        )
        .map_err(|error| error.message().to_owned())
}

fn inset_box(contract: &Contract) -> Option<[i64; 4]> {
    let inset = contract.sheet_inset_mm();
    let values = [
        inset,
        inset,
        contract.sheet_short_axis_mm - inset,
        contract.sheet_long_axis_mm - inset,
    ];
    let mut out = [0i64; 4];
    for (slot, value) in out.iter_mut().zip(values) {
        *slot = to_grid_mm(value)? as i64;
    }
    Some(out)
}

/// The transformed depth of one placement set, in the published convention:
/// `max source y + sheet edge clearance`, on the untouched `f64` rings.
///
/// Placements are matched to pieces **by id**, not by index: the constructor
/// returns them in its own order and a pose fixture in the fixture's, and
/// zipping those against the piece list silently measures the wrong geometry.
/// `validate_placements_against_contract` matches by id for the same reason.
pub fn raw_depth_of(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    contract: &Contract,
) -> f64 {
    let by_id = pieces
        .iter()
        .map(|piece| (piece.id, piece))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut deepest = f64::NEG_INFINITY;
    for placement in placements {
        let Some(piece) = by_id.get(placement.piece_id.as_str()).copied() else {
            return f64::NAN;
        };
        let (sin, cos) = placement.rotation_deg.to_radians().sin_cos();
        for region in piece.polygon.regions() {
            for point in region.outer.source_points() {
                let mirror_x = if placement.mirrored { -point.x } else { point.x };
                let y = mirror_x * sin + point.y * cos + placement.translate_long_axis;
                deepest = deepest.max(y);
            }
        }
    }
    deepest + contract.sheet_edge_clearance_mm
}

struct KernelScan {
    admissible: bool,
    failing_pairs: Vec<(usize, usize)>,
    failing_boundaries: Vec<usize>,
}

fn scan(sets: &[GridSet], two_r: i64, radius: i64, inset: [i64; 4]) -> KernelScan {
    let mut failing_pairs = Vec::new();
    let mut failing_boundaries = Vec::new();
    for (index, set) in sets.iter().enumerate() {
        if !boundary_admissible(set, radius, inset[0], inset[1], inset[2], inset[3]) {
            failing_boundaries.push(index);
        }
    }
    for first in 0..sets.len() {
        for second in (first + 1)..sets.len() {
            if !pair_admissible(&sets[first], &sets[second], two_r) {
                failing_pairs.push((first, second));
            }
        }
    }
    KernelScan {
        admissible: failing_pairs.is_empty() && failing_boundaries.is_empty(),
        failing_pairs,
        failing_boundaries,
    }
}

/// One publication attempt on the current ICS state.
///
/// `incumbent_depth_mm` is the protected exact incumbent's depth; the attempt
/// is only made when the state can beat it by at least 1 µm inside the locked
/// strip. Failure returns to the ICS with `best_exact` untouched - that is the
/// caller's contract, and this function has no way to violate it because it
/// never receives the incumbent by mutable reference.
pub fn attempt(
    state: &IcsState,
    sources: &[PieceSource],
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    contract: &Contract,
    limits: PublicationLimits,
    max_violation_mm: f64,
    incumbent_depth_mm: f64,
    proposal_ordinal: u64,
    work: &mut WorkVector,
) -> Option<Attempt> {
    let proxy_depth = super::state::raw_source_depth_mm(&state.geometry, contract);
    if !(max_violation_mm <= limits.band_mm) {
        return None;
    }
    if proxy_depth > state.target_depth_mm {
        return None;
    }
    if proxy_depth > incumbent_depth_mm - limits.minimum_improvement_mm {
        return None;
    }
    work.exact_checkpoints += 1;
    let settings = publication_settings(settings);
    let mut poses = state.poses.clone();
    let mut placements = placements_of(sources, &poses);
    let mut checkpoint = ExactCheckpoint {
        proposal_ordinal,
        target_depth_mm: state.target_depth_mm,
        max_violation_mm,
        proxy_raw_depth_mm: proxy_depth,
        kernel_exclusive_valid: false,
        contract_valid: false,
        repair_rows: 0,
        repair_max_displacement_mm: 0.0,
        repair_depth_giveback_mm: 0.0,
        published_raw_depth_mm: None,
        refusal: None,
    };

    let radius = match to_grid_mm(contract.expansion_mm()) {
        Some(value) => value as i64,
        None => {
            checkpoint.refusal = Some(
                "the contract radius is not an integer micrometre; the round preflight is bypassed rather than rounded outward"
                    .to_owned(),
            );
            return Some(Attempt {
                checkpoint,
                publication: None,
            });
        }
    };
    let two_r = 2 * radius;
    if !certifies(two_r) {
        checkpoint.refusal =
            Some(format!("the round kernel does not certify at 2r = {two_r}"));
        return Some(Attempt {
            checkpoint,
            publication: None,
        });
    }
    let inset = match inset_box(contract) {
        Some(value) => value,
        None => {
            checkpoint.refusal = Some("the inset rectangle is outside the canonical grid".to_owned());
            return Some(Attempt {
                checkpoint,
                publication: None,
            });
        }
    };

    let mut sets = Vec::with_capacity(placements.len());
    for (piece, placement) in pieces.iter().zip(&placements) {
        let polygon = match transformed(piece, placement) {
            Ok(polygon) => polygon,
            Err(message) => {
                checkpoint.refusal = Some(message);
                return Some(Attempt {
                    checkpoint,
                    publication: None,
                });
            }
        };
        match GridSet::of(&polygon) {
            Some(set) => sets.push(set),
            None => {
                checkpoint.refusal =
                    Some("a transformed ring left the canonical grid; failing closed".to_owned());
                return Some(Attempt {
                    checkpoint,
                    publication: None,
                });
            }
        }
    }

    let mut result = scan(&sets, two_r, radius, inset);
    let mut displacement = vec![0.0f64; placements.len()];
    // The *vector* each piece has already been moved by inside this repair
    // pass. `displacement` is the scalar cap; this is what the next row's slack
    // has to be measured from, because `state.geometry` is the pre-repair
    // geometry and a second row that reads it would grant slack a previous
    // correction has already spent (Sol review 15 §D, `publish.rs:550`).
    let mut offsets = vec![[0.0f64; 2]; placements.len()];
    let mut rows = 0u64;
    let row_budget = limits.repair_rows_per_piece * placements.len();
    while !result.admissible {
        if rows as usize >= row_budget {
            checkpoint.refusal = Some(format!(
                "repair exceeded its {row_budget}-row budget with {} pair and {} boundary rows still failing",
                result.failing_pairs.len(),
                result.failing_boundaries.len()
            ));
            break;
        }
        let corrected = repair_one_row(
            &result,
            &sets,
            state,
            &mut poses,
            &mut displacement,
            &mut offsets,
            contract,
            &limits,
            two_r,
            radius,
            inset,
        );
        let Some(touched) = corrected else {
            checkpoint.refusal = Some(
                "a failing row is outside the 4 µm band or has no sheet slack; discarding the checkpoint"
                    .to_owned(),
            );
            break;
        };
        rows += 1;
        work.repair_rows += 1;
        placements = placements_of(sources, &poses);
        for index in touched {
            let polygon = match transformed(&pieces[index], &placements[index]) {
                Ok(polygon) => polygon,
                Err(message) => {
                    checkpoint.refusal = Some(message);
                    return Some(Attempt {
                        checkpoint,
                        publication: None,
                    });
                }
            };
            match GridSet::of(&polygon) {
                Some(set) => sets[index] = set,
                None => {
                    checkpoint.refusal =
                        Some("a repaired ring left the canonical grid".to_owned());
                    return Some(Attempt {
                        checkpoint,
                        publication: None,
                    });
                }
            }
        }
        result = scan(&sets, two_r, radius, inset);
    }

    checkpoint.repair_rows = rows;
    checkpoint.repair_max_displacement_mm =
        displacement.iter().copied().fold(0.0f64, f64::max);
    checkpoint.kernel_exclusive_valid = result.admissible;
    let published_depth = raw_depth_of(pieces, &placements, contract);
    checkpoint.repair_depth_giveback_mm = published_depth - proxy_depth;
    if !result.admissible {
        return Some(Attempt {
            checkpoint,
            publication: None,
        });
    }
    if published_depth > state.target_depth_mm {
        checkpoint.refusal = Some(
            "repair would have enlarged the locked strip; the target is immutable".to_owned(),
        );
        return Some(Attempt {
            checkpoint,
            publication: None,
        });
    }
    match validate_placements_against_contract(pieces, &placements, settings) {
        Ok(()) => checkpoint.contract_valid = true,
        Err(error) => {
            checkpoint.refusal = Some(error.to_string());
            return Some(Attempt {
                checkpoint,
                publication: None,
            });
        }
    }
    checkpoint.published_raw_depth_mm = Some(published_depth);
    let publication = Publication {
        placement_fingerprint: placement_fingerprint(&placements),
        placements,
        poses,
        raw_source_depth_mm: published_depth,
        repair_rows: rows,
        repair_max_displacement_mm: checkpoint.repair_max_displacement_mm,
        repair_depth_giveback_mm: checkpoint.repair_depth_giveback_mm,
    };
    Some(Attempt {
        checkpoint,
        publication: Some(publication),
    })
}

/// Corrects the lexicographically first failing row, or refuses.
///
/// Boundaries first, then pairs, both in index order: an ordered Gauss-Seidel
/// pass, not a simultaneous solve, so every correction sees the previous one.
/// Rotations are frozen throughout - a repair that turned a piece would be a
/// second optimizer, and its displacement could not be capped per piece.
#[allow(clippy::too_many_arguments)]
fn repair_one_row(
    scan_result: &KernelScan,
    sets: &[GridSet],
    state: &IcsState,
    poses: &mut [Pose],
    displacement: &mut [f64],
    offsets: &mut [[f64; 2]],
    contract: &Contract,
    limits: &PublicationLimits,
    two_r: i64,
    radius: i64,
    inset: [i64; 4],
) -> Option<Vec<usize>> {
    let guard = limits.epsilon_grid_mm;
    let guard_micron = (guard * 1000.0).round() as i64;
    if let Some(index) = scan_result.failing_boundaries.first().copied() {
        let critical = critical_boundary_radius_micron(
            &sets[index],
            inset[0],
            inset[1],
            inset[2],
            inset[3],
        )?;
        let shortfall = radius - critical;
        if shortfall <= 0 || shortfall > guard_micron {
            return None;
        }
        let correction = (shortfall + guard_micron) as f64 / 1000.0;
        let (min_x, min_y, max_x, max_y) = sets[index].bounds_micron();
        // Which side is binding: the tightest of the four, same rule the
        // closed-form critical radius uses.
        let slacks = [
            min_x - inset[0],
            min_y - inset[1],
            inset[2] - max_x,
            inset[3] - max_y,
        ];
        let mut binding = 0usize;
        for candidate in 1..4 {
            if slacks[candidate] < slacks[binding] {
                binding = candidate;
            }
        }
        let direction = match binding {
            0 => [1.0, 0.0],
            1 => [0.0, 1.0],
            2 => [-1.0, 0.0],
            _ => [0.0, -1.0],
        };
        if displacement[index] + correction > limits.max_piece_displacement_mm {
            return None;
        }
        poses[index].tx_mm += direction[0] * correction;
        poses[index].ty_mm += direction[1] * correction;
        displacement[index] += correction;
        offsets[index][0] += direction[0] * correction;
        offsets[index][1] += direction[1] * correction;
        return Some(vec![index]);
    }
    let (first, second) = scan_result.failing_pairs.first().copied()?;
    let ceiling = 8 * two_r.max(1);
    let (critical, saturated) = critical_two_r_micron(&sets[first], &sets[second], ceiling)?;
    if saturated {
        return None;
    }
    let shortfall = two_r - critical;
    if shortfall <= 0 || shortfall > guard_micron {
        return None;
    }
    let correction = (shortfall + guard_micron) as f64 / 1000.0;
    let row = super::energy::pair_row(state, first, second);
    let normal = if libm::hypot(row.contact.normal[0], row.contact.normal[1]) > 0.0 {
        row.contact.normal
    } else {
        // The continuous field has no active normal for this pair - it is
        // clear at Φ's own clearance and only the canonical grid disagrees.
        // Separate along the centroid difference, which is the same direction
        // for a grid-scale disagreement.
        let delta = [
            state.geometry.centroids[first][0] - state.geometry.centroids[second][0],
            state.geometry.centroids[first][1] - state.geometry.centroids[second][1],
        ];
        let length = libm::hypot(delta[0], delta[1]);
        if !(length > 0.0) {
            return None;
        }
        [delta[0] / length, delta[1] / length]
    };
    let slack_first = sheet_slack(
        shifted_bounds(state.geometry.piece_bounds[first], offsets[first]),
        normal,
        contract,
        state.target_depth_mm,
    )
    .min(limits.max_piece_displacement_mm - displacement[first]);
    let slack_second = sheet_slack(
        shifted_bounds(state.geometry.piece_bounds[second], offsets[second]),
        [-normal[0], -normal[1]],
        contract,
        state.target_depth_mm,
    )
    .min(limits.max_piece_displacement_mm - displacement[second]);
    if slack_first.max(0.0) + slack_second.max(0.0) < correction {
        return None;
    }
    let mut share_first = (correction / 2.0).min(slack_first.max(0.0));
    let mut share_second = correction - share_first;
    if share_second > slack_second.max(0.0) {
        share_second = slack_second.max(0.0);
        share_first = correction - share_second;
    }
    if share_first > slack_first.max(0.0) {
        return None;
    }
    poses[first].tx_mm += normal[0] * share_first;
    poses[first].ty_mm += normal[1] * share_first;
    poses[second].tx_mm -= normal[0] * share_second;
    poses[second].ty_mm -= normal[1] * share_second;
    displacement[first] += share_first;
    displacement[second] += share_second;
    offsets[first][0] += normal[0] * share_first;
    offsets[first][1] += normal[1] * share_first;
    offsets[second][0] -= normal[0] * share_second;
    offsets[second][1] -= normal[1] * share_second;
    Some(vec![first, second])
}

/// A pre-repair box translated by what this repair pass has already spent on
/// that piece. Repair freezes rotation, so a rigid translation moves the box
/// exactly - no re-derivation from geometry is needed and none would be more
/// accurate.
#[inline]
fn shifted_bounds(bounds: [f64; 4], offset: [f64; 2]) -> [f64; 4] {
    [
        bounds[0] + offset[0],
        bounds[1] + offset[1],
        bounds[2] + offset[0],
        bounds[3] + offset[1],
    ]
}

/// How far a piece may move along `direction` before its material leaves the
/// strip. Never negative-infinite: an axis the direction does not touch does
/// not bind.
///
/// The four sides carry the same split Phi does: left, right and bottom are
/// physical sheet edges at `edge + sag`; the top is the tighter of the locked
/// strip in the sag-less depth convention and the physical sheet top at
/// `edge + sag` (Grok review 10 §B.1, `publish.rs:550-608`).
fn sheet_slack(
    bounds: [f64; 4],
    direction: [f64; 2],
    contract: &Contract,
    target_depth_mm: f64,
) -> f64 {
    let physical = contract.physical_edge_clearance_mm();
    let top = (target_depth_mm - contract.depth_top_inset_mm())
        .min(contract.sheet_long_axis_mm - physical);
    let mut slack = f64::INFINITY;
    if direction[0] > 0.0 {
        slack = slack.min((contract.sheet_short_axis_mm - physical - bounds[2]) / direction[0]);
    } else if direction[0] < 0.0 {
        slack = slack.min((bounds[0] - physical) / -direction[0]);
    }
    if direction[1] > 0.0 {
        slack = slack.min((top - bounds[3]) / direction[1]);
    } else if direction[1] < 0.0 {
        slack = slack.min((bounds[1] - physical) / -direction[1]);
    }
    if slack.is_finite() {
        slack.max(0.0)
    } else {
        f64::INFINITY
    }
}
