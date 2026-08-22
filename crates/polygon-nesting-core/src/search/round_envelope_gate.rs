//! The round-envelope kernel's soundness battery, as an instrument.
//!
//! Sol review 11 item 1, refined by Sol review 12 §3.2 and kept unmodified by
//! Grok review 7 §2, asks for the kernel's verdict against the current
//! authority on three populations plus a ±1 µm sweep, with **zero false
//! accepts** and no canonical-valid layout lost. That is a per-pair and
//! per-boundary question, and the composite returns one boolean and one
//! message, so — exactly as Gate A did — the deliverable is a census rather
//! than a verdict.
//!
//! This module is the census. It reaches no search path, no scorer and no
//! publication route; it is compiled only under `round-envelope-kernel` and is
//! named by nothing in `src/` outside itself and the battery example.
//!
//! # What it measures, and against what
//!
//! * [`census`] — every pair and every boundary of one pose set under the
//!   kernel, with the critical clearance `2r*` at which each flips. Gate A's
//!   `import_gate` bisects a Clipper offset and its `r*` therefore carries
//!   that offset's own output quantization; this bisects a predicate that is
//!   exact at every integer radius, so `2r*` here is the largest integer
//!   micrometre threshold the canonical rings still admit — the *floor* of
//!   their true rational minimum boundary distance, one-sided by less than
//!   1 µm (sol-review-13 corrected the earlier "full stop" wording).
//! * [`miter_census`] — the same two questions asked of HEAD's authority: the
//!   miter offset rebuilt per placement and `polygons_overlap_exact` per pair.
//!   Same loop, same order, different envelope, so a per-row disagreement is
//!   attributable to the envelope and to nothing else.
//! * [`envelope_half_nanoseconds`] and [`composite_nanoseconds`] — the economy.
//!   The first prices the half this kernel replaces; the second prices the whole
//!   confirmation, which is where Sol's `<=1.25x` is written.
//!
//! # The arming guard
//!
//! [`ArmedKernel`] is the battery's own RAII arming, for exercising the *wired*
//! path — `validate_and_measure_placements` with the kernel installed — rather
//! than only the kernel's own functions. A battery that tested the kernel and
//! not the wire would not have tested the thing that publishes.

use std::collections::BTreeMap;
use std::time::Instant;

use crate::geometry::general_polygon::PolygonSet;
use crate::search::general_fast::{
    collision_expansion_mm, collision_sheet_inset_mm, polygons_overlap_exact,
    validate_and_measure_placements, GeneralFastPiece, GeneralFastPlacement, GeneralFastSettings,
};
use crate::validation::round_envelope::{
    boundary_admissible, certifies, critical_boundary_radius_micron, critical_two_r_micron,
    pair_admissible_measured, vertex_count, GridSet, KernelMode,
};

/// Arms the kernel for a scope and puts back what it found.
///
/// Same shape as `portfolio::RoundEnvelopeArming`, duplicated here rather than
/// exported from there because the battery must be able to arm without
/// constructing a coordinator — and because a test instrument that borrows the
/// production arming would make the production arming untestable.
pub struct ArmedKernel {
    previous: KernelMode,
}

impl ArmedKernel {
    pub fn install(mode: KernelMode) -> Self {
        Self {
            previous: crate::validation::round_envelope::set_kernel_mode(mode),
        }
    }
}

impl Drop for ArmedKernel {
    fn drop(&mut self) {
        crate::validation::round_envelope::set_kernel_mode(self.previous);
    }
}

/// One pair of placements under one envelope.
#[derive(Clone, Debug)]
pub struct PairRow {
    pub first_index: usize,
    pub second_index: usize,
    pub first_piece_id: String,
    pub second_piece_id: String,
    /// Whether this envelope admits the pair.
    pub admissible: bool,
    /// The exact critical doubled radius, in millimetres of material clearance:
    /// the largest `2r` at which the pair is still admissible. `None` when the
    /// material overlaps at any radius — a containment or a crossing.
    pub critical_two_r_mm: Option<f64>,
    /// Whether the bisection saturated its ceiling, so the number above is a
    /// floor and not the answer.
    pub critical_saturated: bool,
    /// Whether the piece-level integer box alone decided it.
    pub certified_by_box: bool,
    /// Segment pairs that reached the exact narrow predicate.
    pub narrow_segment_pairs: u64,
}

/// One placement against the inset sheet under one envelope.
#[derive(Clone, Debug)]
pub struct BoundaryRow {
    pub index: usize,
    pub piece_id: String,
    pub admissible: bool,
    /// The largest radius, in millimetres, at which this placement still fits
    /// the inset rectangle. `None` when the material is already outside it.
    pub critical_radius_mm: Option<f64>,
}

/// The whole census for one pose set under one envelope.
#[derive(Clone, Debug)]
pub struct Census {
    pub label: String,
    pub envelope: &'static str,
    pub expansion_mm: f64,
    /// The expansion on the canonical grid, in micrometres — the integer
    /// `PolygonSet::offset` is handed and the integer the kernel compares
    /// against.
    pub radius_micron: i64,
    pub sheet_inset_mm: f64,
    /// `[low x, low y, high x, high y]` of the inset rectangle, in micrometres,
    /// read exactly as `PolygonSet::fits_rect` reads it.
    pub inset_box_micron: [i64; 4],
    /// `false` when the kernel refused to certify this configuration at all and
    /// the miter authority is the only answer. Always `true` for the miter
    /// census.
    pub certified: bool,
    pub admissible: bool,
    pub pair_count: usize,
    pub pair_failure_count: usize,
    pub boundary_failure_count: usize,
    /// Every failing row, plus the `report_top` tightest by critical radius.
    pub pairs: Vec<PairRow>,
    pub boundaries: Vec<BoundaryRow>,
    /// How many of the `pair_count` pairs the integer box alone certified.
    pub box_certified_pairs: usize,
    /// How many segment pairs reached the exact narrow predicate, over the
    /// whole scan.
    pub narrow_segment_pairs: u64,
    /// The vertices the envelope carries. For the kernel this is the canonical
    /// source rings; for the miter it is Clipper's offset output.
    pub envelope_vertex_total: usize,
}

fn placement_pieces<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    placements: &[GeneralFastPlacement],
) -> Result<Vec<&'a GeneralFastPiece<'a>>, String> {
    let by_id = pieces
        .iter()
        .map(|piece| (piece.id, piece))
        .collect::<BTreeMap<_, _>>();
    placements
        .iter()
        .map(|placement| {
            by_id
                .get(placement.piece_id.as_str())
                .copied()
                .ok_or_else(|| format!("unknown piece {}", placement.piece_id))
        })
        .collect()
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

fn inset_box(settings: GeneralFastSettings) -> Result<[i64; 4], String> {
    let inset = collision_sheet_inset_mm(settings);
    let values = [
        inset,
        inset,
        settings.sheet_short_axis_mm - inset,
        settings.sheet_long_axis_mm - inset,
    ];
    let mut box_micron = [0i64; 4];
    for (slot, value) in box_micron.iter_mut().zip(values) {
        let grid = crate::canonical_grid::to_grid_mm(value)
            .ok_or_else(|| format!("{value} is outside the canonical grid"))?;
        *slot = grid as i64;
    }
    Ok(box_micron)
}

fn radius_micron(settings: GeneralFastSettings) -> Result<i64, String> {
    let expansion = collision_expansion_mm(settings);
    let grid = crate::canonical_grid::to_grid_mm(expansion)
        .ok_or_else(|| format!("expansion {expansion} is outside the canonical grid"))?;
    Ok(grid as i64)
}

/// The kernel's census of one pose set: every pair, every boundary, exact.
///
/// `report_top` bounds the rows returned to the tightest by critical radius;
/// every *failing* row is returned whatever that bound is. The counts and the
/// verdict are over the full scan either way.
pub fn census(
    label: impl Into<String>,
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
    report_top: usize,
) -> Result<Census, String> {
    let radius = radius_micron(settings)?;
    let inset = inset_box(settings)?;
    let by_placement = placement_pieces(pieces, placements)?;
    let mut sets = Vec::with_capacity(placements.len());
    for (piece, placement) in by_placement.iter().zip(placements) {
        let polygon = transformed(piece, placement)?;
        sets.push(GridSet::of(&polygon));
    }
    let certified = certifies(2 * radius) && sets.iter().all(Option::is_some);
    let mut result = Census {
        label: label.into(),
        envelope: "round-kernel",
        expansion_mm: collision_expansion_mm(settings),
        radius_micron: radius,
        sheet_inset_mm: collision_sheet_inset_mm(settings),
        inset_box_micron: inset,
        certified,
        admissible: false,
        pair_count: placements.len() * placements.len().saturating_sub(1) / 2,
        pair_failure_count: 0,
        boundary_failure_count: 0,
        pairs: Vec::new(),
        boundaries: Vec::new(),
        box_certified_pairs: 0,
        narrow_segment_pairs: 0,
        envelope_vertex_total: 0,
    };
    if !certified {
        return Ok(result);
    }
    let sets = sets.into_iter().map(Option::unwrap).collect::<Vec<_>>();
    result.envelope_vertex_total = sets.iter().map(vertex_count).sum();

    // Boundaries, in placement order.
    let ceiling = 8 * radius.max(1);
    for (index, (set, placement)) in sets.iter().zip(placements).enumerate() {
        let admissible =
            boundary_admissible(set, radius, inset[0], inset[1], inset[2], inset[3]);
        if !admissible {
            result.boundary_failure_count += 1;
        }
        result.boundaries.push(BoundaryRow {
            index,
            piece_id: placement.piece_id.clone(),
            admissible,
            critical_radius_mm: critical_boundary_radius_micron(
                set, inset[0], inset[1], inset[2], inset[3],
            )
            .map(|value| value as f64 / 1000.0),
        });
    }

    // Pairs, lexicographically.
    for first_index in 0..sets.len() {
        for second_index in (first_index + 1)..sets.len() {
            let (admissible, work) =
                pair_admissible_measured(&sets[first_index], &sets[second_index], 2 * radius);
            if work.certified_by_box {
                result.box_certified_pairs += 1;
            }
            result.narrow_segment_pairs += work.narrow_segment_pairs;
            if !admissible {
                result.pair_failure_count += 1;
            }
            let critical = critical_two_r_micron(&sets[first_index], &sets[second_index], ceiling);
            result.pairs.push(PairRow {
                first_index,
                second_index,
                first_piece_id: placements[first_index].piece_id.clone(),
                second_piece_id: placements[second_index].piece_id.clone(),
                admissible,
                critical_two_r_mm: critical.map(|(value, _)| value as f64 / 1000.0),
                critical_saturated: critical.is_some_and(|(_, saturated)| saturated),
                certified_by_box: work.certified_by_box,
                narrow_segment_pairs: work.narrow_segment_pairs,
            });
        }
    }
    result.admissible = result.pair_failure_count == 0 && result.boundary_failure_count == 0;
    trim(&mut result, report_top);
    Ok(result)
}

/// HEAD's authority, asked the same two questions in the same order.
///
/// The envelope is `PolygonSet::offset` itself — the production function,
/// unmodified and not a shadow of it — so a row where this and [`census`]
/// disagree is a disagreement between the miter join and the disc, and nothing
/// else differs between the two loops.
pub fn miter_census(
    label: impl Into<String>,
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
    report_top: usize,
) -> Result<Census, String> {
    let radius = radius_micron(settings)?;
    let inset = inset_box(settings)?;
    let expansion = collision_expansion_mm(settings);
    let by_placement = placement_pieces(pieces, placements)?;
    let mut envelopes = Vec::with_capacity(placements.len());
    for (piece, placement) in by_placement.iter().zip(placements) {
        let polygon = transformed(piece, placement)?;
        envelopes.push(
            polygon
                .offset(expansion)
                .map_err(|error| error.message().to_owned())?,
        );
    }
    let mut result = Census {
        label: label.into(),
        envelope: "miter (HEAD authority)",
        expansion_mm: expansion,
        radius_micron: radius,
        sheet_inset_mm: collision_sheet_inset_mm(settings),
        inset_box_micron: inset,
        certified: true,
        admissible: false,
        pair_count: placements.len() * placements.len().saturating_sub(1) / 2,
        pair_failure_count: 0,
        boundary_failure_count: 0,
        pairs: Vec::new(),
        boundaries: Vec::new(),
        box_certified_pairs: 0,
        narrow_segment_pairs: 0,
        envelope_vertex_total: envelopes.iter().map(PolygonSet::vertex_count).sum(),
    };
    let inset_mm = collision_sheet_inset_mm(settings);
    for (index, (envelope, placement)) in envelopes.iter().zip(placements).enumerate() {
        let admissible = envelope.fits_rect(
            inset_mm,
            inset_mm,
            settings.sheet_short_axis_mm - inset_mm,
            settings.sheet_long_axis_mm - inset_mm,
        );
        if !admissible {
            result.boundary_failure_count += 1;
        }
        result.boundaries.push(BoundaryRow {
            index,
            piece_id: placement.piece_id.clone(),
            admissible,
            critical_radius_mm: None,
        });
    }
    for first_index in 0..envelopes.len() {
        for second_index in (first_index + 1)..envelopes.len() {
            let overlaps = polygons_overlap_exact(&envelopes[first_index], &envelopes[second_index])
                .map_err(|error| error.message().to_owned())?;
            if overlaps {
                result.pair_failure_count += 1;
            }
            result.pairs.push(PairRow {
                first_index,
                second_index,
                first_piece_id: placements[first_index].piece_id.clone(),
                second_piece_id: placements[second_index].piece_id.clone(),
                admissible: !overlaps,
                critical_two_r_mm: None,
                critical_saturated: false,
                certified_by_box: false,
                narrow_segment_pairs: 0,
            });
        }
    }
    result.admissible = result.pair_failure_count == 0 && result.boundary_failure_count == 0;
    trim(&mut result, report_top);
    Ok(result)
}

/// Reduces a census's row lists to every failing row plus the `report_top`
/// tightest, after any full-scan comparison the caller wanted has been taken.
///
/// Separate from [`census`] so that a caller can ask for the whole scan, compare
/// it row by row against [`miter_census`], and only then decide what to print —
/// which is what the battery does, because a P0 is a row-level disagreement and
/// a trimmed list could hide one.
pub fn trim(census: &mut Census, report_top: usize) {
    census.pairs.sort_by(|first, second| {
        first
            .admissible
            .cmp(&second.admissible)
            .then_with(|| {
                first
                    .critical_two_r_mm
                    .unwrap_or(f64::NEG_INFINITY)
                    .total_cmp(&second.critical_two_r_mm.unwrap_or(f64::NEG_INFINITY))
            })
            .then_with(|| first.first_index.cmp(&second.first_index))
            .then_with(|| first.second_index.cmp(&second.second_index))
    });
    let keep = census
        .pairs
        .iter()
        .filter(|row| !row.admissible)
        .count()
        .max(report_top);
    census.pairs.truncate(keep);
    census.boundaries.sort_by(|first, second| {
        first
            .admissible
            .cmp(&second.admissible)
            .then_with(|| {
                first
                    .critical_radius_mm
                    .unwrap_or(f64::NEG_INFINITY)
                    .total_cmp(&second.critical_radius_mm.unwrap_or(f64::NEG_INFINITY))
            })
            .then_with(|| first.index.cmp(&second.index))
    });
    let keep = census
        .boundaries
        .iter()
        .filter(|row| !row.admissible)
        .count()
        .max(report_top);
    census.boundaries.truncate(keep);
}

/// The two envelope halves, timed against each other in one process.
///
/// Returns `(miter nanoseconds, round nanoseconds)` as the **median** over
/// `repetitions` interleaved passes. Interleaved rather than blocked because
/// this box runs other campaign rounds and a blocked A/B would attribute their
/// load to whichever arm went second; the median over interleaved passes is the
/// weakest claim that still separates a 5x from a 1.05x, which is the size of
/// the question.
///
/// The miter arm is the rebuild plus `polygons_overlap_exact` per pair — the
/// envelope half of `validate_and_measure_placements` and nothing else. The
/// round arm is `GridSet::of` per placement plus `pair_admissible` per pair.
/// Neither includes the material contract validator, which is unchanged and is
/// where the great majority of a confirmation's milliseconds are
/// (docs/experiments/parallel-compression-schedule/ §3).
pub fn envelope_half_nanoseconds(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
    repetitions: usize,
) -> Result<(f64, f64), String> {
    let radius = radius_micron(settings)?;
    let inset = inset_box(settings)?;
    let expansion = collision_expansion_mm(settings);
    let inset_mm = collision_sheet_inset_mm(settings);
    let by_placement = placement_pieces(pieces, placements)?;
    let mut miter = Vec::with_capacity(repetitions);
    let mut round = Vec::with_capacity(repetitions);
    for _ in 0..repetitions.max(1) {
        let started = Instant::now();
        let mut envelopes = Vec::with_capacity(placements.len());
        let mut fits = true;
        for (piece, placement) in by_placement.iter().zip(placements) {
            let polygon = transformed(piece, placement)?;
            let envelope = polygon
                .offset(expansion)
                .map_err(|error| error.message().to_owned())?;
            fits &= envelope.fits_rect(
                inset_mm,
                inset_mm,
                settings.sheet_short_axis_mm - inset_mm,
                settings.sheet_long_axis_mm - inset_mm,
            );
            envelopes.push(envelope);
        }
        let mut clear = true;
        for first in 0..envelopes.len() {
            for second in (first + 1)..envelopes.len() {
                clear &= !polygons_overlap_exact(&envelopes[first], &envelopes[second])
                    .map_err(|error| error.message().to_owned())?;
            }
        }
        std::hint::black_box((fits, clear));
        miter.push(started.elapsed().as_nanos() as f64);

        let started = Instant::now();
        let mut sets = Vec::with_capacity(placements.len());
        let mut fits = true;
        for (piece, placement) in by_placement.iter().zip(placements) {
            let polygon = transformed(piece, placement)?;
            let set = GridSet::of(&polygon).ok_or("outside the kernel's domain")?;
            fits &= boundary_admissible(&set, radius, inset[0], inset[1], inset[2], inset[3]);
            sets.push(set);
        }
        let mut clear = true;
        for first in 0..sets.len() {
            for second in (first + 1)..sets.len() {
                clear &= pair_admissible_measured(&sets[first], &sets[second], 2 * radius).0;
            }
        }
        std::hint::black_box((fits, clear));
        round.push(started.elapsed().as_nanos() as f64);
    }
    Ok((median(&mut miter), median(&mut round)))
}

/// The whole confirmation, timed in each of the three modes.
///
/// This is the quantity Sol review 12 §3.2's `<=1.25x` is written in: one call
/// to `validate_and_measure_placements`, both halves, exactly as a mode-34
/// confirmation asks it.
///
/// Interleaved rather than blocked because this box runs other campaign rounds
/// and a blocked A/B would attribute their load to whichever arm went last.
///
/// A confirmation that **refuses** short-circuits, and the three modes
/// short-circuit in different places, so only a layout all three admit prices
/// the same amount of work three times. The caller is given the three verdicts
/// so it can say which cells those are.
pub fn composite_nanoseconds(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
    repetitions: usize,
) -> CompositeTiming {
    let mut timings = [
        (KernelMode::Off, Vec::new(), false),
        (KernelMode::Exclusive, Vec::new(), false),
        (KernelMode::Union, Vec::new(), false),
    ];
    for _ in 0..repetitions.max(1) {
        for slot in timings.iter_mut() {
            let _armed = ArmedKernel::install(slot.0);
            let started = Instant::now();
            let verdict = validate_and_measure_placements(pieces, placements, settings);
            slot.1.push(started.elapsed().as_nanos() as f64);
            slot.2 = verdict.is_ok();
            std::hint::black_box(verdict.is_ok());
        }
    }
    CompositeTiming {
        miter_nanoseconds: median(&mut timings[0].1),
        exclusive_nanoseconds: median(&mut timings[1].1),
        union_nanoseconds: median(&mut timings[2].1),
        miter_accepts: timings[0].2,
        exclusive_accepts: timings[1].2,
        union_accepts: timings[2].2,
    }
}

/// One layout's confirmation cost under each of the three envelope authorities.
#[derive(Clone, Copy, Debug)]
pub struct CompositeTiming {
    pub miter_nanoseconds: f64,
    pub exclusive_nanoseconds: f64,
    pub union_nanoseconds: f64,
    pub miter_accepts: bool,
    pub exclusive_accepts: bool,
    pub union_accepts: bool,
}

/// The intersection area, in mm², of two placements' **miter** envelopes at the
/// production radius.
///
/// This is what completes the attribution of a row where the miter authority
/// admits a pair and the kernel refuses it. `offset_miter(P, r)` contains
/// `P (+) disc(r)` exactly, so if the kernel's exact minimum boundary distance
/// is below `2r` then the *true* miter envelopes overlap with positive area.
/// A measured area of exactly zero therefore is not evidence that the miter
/// disagrees about the geometry — it is evidence that Clipper's offset output
/// was re-quantized to the canonical grid and the sliver rounded away. That is
/// a one-grid-step property of the output stage, not of the join.
pub fn miter_pair_intersection_area_mm2(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
    first: usize,
    second: usize,
) -> Result<f64, String> {
    let expansion = collision_expansion_mm(settings);
    let by_placement = placement_pieces(pieces, placements)?;
    let build = |index: usize| -> Result<PolygonSet, String> {
        let piece = by_placement.get(index).ok_or("index out of range")?;
        transformed(piece, &placements[index])?
            .offset(expansion)
            .map_err(|error| error.message().to_owned())
    };
    build(first)?
        .intersection_area_mm2(&build(second)?)
        .map_err(|error| error.message().to_owned())
}

fn median(values: &mut [f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        values[middle]
    } else {
        (values[middle - 1] + values[middle]) / 2.0
    }
}

/// The wired composite's verdict in each of the three modes.
///
/// `Ok((short axis span, long axis depth))` or the authority's own message. The
/// point of going through `validate_and_measure_placements` rather than through
/// [`census`] is that this is the function that publishes: a battery that
/// exercised the kernel and not the wire would not have tested the thing under
/// test.
pub fn wired_verdicts(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
) -> WiredVerdicts {
    let one = |mode: KernelMode| -> Result<(f64, f64), String> {
        let _armed = ArmedKernel::install(mode);
        validate_and_measure_placements(pieces, placements, settings)
            .map(|metrics| {
                (
                    metrics.used_short_axis_span_mm,
                    metrics.used_long_axis_depth_mm,
                )
            })
            .map_err(|error| error.to_string())
    };
    WiredVerdicts {
        miter: one(KernelMode::Off),
        exclusive: one(KernelMode::Exclusive),
        union: one(KernelMode::Union),
    }
}

/// One layout's verdict under each of the three envelope authorities.
#[derive(Clone, Debug)]
pub struct WiredVerdicts {
    /// HEAD's authority, unmodified.
    pub miter: Result<(f64, f64), String>,
    /// The kernel alone.
    pub exclusive: Result<(f64, f64), String>,
    /// The hybrid: whichever of the two admits it.
    pub union: Result<(f64, f64), String>,
}
