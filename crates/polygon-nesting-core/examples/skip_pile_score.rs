//! The skip pile, scored by three authorities.
//!
//! ```text
//! skip_pile_score PLAN.json
//! ```
//!
//! One JSON document on stdout. Nothing is written in place.
//!
//! # The question
//!
//! The compression schedule never asks the exact tier about a frontier its
//! *relaxed surrogate* already calls infeasible, and the surrogate's collision
//! geometry is the production **miter** offset. The round-envelope gate measured
//! the consequence: `confirmationsRefused = 0` on all 108 runs of its
//! twelve-parent matched gate, because 149 762 frontiers were suppressed one
//! level above the confirmation. So the proxy, not the confirmation, is the
//! filter — and the question is whether it hides a **released region**: a
//! frontier the disc kernel accepts and the miter refuses.
//!
//! `search::skip_pile_dump` writes those frontiers out as poses. This reads them
//! back and asks three authorities about each one, at two radii:
//!
//! | verdict | function | what it is |
//! |---|---|---|
//! | contract | `import_gate::authority_verdict().contract_only` | the untouched material contract validator |
//! | miter | `round_envelope_gate::wired_verdicts().miter` | HEAD's authority through the real wire point |
//! | kernel | `round_envelope_gate::wired_verdicts().exclusive` | the certified disc kernel, same wire point |
//! | union | `round_envelope_gate::wired_verdicts().union` | whichever half admits it |
//!
//! Both composite verdicts run the material contract on the same placements, so
//! `kernel accept ∧ miter refuse` is a **released layout** and not a weaker
//! authority's opinion. The per-pair attribution beside it is the same pair of
//! censuses the soundness battery uses, so the magnitude of a release can be
//! read against Gate A's own 0.5057 mm and against the 1 µm canonical grid step.
//!
//! # The sample
//!
//! A cell's dump is a sequence in step order. Scoring all of it is not
//! affordable — a full double census is 1830 exact pair tests plus 1830 Clipper
//! overlap tests — so the plan names a per-cell `sample`, and the records are
//! taken **evenly spread across the whole sequence** rather than from its head.
//! A head sample would be a sample of the shallow end of the ladder, which is
//! the part of it where the least is at stake.
//!
//! Everything about the sample is in the output: `dumpedRecords`,
//! `sampledRecords`, and every sampled record's own `seq` and `step`.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader};

use polygon_nesting_core::domain::ImportedPiece;
use polygon_nesting_core::geometry::general_polygon::PolygonSet;
use polygon_nesting_core::geometry::general_source::polygon_set_from_imported_piece;
use polygon_nesting_core::search::general_fast::{
    GeneralFastPiece, GeneralFastPlacement, GeneralFastSettings,
};
use polygon_nesting_core::search::import_gate::authority_verdict;
use polygon_nesting_core::search::round_envelope_gate::{
    census, miter_census, miter_pair_intersection_area_mm2, trim, wired_verdicts, ArmedKernel,
    Census,
};
use polygon_nesting_core::validation::general_polygon::{
    material_pair_distance_mm, material_sheet_clearance_mm, GeneralPlacement,
};
use polygon_nesting_core::validation::round_envelope::KernelMode;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------- the plan

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Plan {
    request: String,
    edge_clearance_mm: f64,
    pair_clearance_mm: f64,
    /// The radii every sampled record is asked about. `0.002` is the allowance
    /// the ladder ran at, so it is the radius the skip actually happened at;
    /// `0.0` is the contract radius itself.
    allowances_mm: Vec<f64>,
    /// The one radius the per-pair censuses are taken at.
    census_allowance_mm: f64,
    /// How many sampled records per cell get the full double census, on top of
    /// every record whose composite verdicts disagree — those always get one.
    census_budget: usize,
    report_top: usize,
    cells: Vec<Cell>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Cell {
    label: String,
    /// The JSONL the dump hook wrote.
    path: String,
    /// How many records to score. Taken evenly spread across the whole file.
    sample: usize,
    /// The cell's `confirmationsSkippedInfeasible`, as the run that wrote this
    /// dump reported it. Restated here so the scoring document carries the
    /// reproduction rather than pointing at another file for it.
    #[serde(default)]
    skips_suppressed: Option<usize>,
    /// How many *distinct* frontiers the sink says it wrote. The dump
    /// deduplicates by placement fingerprint, so this is smaller than
    /// `skips_suppressed` by exactly the repeated ones, and the scorer checks
    /// the file it read against it.
    #[serde(default)]
    distinct_expected: Option<usize>,
}

// ------------------------------------------------------- the request loader
//
// `examples/round_envelope_battery.rs`'s, unchanged in substance: the same
// `polygon_set_from_imported_piece`, the same `GeneralFastSettings::
// deterministic_test` base, the same `sheet.width >= sheet.height` axis
// normalisation.

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    sheet: Sheet,
    #[serde(default)]
    padding: Option<f64>,
    pieces: Vec<RequestPiece>,
    source_pieces: Vec<ImportedPiece>,
    #[serde(default)]
    settings: Option<RequestSettings>,
    #[serde(default)]
    options: Option<LegacyOptions>,
}

#[derive(Deserialize)]
struct Sheet {
    width: f64,
    height: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestPiece {
    id: String,
    source_piece_id: String,
    allow_rotation: bool,
    #[serde(default = "default_true")]
    allow_mirror: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestSettings {
    padding: f64,
    allow_global_rotation: bool,
    #[serde(default = "default_true")]
    allow_global_mirror: bool,
    geometry: GeometrySettings,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyOptions {
    allow_global_rotation: bool,
    #[serde(default = "default_true")]
    allow_global_mirror: bool,
    irregular_settings: LegacyIrregularSettings,
}

#[derive(Deserialize)]
struct LegacyIrregularSettings {
    geometry: GeometrySettings,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeometrySettings {
    flattening_sag_tolerance_mm: f64,
    clearance_safety_margin_mm: f64,
}

fn default_true() -> bool {
    true
}

struct OwnedPiece {
    id: String,
    polygon: PolygonSet,
    allow_rotation: bool,
    allow_mirror: bool,
}

// ------------------------------------------------------ the dump's own rows

/// One line of the dump. The `placements` array is the pose-fixture shape the
/// battery already reads, so nothing here re-invents a pose reader.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkipLine {
    seq: usize,
    step: usize,
    work_units: usize,
    frontier_depth_mm: f64,
    floor_depth_mm: f64,
    /// The slice's incumbent at the moment of the skip. A released layout whose
    /// own depth beats this is not just a legal layout HEAD refused — it is a
    /// **publication** HEAD refused.
    published_depth_mm: f64,
    parent_depth_mm: f64,
    collision_pairs: usize,
    boundary_violations: usize,
    boundary_loss: f64,
    fingerprint: String,
    placements: Vec<Pose>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pose {
    piece_id: String,
    rotation_deg: f64,
    #[serde(default)]
    mirrored: bool,
    translate_short_axis: f64,
    translate_long_axis: f64,
}

fn placements_of(line: &SkipLine) -> Vec<GeneralFastPlacement> {
    line.placements
        .iter()
        .map(|pose| GeneralFastPlacement {
            piece_id: pose.piece_id.clone(),
            rotation_deg: pose.rotation_deg,
            mirrored: pose.mirrored,
            translate_short_axis: pose.translate_short_axis,
            translate_long_axis: pose.translate_long_axis,
        })
        .collect()
}

/// The indices to score, evenly spread over `total` records.
///
/// `floor(i * total / want)` rather than the first `want`: the dump is in step
/// order and the head of it is the shallow end of the ladder. With
/// `want >= total` every record is taken.
fn spread(total: usize, want: usize) -> Vec<usize> {
    if total == 0 {
        return Vec::new();
    }
    if want == 0 || want >= total {
        return (0..total).collect();
    }
    (0..want).map(|index| index * total / want).collect()
}

// -------------------------------------------------------------- reporting

fn verdict_json(verdict: &Result<(f64, f64), String>) -> serde_json::Value {
    match verdict {
        Ok((span, depth)) => json!({
            "accepted": true,
            "usedShortAxisSpanMm": span,
            "usedLongAxisDepthMm": depth,
        }),
        Err(message) => json!({"accepted": false, "message": message}),
    }
}

fn pair_material_distance(
    piece_by_id: &BTreeMap<String, GeneralFastPiece<'_>>,
    placements: &[GeneralFastPlacement],
    first: usize,
    second: usize,
) -> Option<f64> {
    let build = |index: usize| -> Option<GeneralPlacement<'_>> {
        let placement = placements.get(index)?;
        let piece = piece_by_id.get(&placement.piece_id)?;
        Some(GeneralPlacement {
            piece_id: piece.id,
            polygon: piece.polygon,
            rotation_deg: placement.rotation_deg,
            mirrored: placement.mirrored,
            translate_x: placement.translate_short_axis,
            translate_y: placement.translate_long_axis,
        })
    };
    material_pair_distance_mm(&build(first)?, &build(second)?).ok()
}

/// The binding edge clearance of one placement, on untouched source rings.
///
/// The contract validator's own accessor, so a boundary release's magnitude can
/// be read against the material the contract demands rather than against a
/// second implementation of the same distance.
fn sheet_material_clearance(
    piece_by_id: &BTreeMap<String, GeneralFastPiece<'_>>,
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
    index: usize,
) -> Option<f64> {
    let placement = placements.get(index)?;
    let piece = piece_by_id.get(&placement.piece_id)?;
    let independent = GeneralPlacement {
        piece_id: piece.id,
        polygon: piece.polygon,
        rotation_deg: placement.rotation_deg,
        mirrored: placement.mirrored,
        translate_x: placement.translate_short_axis,
        translate_y: placement.translate_long_axis,
    };
    material_sheet_clearance_mm(
        &independent,
        settings.sheet_short_axis_mm,
        settings.sheet_long_axis_mm,
    )
    .ok()?
    .into_iter()
    .fold(None, |best: Option<f64>, value| {
        Some(best.map_or(value, |current| current.min(value)))
    })
}

/// The per-pair comparison between the two envelopes, over the **full** scan.
struct PairSplit {
    both_admit: usize,
    both_refuse: usize,
    /// The disc admits, the miter refuses. This is the join's price, and its
    /// magnitude is the whole question about the released region's class.
    kernel_admits_miter_refuses: Vec<(usize, usize)>,
    /// The miter admits, the disc refuses. A P0 if it ever happens on a layout
    /// the miter accepts whole; reported unconditionally so it cannot hide.
    miter_admits_kernel_refuses: Vec<(usize, usize)>,
}

fn split(kernel: &Census, miter: &Census) -> PairSplit {
    let mut result = PairSplit {
        both_admit: 0,
        both_refuse: 0,
        kernel_admits_miter_refuses: Vec::new(),
        miter_admits_kernel_refuses: Vec::new(),
    };
    let miter_pairs = miter
        .pairs
        .iter()
        .map(|row| ((row.first_index, row.second_index), row.admissible))
        .collect::<BTreeMap<_, _>>();
    for row in &kernel.pairs {
        let key = (row.first_index, row.second_index);
        let Some(&miter_admits) = miter_pairs.get(&key) else {
            continue;
        };
        match (row.admissible, miter_admits) {
            (true, true) => result.both_admit += 1,
            (false, false) => result.both_refuse += 1,
            (true, false) => result.kernel_admits_miter_refuses.push(key),
            (false, true) => result.miter_admits_kernel_refuses.push(key),
        }
    }
    result
}

/// The same comparison on the **sheet boundary**, which is where the first
/// release this diagnostic found actually lived.
///
/// The miter envelope's binding corner sticks further out than the disc's, so a
/// placement can fit the inset rectangle under one envelope and not the other
/// with no pair involved at all. A per-pair-only reading would have reported
/// that layout as an unattributed release.
struct BoundarySplit {
    both_admit: usize,
    both_refuse: usize,
    kernel_admits_miter_refuses: Vec<usize>,
    miter_admits_kernel_refuses: Vec<usize>,
}

fn split_boundaries(kernel: &Census, miter: &Census) -> BoundarySplit {
    let mut result = BoundarySplit {
        both_admit: 0,
        both_refuse: 0,
        kernel_admits_miter_refuses: Vec::new(),
        miter_admits_kernel_refuses: Vec::new(),
    };
    let miter_boundaries = miter
        .boundaries
        .iter()
        .map(|row| (row.index, row.admissible))
        .collect::<BTreeMap<_, _>>();
    for row in &kernel.boundaries {
        let Some(&miter_admits) = miter_boundaries.get(&row.index) else {
            continue;
        };
        match (row.admissible, miter_admits) {
            (true, true) => result.both_admit += 1,
            (false, false) => result.both_refuse += 1,
            (true, false) => result.kernel_admits_miter_refuses.push(row.index),
            (false, true) => result.miter_admits_kernel_refuses.push(row.index),
        }
    }
    result
}

/// The class an excursion falls in, named rather than left to the reader.
///
/// The two classes the diagnostic was written to tell apart are the canonical
/// grid's own step — one micrometre, where a disagreement is quantisation and
/// buys nothing — and the join tax Gate A measured at a median 0.5057 mm on a
/// miter-refused pair. The cut is deliberately generous to the small class:
/// the canonicalisation budget is `sqrt(2)` µm and one whole grid step on top
/// of it is `2.4143` µm, so anything at or below **ten** of them is called
/// grid-class rather than argued about. The label says `grid-class` and not
/// `sub-micron` because the bound is 10 µm and a label has to be true.
fn shortfall_class(millimetres: f64) -> &'static str {
    let magnitude = millimetres.abs();
    if magnitude <= 0.01 {
        "grid-class (<=10 um)"
    } else if magnitude < 0.1 {
        "intermediate (10 um .. 0.1 mm)"
    } else {
        "join-tax-class (>=0.1 mm)"
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args().skip(1);
    let plan_path = arguments
        .next()
        .ok_or("usage: skip_pile_score PLAN.json")?;
    if arguments.next().is_some() {
        return Err("no extra arguments are accepted".into());
    }
    let plan_bytes = fs::read(&plan_path)?;
    let plan_sha256 = format!("{:x}", Sha256::digest(&plan_bytes));
    let plan: Plan = serde_json::from_slice(&plan_bytes)?;

    let request_bytes = fs::read(&plan.request)?;
    let request_sha256 = format!("{:x}", Sha256::digest(&request_bytes));
    let request: Request = serde_json::from_slice(&request_bytes)?;
    let (allow_global_rotation, allow_global_mirror, geometry) =
        match (&request.settings, &request.options) {
            (Some(settings), None) => {
                let _ = settings.padding;
                (
                    settings.allow_global_rotation,
                    settings.allow_global_mirror,
                    settings.geometry,
                )
            }
            (None, Some(options)) => {
                request
                    .padding
                    .ok_or("legacy requests require top-level padding")?;
                (
                    options.allow_global_rotation,
                    options.allow_global_mirror,
                    options.irregular_settings.geometry,
                )
            }
            _ => return Err("a request must contain settings or legacy options, not both".into()),
        };
    let source_by_id = request
        .source_pieces
        .iter()
        .map(|source| (source.id.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let normalize_axes = request.sheet.width >= request.sheet.height;
    let owned = request
        .pieces
        .iter()
        .map(|piece| {
            let source = *source_by_id
                .get(piece.source_piece_id.as_str())
                .ok_or_else(|| format!("missing source piece {}", piece.source_piece_id))?;
            let polygon =
                polygon_set_from_imported_piece(source, geometry.flattening_sag_tolerance_mm)?;
            let polygon = if normalize_axes {
                let rotated = polygon.transformed(270.0, false, 0.0, 0.0)?;
                let bounds = rotated.bounds().ok_or("cannot normalize empty geometry")?;
                rotated.translated(-bounds.min_x, -bounds.min_y)?
            } else {
                polygon
            };
            Ok(OwnedPiece {
                id: piece.id.clone(),
                polygon,
                allow_rotation: allow_global_rotation && piece.allow_rotation,
                allow_mirror: allow_global_mirror && piece.allow_mirror,
            })
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    let pieces = owned
        .iter()
        .map(|piece| GeneralFastPiece {
            id: &piece.id,
            polygon: &piece.polygon,
            allow_rotation: piece.allow_rotation,
            allow_mirror: piece.allow_mirror,
        })
        .collect::<Vec<_>>();
    let piece_by_id = pieces
        .iter()
        .map(|piece| (piece.id.to_owned(), *piece))
        .collect::<BTreeMap<_, _>>();

    let base_settings = {
        let mut settings = GeneralFastSettings::deterministic_test(
            request.sheet.width.min(request.sheet.height),
            request.sheet.width.max(request.sheet.height),
        );
        settings.total_padding_mm = plan.pair_clearance_mm;
        settings.sheet_edge_clearance_mm = Some(plan.edge_clearance_mm);
        settings.clearance_safety_margin_mm = geometry.clearance_safety_margin_mm;
        settings.flattening_sag_tolerance_mm = geometry.flattening_sag_tolerance_mm;
        settings
    };
    let settings_at = |allowance: f64| {
        let mut settings = base_settings;
        settings.search_offset_allowance_mm = allowance;
        settings
    };

    let mut cell_rows = Vec::new();
    // The joint table, keyed by `contract x miter x kernel` at each allowance.
    let mut joint: BTreeMap<(String, bool, bool, bool), usize> = BTreeMap::new();
    let mut released_rows = Vec::new();
    let mut released_pair_shortfalls: Vec<f64> = Vec::new();
    let mut released_boundary_excursions: Vec<f64> = Vec::new();
    let mut p0_rows = Vec::new();
    let mut composite_readings_agree = true;

    for cell in &plan.cells {
        let file = fs::File::open(&cell.path)?;
        let mut lines = Vec::new();
        for line in BufReader::new(file).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            lines.push(serde_json::from_str::<SkipLine>(&line)?);
        }
        let indices = spread(lines.len(), cell.sample);
        // The census sample is spread over the *sampled* records for the same
        // reason the sample is spread over the file.
        let census_positions = spread(indices.len(), plan.census_budget)
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        let mut records = Vec::new();
        let mut cell_released = 0usize;
        let mut cell_all_refuse = 0usize;
        for (position, &index) in indices.iter().enumerate() {
            let line = &lines[index];
            let placements = placements_of(line);
            let mut allowance_rows = Vec::new();
            let mut census_json = serde_json::Value::Null;
            for allowance in &plan.allowances_mm {
                let settings = settings_at(*allowance);
                // The contract, and HEAD's composite, from the committed Gate A
                // instrument. The kernel is explicitly parked at `Off` around
                // it: `authority_verdict` runs whatever this process last
                // armed, and a diagnostic that inherited an arm would be
                // labelling its own arm wrongly.
                let authority = {
                    let _parked = ArmedKernel::install(KernelMode::Off);
                    authority_verdict(&pieces, &placements, settings)
                };
                let authority = authority.map_err(|error| -> Box<dyn std::error::Error> {
                    error.into()
                })?;
                let wired = wired_verdicts(&pieces, &placements, settings);
                // Two independent readings of the same authority, taken through
                // two different committed instruments. They must agree; if they
                // ever do not, one of the two is not asking what it says it is.
                if authority.composite.is_ok() != wired.miter.is_ok() {
                    composite_readings_agree = false;
                }
                let contract = authority.contract_only.is_ok();
                let miter = wired.miter.is_ok();
                let kernel = wired.exclusive.is_ok();
                *joint
                    .entry((format!("{allowance:.4}"), contract, miter, kernel))
                    .or_insert(0) += 1;
                let released = kernel && !miter;
                let p0 = miter && !kernel;
                // What this frontier was worth, whatever the authorities said
                // about it: the incumbent it would have had to beat, read at
                // the step of the skip, minus its own depth. Computed for every
                // record and not only the released ones, because the largest
                // row of the joint table turns out to be layouts *all three*
                // authorities accept — and the only interesting question about
                // that row is whether confirming them would have bought
                // anything.
                let gain = line.published_depth_mm - authority.raw_source_depth_mm;
                if (*allowance - plan.census_allowance_mm).abs() < f64::EPSILON {
                    if released {
                        cell_released += 1;
                    }
                    if !contract && !miter && !kernel {
                        cell_all_refuse += 1;
                    }
                }
                // Every record whose two envelopes disagree gets a census
                // whatever the budget says; the budget only decides how many
                // *agreeing* records are also censused, and that sample exists
                // to characterise the pile rather than to find the release.
                let wants_census = (*allowance - plan.census_allowance_mm).abs() < f64::EPSILON
                    && (released || p0 || census_positions.contains(&position));
                if wants_census {
                    // `usize::MAX`, and `trim` afterwards, because `census`'s
                    // `report_top` argument *reduces the row lists it returns*
                    // — and the comparison below reads those lists. Asking for
                    // a trimmed census and then comparing it finds only the
                    // disagreements that happened to survive the trim. A first
                    // pass of this driver passed `report_top` straight in and
                    // attributed 2 pair rows and 7 boundary rows across the
                    // whole corpus; the full scan finds 53 and 70. The layout
                    // verdicts were never affected — they come from
                    // `wired_verdicts` — but the attribution was nearly empty.
                    // This is `round_envelope_battery`'s own order, and `trim`'s
                    // own doc comment says why it is a separate function.
                    let mut kernel_census =
                        census(&cell.label, &pieces, &placements, settings, usize::MAX)
                            .map_err(|error| -> Box<dyn std::error::Error> { error.into() })?;
                    let mut miter_scan =
                        miter_census(&cell.label, &pieces, &placements, settings, usize::MAX)
                            .map_err(|error| -> Box<dyn std::error::Error> { error.into() })?;
                    let pair_split = split(&kernel_census, &miter_scan);
                    let boundary_split = split_boundaries(&kernel_census, &miter_scan);
                    let two_r = 2.0 * kernel_census.expansion_mm;
                    let radius = kernel_census.expansion_mm;
                    let mut boundary_attributed = Vec::new();
                    for index in &boundary_split.kernel_admits_miter_refuses {
                        let critical = kernel_census
                            .boundaries
                            .iter()
                            .find(|row| row.index == *index)
                            .and_then(|row| row.critical_radius_mm);
                        let excursion = critical.map(|value| value - radius);
                        if released {
                            if let Some(value) = excursion {
                                released_boundary_excursions.push(value);
                            }
                        }
                        boundary_attributed.push(json!({
                            "placementIndex": index,
                            "pieceId": placements.get(*index).map(|p| p.piece_id.clone()),
                            "kernelCriticalRadiusMm": critical,
                            "radiusMm": radius,
                            "excursionMm": excursion,
                            "excursionMicron": excursion.map(|value| value * 1000.0),
                            "class": excursion.map(shortfall_class),
                            "materialSheetClearanceMm": sheet_material_clearance(
                                &piece_by_id, &placements, settings, *index),
                        }));
                    }
                    let mut attributed = Vec::new();
                    for (first, second) in &pair_split.kernel_admits_miter_refuses {
                        let critical = kernel_census
                            .pairs
                            .iter()
                            .find(|row| (row.first_index, row.second_index) == (*first, *second))
                            .and_then(|row| row.critical_two_r_mm);
                        let shortfall = critical.map(|value| value - two_r);
                        if released {
                            if let Some(value) = shortfall {
                                released_pair_shortfalls.push(value);
                            }
                        }
                        attributed.push(json!({
                            "placementIndices": [first, second],
                            "kernelCriticalTwoRMm": critical,
                            "twoRMm": two_r,
                            "excursionMm": shortfall,
                            "excursionMicron": shortfall.map(|value| value * 1000.0),
                            "class": shortfall.map(shortfall_class),
                            "miterEnvelopeIntersectionAreaMm2":
                                miter_pair_intersection_area_mm2(
                                    &pieces, &placements, settings, *first, *second).ok(),
                            "materialClearanceMm": pair_material_distance(
                                &piece_by_id, &placements, *first, *second),
                        }));
                    }
                    // The other direction, attributed with the same three
                    // quantities. A row here is the promotion rule's KILL
                    // clause — the disc refusing a pair HEAD's authority
                    // admits — so it gets numbers rather than a count, and the
                    // number that decides whether it is a soundness failure or
                    // a radius artefact is the material clearance: a pair whose
                    // untouched source rings are closer than `2r` is one the
                    // disc is *right* to refuse.
                    let mut p0_attributed = Vec::new();
                    for (first, second) in &pair_split.miter_admits_kernel_refuses {
                        let critical = kernel_census
                            .pairs
                            .iter()
                            .find(|row| (row.first_index, row.second_index) == (*first, *second))
                            .and_then(|row| row.critical_two_r_mm);
                        p0_attributed.push(json!({
                            "placementIndices": [first, second],
                            "kernelCriticalTwoRMm": critical,
                            "twoRMm": two_r,
                            "shortfallMm": critical.map(|value| value - two_r),
                            "shortfallMicron": critical.map(|value| (value - two_r) * 1000.0),
                            "miterEnvelopeIntersectionAreaMm2":
                                miter_pair_intersection_area_mm2(
                                    &pieces, &placements, settings, *first, *second).ok(),
                            "materialClearanceMm": pair_material_distance(
                                &piece_by_id, &placements, *first, *second),
                        }));
                    }
                    // Only now, after every comparison the caller wanted has
                    // been taken over the full scan.
                    trim(&mut kernel_census, plan.report_top);
                    trim(&mut miter_scan, plan.report_top);
                    census_json = json!({
                        "allowanceMm": allowance,
                        "expansionMm": kernel_census.expansion_mm,
                        "kernelCertified": kernel_census.certified,
                        "pairCount": kernel_census.pair_count,
                        "kernelPairFailures": kernel_census.pair_failure_count,
                        "miterPairFailures": miter_scan.pair_failure_count,
                        "kernelBoundaryFailures": kernel_census.boundary_failure_count,
                        "miterBoundaryFailures": miter_scan.boundary_failure_count,
                        "bothAdmit": pair_split.both_admit,
                        "bothRefuse": pair_split.both_refuse,
                        "kernelAdmitsMiterRefuses":
                            pair_split.kernel_admits_miter_refuses.len(),
                        "miterAdmitsKernelRefuses":
                            pair_split.miter_admits_kernel_refuses.len(),
                        "miterAdmitsKernelRefusesRows":
                            pair_split.miter_admits_kernel_refuses,
                        "miterAdmitsKernelRefusesAttributed": p0_attributed,
                        "kernelAdmitsMiterRefusesAttributed": attributed,
                        "boundaries": {
                            "bothAdmit": boundary_split.both_admit,
                            "bothRefuse": boundary_split.both_refuse,
                            "kernelAdmitsMiterRefuses":
                                boundary_split.kernel_admits_miter_refuses.len(),
                            "miterAdmitsKernelRefuses":
                                boundary_split.miter_admits_kernel_refuses.len(),
                            "miterAdmitsKernelRefusesRows":
                                boundary_split.miter_admits_kernel_refuses,
                            "kernelAdmitsMiterRefusesAttributed":
                                boundary_attributed,
                        },
                    });
                }
                allowance_rows.push(json!({
                    "allowanceMm": allowance,
                    "expansionMm": authority.expansion_mm,
                    "contractAccepts": contract,
                    "contractMessage": authority.contract_only.clone().err(),
                    "miterVerdict": verdict_json(&wired.miter),
                    "kernelVerdict": verdict_json(&wired.exclusive),
                    "unionVerdict": verdict_json(&wired.union),
                    "gateAComposite": verdict_json(&authority.composite.map(|metrics| (
                        metrics.used_short_axis_span_mm,
                        metrics.used_long_axis_depth_mm))
                        .map_err(|error| error.to_string())),
                    "released": released,
                    "kernelRefusesMiterAccepts": p0,
                    "rawSourceDepthMm": authority.raw_source_depth_mm,
                    "gainOverIncumbentMm": gain,
                    "wouldHavePublished": gain > 0.0,
                }));
                if released {
                    released_rows.push(json!({
                        "cell": cell.label,
                        "seq": line.seq,
                        "step": line.step,
                        "allowanceMm": allowance,
                        "fingerprint": line.fingerprint,
                        "frontierDepthMm": line.frontier_depth_mm,
                        "floorDepthMm": line.floor_depth_mm,
                        "incumbentPublishedDepthMm": line.published_depth_mm,
                        "rawSourceDepthMm": authority.raw_source_depth_mm,
                        "gainOverIncumbentMm": gain,
                        "wouldHavePublished": gain > 0.0,
                        "proxyCollisionPairs": line.collision_pairs,
                        "proxyBoundaryViolations": line.boundary_violations,
                        "miterMessage": wired.miter.clone().err(),
                    }));
                }
                if p0 {
                    p0_rows.push(json!({
                        "cell": cell.label,
                        "seq": line.seq,
                        "step": line.step,
                        "allowanceMm": allowance,
                        "fingerprint": line.fingerprint,
                    }));
                }
            }
            records.push(json!({
                "seq": line.seq,
                "step": line.step,
                "workUnits": line.work_units,
                "fingerprint": line.fingerprint,
                "frontierDepthMm": line.frontier_depth_mm,
                "floorDepthMm": line.floor_depth_mm,
                "incumbentPublishedDepthMm": line.published_depth_mm,
                "parentDepthMm": line.parent_depth_mm,
                "proxyCollisionPairs": line.collision_pairs,
                "proxyBoundaryViolations": line.boundary_violations,
                "proxyBoundaryLoss": line.boundary_loss,
                "placementCount": line.placements.len(),
                "allowances": allowance_rows,
                "census": census_json,
            }));
        }
        let distinct = lines
            .iter()
            .map(|line| line.fingerprint.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        cell_rows.push(json!({
            "label": cell.label,
            "path": cell.path,
            "dumpedRecords": lines.len(),
            "distinctFingerprints": distinct,
            "skipsSuppressed": cell.skips_suppressed,
            "distinctExpected": cell.distinct_expected,
            "dumpMatchesSinkTally": cell
                .distinct_expected
                .map(|value| value == lines.len() && distinct == lines.len()),
            "duplicateSkips": match (cell.skips_suppressed, cell.distinct_expected) {
                (Some(skips), Some(distinct_expected)) => {
                    json!(skips.saturating_sub(distinct_expected))
                }
                _ => serde_json::Value::Null,
            },
            "sampledRecords": records.len(),
            "censusRecords": census_positions.len(),
            "releasedAtCensusAllowance": cell_released,
            "allThreeRefuseAtCensusAllowance": cell_all_refuse,
            "records": records,
        }));
    }

    let describe = |values: &[f64]| -> serde_json::Value {
        let mut ordered = values.to_vec();
        ordered.sort_by(f64::total_cmp);
        let median = if ordered.is_empty() {
            serde_json::Value::Null
        } else {
            let middle = ordered.len() / 2;
            json!(if ordered.len() % 2 == 1 {
                ordered[middle]
            } else {
                (ordered[middle - 1] + ordered[middle]) / 2.0
            })
        };
        json!({
            "count": ordered.len(),
            "median": median,
            "min": ordered.first(),
            "max": ordered.last(),
        })
    };
    let joint_rows = joint
        .iter()
        .map(|((allowance, contract, miter, kernel), count)| {
            json!({
                "allowanceMm": allowance,
                "contractAccepts": contract,
                "miterAccepts": miter,
                "kernelAccepts": kernel,
                "records": count,
            })
        })
        .collect::<Vec<_>>();
    let sampled_total = cell_rows
        .iter()
        .filter_map(|row| row["sampledRecords"].as_u64())
        .sum::<u64>();
    let dumped_total = cell_rows
        .iter()
        .filter_map(|row| row["dumpedRecords"].as_u64())
        .sum::<u64>();

    let document = json!({
        "plan": plan_path,
        "planSha256": plan_sha256,
        "request": plan.request,
        "requestSha256": request_sha256,
        "pairClearanceMm": plan.pair_clearance_mm,
        "edgeClearanceMm": plan.edge_clearance_mm,
        "allowancesMm": plan.allowances_mm,
        "censusAllowanceMm": plan.census_allowance_mm,
        "pieceCount": pieces.len(),
        "dumpedRecordsTotal": dumped_total,
        "sampledRecordsTotal": sampled_total,
        "jointDistribution": joint_rows,
        "releasedLayouts": released_rows,
        "releasedLayoutCount": released_rows.len(),
        "kernelRefusesMiterAcceptsLayouts": p0_rows,
        "kernelRefusesMiterAcceptsCount": p0_rows.len(),
        "releasedPairExcursionMm": describe(&released_pair_shortfalls),
        "releasedBoundaryExcursionMm": describe(&released_boundary_excursions),
        "compositeReadingsAgree": composite_readings_agree,
        "cells": cell_rows,
    });
    println!("{}", serde_json::to_string_pretty(&document)?);
    Ok(())
}
