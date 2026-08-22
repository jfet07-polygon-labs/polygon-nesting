//! The certified round-envelope kernel's soundness battery.
//!
//! ```text
//! round_envelope_battery PLAN.json
//! ```
//!
//! One JSON document on stdout. Nothing is written in place. The plan names the
//! request, the corpora and the sweep sizes; every number below is measured in
//! this process against the engine's own functions.
//!
//! The four sections are Sol review 11 item 1 as refined by Sol review 12 §3.2,
//! in its own order:
//!
//! 1. **canonical corpus** — every currently canonical-valid layout must stay
//!    valid under the kernel. A miter-accepted layout the kernel refuses is a
//!    P0 and is reported as one.
//! 2. **material-valid / canonical-invalid proposals** — the population where a
//!    false accept can happen at all. Constructed by walking a pair of a pinned
//!    parent across the miter threshold in 1 µm steps, because the contact-block
//!    round's own refused outputs were not committed with their placements
//!    (its evidence carries per-round statistics, not pose sets).
//! 3. **the Sparrow differential** — Gate A's pre-committed expectations, taken
//!    as a pass/fail rather than as a measurement.
//! 4. **the ±1 µm sweeps** — the kernel's flip point against the exact material
//!    distance, and its monotonicity.
//!
//! The request loader is `examples/sparrow_import_gate.rs`'s, unchanged in
//! substance: the same `polygon_set_from_imported_piece`, the same
//! `GeneralFastSettings::deterministic_test` base, the same
//! `sheet.width >= sheet.height` axis-normalisation rule.

use std::collections::BTreeMap;
use std::env;
use std::fs;

use polygon_nesting_core::domain::ImportedPiece;
use polygon_nesting_core::geometry::general_polygon::PolygonSet;
use polygon_nesting_core::geometry::general_source::polygon_set_from_imported_piece;
use polygon_nesting_core::search::general_fast::{
    construct_short_side_first, GeneralFastPiece, GeneralFastPlacement, GeneralFastSettings,
};
use polygon_nesting_core::search::import_gate::authority_verdict;
use polygon_nesting_core::search::round_envelope_gate::{
    census, composite_nanoseconds, envelope_half_nanoseconds, miter_census,
    miter_pair_intersection_area_mm2, trim, wired_verdicts, ArmedKernel, Census,
};
use polygon_nesting_core::validation::general_polygon::{
    material_pair_distance_mm, material_sheet_clearance_mm, GeneralPlacement,
};
use polygon_nesting_core::validation::round_envelope::{
    critical_two_r_micron, pair_admissible, GridSet, KernelMode,
};
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
    /// The allowances the corpus section runs at. 0.002 is the from-request
    /// default and HEAD's shipping authority; 0.0005 is the record lineage the
    /// g2-g4 gates were measured on; 0.0 is the contract radius itself. The
    /// first is the one the sweeps and the economy use.
    corpus_allowances_mm: Vec<f64>,
    canonical_corpus: Vec<CorpusEntry>,
    sparrow: SparrowPlan,
    sweeps: SweepPlan,
    economy: EconomyPlan,
    report_top: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CorpusEntry {
    label: String,
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SparrowPlan {
    path: String,
    allowances_mm: Vec<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SweepPlan {
    /// Which corpus entries the near-critical pairs are drawn from.
    sources: Vec<String>,
    /// How many of the tightest pairs to take from each source.
    pairs_per_source: usize,
    /// How many of the tightest boundaries to take from each source.
    boundaries_per_source: usize,
    /// Steps each way from the starting pose, in whole micrometres.
    steps_each_way: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EconomyPlan {
    sources: Vec<String>,
    repetitions: usize,
}

// ------------------------------------------------------- the request loader

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PoseFixture {
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

fn default_true() -> bool {
    true
}

struct OwnedPiece {
    id: String,
    polygon: PolygonSet,
    allow_rotation: bool,
    allow_mirror: bool,
}

fn load_placements(path: &str) -> Result<Vec<GeneralFastPlacement>, Box<dyn std::error::Error>> {
    let fixture: PoseFixture = serde_json::from_slice(&fs::read(path)?)?;
    Ok(fixture
        .placements
        .iter()
        .map(|pose| GeneralFastPlacement {
            piece_id: pose.piece_id.clone(),
            rotation_deg: pose.rotation_deg,
            mirrored: pose.mirrored,
            translate_short_axis: pose.translate_short_axis,
            translate_long_axis: pose.translate_long_axis,
        })
        .collect())
}

// -------------------------------------------------------------- reporting

fn census_json(census: &Census) -> serde_json::Value {
    json!({
        "label": census.label,
        "envelope": census.envelope,
        "expansionMm": census.expansion_mm,
        "radiusMicron": census.radius_micron,
        "sheetInsetMm": census.sheet_inset_mm,
        "insetBoxMicron": census.inset_box_micron,
        "certified": census.certified,
        "admissible": census.admissible,
        "pairCount": census.pair_count,
        "pairFailureCount": census.pair_failure_count,
        "boundaryFailureCount": census.boundary_failure_count,
        "boxCertifiedPairs": census.box_certified_pairs,
        "narrowSegmentPairs": census.narrow_segment_pairs,
        "envelopeVertexTotal": census.envelope_vertex_total,
        "pairs": census.pairs.iter().map(|row| json!({
            "placementIndices": [row.first_index, row.second_index],
            "pieceIds": [row.first_piece_id, row.second_piece_id],
            "admissible": row.admissible,
            "criticalTwoRMm": row.critical_two_r_mm,
            "criticalSaturated": row.critical_saturated,
            "certifiedByBox": row.certified_by_box,
            "narrowSegmentPairs": row.narrow_segment_pairs,
        })).collect::<Vec<_>>(),
        "boundaries": census.boundaries.iter().map(|row| json!({
            "placementIndex": row.index,
            "pieceId": row.piece_id,
            "admissible": row.admissible,
            "criticalRadiusMm": row.critical_radius_mm,
        })).collect::<Vec<_>>(),
    })
}

/// The per-row comparison between the two authorities, over the **full** scan.
struct Disagreement {
    both_admit: usize,
    both_refuse: usize,
    /// The kernel admits and the miter refuses. This is the join's price and is
    /// the expected direction.
    kernel_admits_miter_refuses: Vec<(usize, usize)>,
    /// The miter admits and the kernel refuses. **P0.**
    miter_admits_kernel_refuses: Vec<(usize, usize)>,
}

fn compare(kernel: &Census, miter: &Census) -> (Disagreement, Disagreement) {
    let mut pairs = Disagreement {
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
            (true, true) => pairs.both_admit += 1,
            (false, false) => pairs.both_refuse += 1,
            (true, false) => pairs.kernel_admits_miter_refuses.push(key),
            (false, true) => pairs.miter_admits_kernel_refuses.push(key),
        }
    }
    let mut boundaries = Disagreement {
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
            (true, true) => boundaries.both_admit += 1,
            (false, false) => boundaries.both_refuse += 1,
            (true, false) => boundaries
                .kernel_admits_miter_refuses
                .push((row.index, row.index)),
            (false, true) => boundaries
                .miter_admits_kernel_refuses
                .push((row.index, row.index)),
        }
    }
    (pairs, boundaries)
}

fn disagreement_json(value: &Disagreement) -> serde_json::Value {
    json!({
        "bothAdmit": value.both_admit,
        "bothRefuse": value.both_refuse,
        "kernelAdmitsMiterRefuses": value.kernel_admits_miter_refuses.len(),
        "kernelAdmitsMiterRefusesRows": value.kernel_admits_miter_refuses,
        "miterAdmitsKernelRefuses": value.miter_admits_kernel_refuses.len(),
        "miterAdmitsKernelRefusesRows": value.miter_admits_kernel_refuses,
    })
}

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

/// The canonicalization budget, in millimetres.
///
/// Every source vertex is snapped to the nearest micrometre, which moves it by
/// at most `sqrt(2)/2` µm; interior points of an edge are convex combinations of
/// its endpoints and so move by at most the same. Two rings, so the distance
/// between the canonical rings differs from the distance between the source
/// rings by at most `sqrt(2)` µm.
///
/// It is the tolerance a *false accept* has to be judged against: the kernel
/// measures canonical rings, the material clearance below is measured on
/// untouched `f64` source rings, and the miter authority is on the same
/// canonical rings the kernel is. A kernel accept whose source-ring clearance
/// is below `2r` by more than this is a false accept; inside it, the gap
/// belongs to the canonical grid, which both authorities share and neither
/// invented.
const CANONICALIZATION_BUDGET_MM: f64 = 0.0014143;

/// The budget for comparing the kernel's own **bisected** critical clearance
/// against the source-ring measurement, in millimetres.
///
/// [`CANONICALIZATION_BUDGET_MM`] plus one whole grid step, because
/// `critical_two_r_micron` returns the largest *integer* micrometre at which the
/// pair is still admissible — the floor of the canonical distance, not the
/// canonical distance. A comparison that used the smaller budget would report a
/// floor as a disagreement, which is what a first pass of this battery did on
/// 3 of 30 sweeps.
const SWEEP_FLOOR_BUDGET_MM: f64 = 0.0024143;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args().skip(1);
    let plan_path = arguments.next().ok_or("usage: round_envelope_battery PLAN.json")?;
    if arguments.next().is_some() {
        return Err("no extra arguments are accepted".into());
    }
    let plan_bytes = fs::read(&plan_path)?;
    let plan_sha256 = format!("{:x}", Sha256::digest(&plan_bytes));
    let plan: Plan = serde_json::from_slice(&plan_bytes)?;

    let request_bytes = fs::read(&plan.request)?;
    let request_sha256 = format!("{:x}", Sha256::digest(&request_bytes));
    let request: Request = serde_json::from_slice(&request_bytes)?;
    let (_request_total_padding_mm, allow_global_rotation, allow_global_mirror, geometry) =
        match (&request.settings, &request.options) {
            (Some(settings), None) => (
                settings.padding,
                settings.allow_global_rotation,
                settings.allow_global_mirror,
                settings.geometry,
            ),
            (None, Some(options)) => (
                request.padding.ok_or("legacy requests require top-level padding")?,
                options.allow_global_rotation,
                options.allow_global_mirror,
                options.irregular_settings.geometry,
            ),
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
    let corpus_settings = {
        let mut settings = base_settings;
        settings.search_offset_allowance_mm = *plan
            .corpus_allowances_mm
            .first()
            .ok_or("the plan names no corpus allowance")?;
        settings
    };

    // ------------------------------------------------- 1. canonical corpus
    let mut corpus_rows = Vec::new();
    let mut corpus_layouts: BTreeMap<String, Vec<GeneralFastPlacement>> = BTreeMap::new();
    let mut p0_layouts = Vec::new();
    let mut p0_rows_total = 0usize;
    let mut fresh = vec![(
        "constructor-fresh".to_owned(),
        construct_short_side_first(&pieces, corpus_settings)?.placements,
    )];
    for entry in &plan.canonical_corpus {
        fresh.push((entry.label.clone(), load_placements(&entry.path)?));
    }
    let mut total_pair_rows = 0usize;
    let mut total_boundary_rows = 0usize;
    for (label, placements) in &fresh {
        corpus_layouts.insert(label.clone(), placements.clone());
        for allowance in &plan.corpus_allowances_mm {
            let mut settings = base_settings;
            settings.search_offset_allowance_mm = *allowance;
            let wired = wired_verdicts(&pieces, placements, settings);
            let (miter_verdict, kernel_verdict) =
                (wired.miter.clone(), wired.exclusive.clone());
            let mut kernel = census(label, &pieces, placements, settings, usize::MAX)
                .map_err(|error| -> Box<dyn std::error::Error> { error.into() })?;
            let mut miter = miter_census(label, &pieces, placements, settings, usize::MAX)
                .map_err(|error| -> Box<dyn std::error::Error> { error.into() })?;
            let (pair_diff, boundary_diff) = compare(&kernel, &miter);
            total_pair_rows += kernel.pair_count;
            total_boundary_rows += placements.len();
            // A layout the miter authority accepts whole must stay accepted
            // whole. A layout the miter authority already refuses cannot be a
            // regression, whatever the kernel says about it.
            let layout_p0 = miter_verdict.is_ok() && kernel_verdict.is_err();
            let union_p0 = miter_verdict.is_ok() && wired.union.is_err();
            let p0 = !pair_diff.miter_admits_kernel_refuses.is_empty()
                || !boundary_diff.miter_admits_kernel_refuses.is_empty();
            // Every row the miter admits and the kernel refuses, with the two
            // quantities that attribute it: the kernel's exact critical
            // clearance and the miter envelopes' own intersection area.
            let two_r = 2.0 * kernel.expansion_mm;
            let mut attributed = Vec::new();
            for (first_index, second_index) in &pair_diff.miter_admits_kernel_refuses {
                let critical = kernel
                    .pairs
                    .iter()
                    .find(|row| (row.first_index, row.second_index)
                        == (*first_index, *second_index))
                    .and_then(|row| row.critical_two_r_mm);
                attributed.push(json!({
                    "placementIndices": [first_index, second_index],
                    "kernelCriticalTwoRMm": critical,
                    "shortfallMicron": critical.map(|value| (value - two_r) * 1000.0),
                    "miterEnvelopeIntersectionAreaMm2": miter_pair_intersection_area_mm2(
                        &pieces, placements, settings, *first_index, *second_index).ok(),
                    "materialClearanceMm": pair_material_distance(
                        &piece_by_id, placements, *first_index, *second_index),
                }));
            }
            if p0 {
                p0_layouts.push(format!("{label}@{allowance}"));
                p0_rows_total += pair_diff.miter_admits_kernel_refuses.len()
                    + boundary_diff.miter_admits_kernel_refuses.len();
            }
            trim(&mut kernel, plan.report_top);
            trim(&mut miter, plan.report_top);
            corpus_rows.push(json!({
                "label": label,
                "searchOffsetAllowanceMm": allowance,
                "placementCount": placements.len(),
                "compositeMiterVerdict": verdict_json(&miter_verdict),
                "compositeRoundVerdict": verdict_json(&kernel_verdict),
                "compositeUnionVerdict": verdict_json(&wired.union),
                "layoutP0": layout_p0,
                "layoutP0Union": union_p0,
                "rowP0": p0,
                "miterAdmitsKernelRefusesAttributed": attributed,
                "pairs": disagreement_json(&pair_diff),
                "boundaries": disagreement_json(&boundary_diff),
                "kernelCensus": census_json(&kernel),
                "miterCensus": census_json(&miter),
            }));
        }
    }

    // ------------------- 2 and 4. the pair walk: proposals and the sweeps
    let step_mm = 0.001;
    let mut sweep_rows = Vec::new();
    let mut proposals = Vec::new();
    let mut false_accepts = Vec::new();
    let mut suspicious = Vec::new();
    for source in &plan.sweeps.sources {
        let Some(placements) = corpus_layouts.get(source) else {
            return Err(format!("sweep source {source} is not in the canonical corpus").into());
        };
        let full = census(source, &pieces, placements, corpus_settings, usize::MAX)
            .map_err(|error| -> Box<dyn std::error::Error> { error.into() })?;
        let mut tightest = full
            .pairs
            .iter()
            .filter(|row| row.admissible && !row.critical_saturated)
            .map(|row| {
                (
                    row.critical_two_r_mm.unwrap_or(f64::INFINITY),
                    row.first_index,
                    row.second_index,
                )
            })
            .collect::<Vec<_>>();
        tightest.sort_by(|first, second| {
            first
                .0
                .total_cmp(&second.0)
                .then_with(|| first.1.cmp(&second.1))
                .then_with(|| first.2.cmp(&second.2))
        });
        tightest.truncate(plan.sweeps.pairs_per_source);
        for (_, first_index, second_index) in tightest {
            let row = sweep_pair(
                source,
                &piece_by_id,
                &pieces,
                placements,
                corpus_settings,
                first_index,
                second_index,
                plan.sweeps.steps_each_way,
                step_mm,
                &mut proposals,
                &mut false_accepts,
                &mut suspicious,
            )?;
            sweep_rows.push(row);
        }

        // Boundary sweeps: the tightest placements against the inset sheet.
        let mut tight_boundaries = full
            .boundaries
            .iter()
            .filter(|row| row.admissible)
            .map(|row| (row.critical_radius_mm.unwrap_or(f64::INFINITY), row.index))
            .collect::<Vec<_>>();
        tight_boundaries.sort_by(|first, second| {
            first.0.total_cmp(&second.0).then_with(|| first.1.cmp(&second.1))
        });
        tight_boundaries.truncate(plan.sweeps.boundaries_per_source);
        for (_, index) in tight_boundaries {
            sweep_rows.push(sweep_boundary(
                source,
                &piece_by_id,
                &pieces,
                placements,
                corpus_settings,
                index,
                plan.sweeps.steps_each_way,
                step_mm,
                &mut proposals,
                &mut false_accepts,
                &mut suspicious,
            )?);
        }
    }

    // --------------------------------------------- 3. the Sparrow differential
    let sparrow_bytes = fs::read(&plan.sparrow.path)?;
    let sparrow_sha256 = format!("{:x}", Sha256::digest(&sparrow_bytes));
    let sparrow = load_placements(&plan.sparrow.path)?;
    let mut sparrow_rows = Vec::new();
    for allowance in &plan.sparrow.allowances_mm {
        let mut settings = base_settings;
        settings.search_offset_allowance_mm = *allowance;
        let wired = wired_verdicts(&pieces, &sparrow, settings);
        let (miter_verdict, kernel_verdict) = (wired.miter.clone(), wired.exclusive.clone());
        let contract_only = {
            let _disarmed = ArmedKernel::install(KernelMode::Off);
            authority_verdict(&pieces, &sparrow, settings)
                .map_err(|error| -> Box<dyn std::error::Error> { error.into() })?
                .contract_only
        };
        let mut kernel = census(
            format!("sparrow@{allowance}"),
            &pieces,
            &sparrow,
            settings,
            usize::MAX,
        )
        .map_err(|error| -> Box<dyn std::error::Error> { error.into() })?;
        let refused = kernel
            .pairs
            .iter()
            .filter(|row| !row.admissible)
            .map(|row| {
                json!({
                    "placementIndices": [row.first_index, row.second_index],
                    "pieceIds": [row.first_piece_id, row.second_piece_id],
                    "criticalTwoRMm": row.critical_two_r_mm,
                    "materialClearanceMm": pair_material_distance(
                        &piece_by_id, &sparrow, row.first_index, row.second_index),
                })
            })
            .collect::<Vec<_>>();
        // Gate A's pinned rows: pose indices [0, 1] (Sparrow items 38 and 39)
        // and [42, 44] (items 50 and 52), the only two whose refusal at the
        // from-request radius is caused by the radius rather than by the join.
        let pair_38_39 = kernel
            .pairs
            .iter()
            .find(|row| (row.first_index, row.second_index) == (0, 1))
            .map(|row| {
                json!({
                    "admissible": row.admissible,
                    "criticalTwoRMm": row.critical_two_r_mm,
                })
            });
        let pair_50_52 = kernel
            .pairs
            .iter()
            .find(|row| (row.first_index, row.second_index) == (42, 44))
            .map(|row| {
                json!({
                    "admissible": row.admissible,
                    "criticalTwoRMm": row.critical_two_r_mm,
                })
            });
        let refused_indices = kernel
            .pairs
            .iter()
            .filter(|row| !row.admissible)
            .map(|row| (row.first_index, row.second_index))
            .collect::<Vec<_>>();
        trim(&mut kernel, plan.report_top);
        sparrow_rows.push(json!({
            "searchOffsetAllowanceMm": allowance,
            "expansionMm": kernel.expansion_mm,
            "radiusMicron": kernel.radius_micron,
            "contractOnlyAccepts": contract_only.is_ok(),
            "contractOnlyMessage": contract_only.err(),
            "compositeMiterVerdict": verdict_json(&miter_verdict),
            "compositeRoundVerdict": verdict_json(&kernel_verdict),
            "compositeUnionVerdict": verdict_json(&wired.union),
            "kernelPairFailureCount": kernel.pair_failure_count,
            "kernelBoundaryFailureCount": kernel.boundary_failure_count,
            "kernelRefusedPairIndices": refused_indices,
            "kernelRefusedPairs": refused,
            "pair38x39": pair_38_39,
            "pair50x52": pair_50_52,
            "kernelCensus": census_json(&kernel),
        }));
    }

    // ---------------------------------------------------------- the economy
    let mut economy_rows = Vec::new();
    for source in &plan.economy.sources {
        let Some(placements) = corpus_layouts.get(source) else {
            return Err(format!("economy source {source} is not in the canonical corpus").into());
        };
        let (miter_half, round_half) = envelope_half_nanoseconds(
            &pieces,
            placements,
            corpus_settings,
            plan.economy.repetitions,
        )
        .map_err(|error| -> Box<dyn std::error::Error> { error.into() })?;
        let timing = composite_nanoseconds(
            &pieces,
            placements,
            corpus_settings,
            plan.economy.repetitions,
        );
        let (miter_all, round_all) = (timing.miter_nanoseconds, timing.exclusive_nanoseconds);
        let (miter_ok, round_ok) = (timing.miter_accepts, timing.exclusive_accepts);
        economy_rows.push(json!({
            "label": source,
            "repetitions": plan.economy.repetitions,
            "envelopeHalfMiterMs": miter_half / 1.0e6,
            "envelopeHalfRoundMs": round_half / 1.0e6,
            "envelopeHalfRatio": round_half / miter_half,
            "confirmationMiterMs": miter_all / 1.0e6,
            "confirmationRoundMs": round_all / 1.0e6,
            "confirmationRatio": round_all / miter_all,
            "confirmationUnionMs": timing.union_nanoseconds / 1.0e6,
            "confirmationUnionRatio": timing.union_nanoseconds / miter_all,
            "miterAccepts": miter_ok,
            "roundAccepts": round_ok,
            "unionAccepts": timing.union_accepts,
            // A confirmation that *refuses* short-circuits, and the two
            // authorities short-circuit at different places, so only a cell
            // both accept prices the same amount of work twice. The envelope
            // half above is a full scan on both arms and is comparable on every
            // cell.
            "confirmationComparable": miter_ok && round_ok,
            "confirmationUnionComparable": miter_ok && timing.union_accepts,
        }));
    }

    let median_of = |mut values: Vec<f64>| -> Option<f64> {
        values.sort_by(f64::total_cmp);
        values.get(values.len() / 2).copied()
    };
    let envelope_half_ratio_median = median_of(
        economy_rows
            .iter()
            .filter_map(|row| row["envelopeHalfRatio"].as_f64())
            .collect(),
    );
    let confirmation_ratio_median = median_of(
        economy_rows
            .iter()
            .filter(|row| row["confirmationComparable"] == json!(true))
            .filter_map(|row| row["confirmationRatio"].as_f64())
            .collect(),
    );
    let confirmation_union_ratio_median = median_of(
        economy_rows
            .iter()
            .filter(|row| row["confirmationUnionComparable"] == json!(true))
            .filter_map(|row| row["confirmationUnionRatio"].as_f64())
            .collect(),
    );

    let executable_sha256 = env::current_exe()
        .ok()
        .and_then(|path| fs::read(path).ok())
        .map(|bytes| format!("{:x}", Sha256::digest(&bytes)));

    let document = json!({
        "experiment": "round-envelope-kernel",
        "instrument": "crates/polygon-nesting-core/src/validation/round_envelope.rs",
        "censusInstrument": "crates/polygon-nesting-core/src/search/round_envelope_gate.rs",
        "executableSha256": executable_sha256,
        "plan": {"path": plan_path, "sha256": plan_sha256},
        "request": {"path": plan.request, "sha256": request_sha256,
                    "sheetShortAxisMm": base_settings.sheet_short_axis_mm,
                    "sheetLongAxisMm": base_settings.sheet_long_axis_mm,
                    "normalizeAxes": normalize_axes},
        "contract": {
            "pairClearanceMm": base_settings.total_padding_mm,
            "sheetEdgeClearanceMm": plan.edge_clearance_mm,
            "clearanceSafetyMarginMm": base_settings.clearance_safety_margin_mm,
            "flatteningSagToleranceMm": base_settings.flattening_sag_tolerance_mm,
            "sweepAndEconomyAllowanceMm": corpus_settings.search_offset_allowance_mm,
        },
        "canonicalGridStepMm": 0.001,
        "canonicalizationBudgetMm": CANONICALIZATION_BUDGET_MM,
        "population1CanonicalCorpus": {
            "allowancesMm": plan.corpus_allowances_mm,
            "layoutCount": fresh.len(),
            "cellCount": corpus_rows.len(),
            "pairRowsCompared": total_pair_rows,
            "boundaryRowsCompared": total_boundary_rows,
            "p0Layouts": p0_layouts,
            "p0RowCount": p0_rows_total,
            "rows": corpus_rows,
        },
        "population2MaterialValidCanonicalInvalid": {
            "note": "constructed by walking a near-critical pair (or a placement \
                     against the sheet edge) across the miter threshold in 1 µm \
                     steps; the contact-block round's own refused outputs were \
                     not committed with their placements",
            "proposalCount": proposals.len(),
            "kernelAcceptCount": proposals.iter()
                .filter(|row| row["kernelAccepts"] == json!(true)).count(),
            "falseAcceptCount": false_accepts.len(),
            "falseAccepts": false_accepts,
            "insideCanonicalizationBudgetCount": suspicious.len(),
            "insideCanonicalizationBudget": suspicious,
            "proposals": proposals,
        },
        "population3SparrowDifferential": {
            "poses": {"path": plan.sparrow.path, "sha256": sparrow_sha256,
                      "placementCount": sparrow.len()},
            "rows": sparrow_rows,
        },
        "population4Sweeps": {
            "stepMm": step_mm,
            "stepsEachWay": plan.sweeps.steps_each_way,
            "sweepCount": sweep_rows.len(),
            "rows": sweep_rows,
        },
        "economy": {
            "note": "envelopeHalf* is a full scan on both arms and is comparable \
                     on every cell; confirmation* is only comparable where \
                     confirmationComparable is true, because a refusing \
                     confirmation short-circuits",
            "envelopeHalfRatioMedian": envelope_half_ratio_median,
            "confirmationRatioMedianComparable": confirmation_ratio_median,
            "confirmationUnionRatioMedianComparable": confirmation_union_ratio_median,
            "rows": economy_rows,
        },
    });
    println!("{}", serde_json::to_string_pretty(&document)?);
    Ok(())
}

fn pair_material_distance(
    piece_by_id: &BTreeMap<String, GeneralFastPiece<'_>>,
    placements: &[GeneralFastPlacement],
    first: usize,
    second: usize,
) -> Option<f64> {
    let build = |index: usize| -> Option<GeneralPlacement<'_>> {
        let placement = placements.get(index)?;
        let piece = piece_by_id.get(placement.piece_id.as_str())?;
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

/// One near-critical pair, walked across its threshold in whole micrometres.
///
/// The sub-layout is the two placements alone, which is what makes the walk a
/// *pair* measurement: with the other 59 pieces present, a step that tightens
/// this pair can loosen or tighten another and the layout verdict stops being
/// attributable. The two poses are the parent's own, unmodified but for the
/// walked translation.
#[allow(clippy::too_many_arguments)]
fn sweep_pair(
    source: &str,
    piece_by_id: &BTreeMap<String, GeneralFastPiece<'_>>,
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
    first_index: usize,
    second_index: usize,
    steps_each_way: i64,
    step_mm: f64,
    proposals: &mut Vec<serde_json::Value>,
    false_accepts: &mut Vec<serde_json::Value>,
    suspicious: &mut Vec<serde_json::Value>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let anchor = placements[first_index].clone();
    let mover = placements[second_index].clone();
    let anchor_piece = *piece_by_id
        .get(anchor.piece_id.as_str())
        .ok_or("unknown anchor piece")?;
    let mover_piece = *piece_by_id
        .get(mover.piece_id.as_str())
        .ok_or("unknown mover piece")?;
    // The walk direction: from the mover's canonical box centre toward the
    // anchor's, normalised. A step is one micrometre along it.
    let centre = |piece: &GeneralFastPiece<'_>, placement: &GeneralFastPlacement| {
        let polygon = piece
            .polygon
            .transformed(
                placement.rotation_deg,
                placement.mirrored,
                placement.translate_short_axis,
                placement.translate_long_axis,
            )
            .ok()?;
        let set = GridSet::of(&polygon)?;
        let (min_x, min_y, max_x, max_y) = set.bounds_micron();
        Some((
            (min_x + max_x) as f64 / 2.0,
            (min_y + max_y) as f64 / 2.0,
        ))
    };
    let from = centre(&mover_piece, &mover).ok_or("mover is outside the kernel domain")?;
    let to = centre(&anchor_piece, &anchor).ok_or("anchor is outside the kernel domain")?;
    let (mut dx, mut dy) = (to.0 - from.0, to.1 - from.1);
    let length = (dx * dx + dy * dy).sqrt();
    if length == 0.0 {
        return Err("the two placements share a box centre".into());
    }
    dx /= length;
    dy /= length;

    let two_r = 2.0 * (settings.total_padding_mm / 2.0
        + settings.clearance_safety_margin_mm
        + settings.search_offset_allowance_mm);
    let two_r_micron = (two_r * 1000.0).round() as i64;
    // Where the flip actually is, found before the window is chosen.
    //
    // A window centred on the parent's own pose only straddles the threshold if
    // the pair happened to start within `steps_each_way` micrometres of it, and
    // a sweep that never crosses proves nothing about the flip point. So the
    // crossing is located first — doubling then bisecting on the kernel's own
    // pair predicate, which is exact and cheap — and the recorded window is
    // centred on it. The predicate is monotone in the walk for the same reason
    // it is monotone in the radius, and the recorded steps re-measure it either
    // way, so a mis-located centre would show up as a window with no flip in it
    // rather than as a wrong answer.
    let pair_ok = |k: i64| -> bool {
        let mut moved = mover.clone();
        moved.translate_short_axis = mover.translate_short_axis + (k as f64) * step_mm * dx;
        moved.translate_long_axis = mover.translate_long_axis + (k as f64) * step_mm * dy;
        let first = anchor_piece.polygon.transformed(
            anchor.rotation_deg,
            anchor.mirrored,
            anchor.translate_short_axis,
            anchor.translate_long_axis,
        );
        let second = mover_piece.polygon.transformed(
            moved.rotation_deg,
            moved.mirrored,
            moved.translate_short_axis,
            moved.translate_long_axis,
        );
        match (first, second) {
            (Ok(first), Ok(second)) => match (GridSet::of(&first), GridSet::of(&second)) {
                (Some(first), Some(second)) => pair_admissible(&first, &second, two_r_micron),
                _ => false,
            },
            _ => false,
        }
    };
    let centre_step = locate_flip(&pair_ok, 1 << 20);
    let mut steps = Vec::new();
    for k in (centre_step - steps_each_way)..=(centre_step + steps_each_way) {
        let mut moved = mover.clone();
        moved.translate_short_axis = mover.translate_short_axis + (k as f64) * step_mm * dx;
        moved.translate_long_axis = mover.translate_long_axis + (k as f64) * step_mm * dy;
        let two = vec![anchor.clone(), moved];
        let wired = wired_verdicts(pieces, &two, settings);
        let (miter_verdict, kernel_verdict) = (&wired.miter, &wired.exclusive);
        let contract = {
            let _disarmed = ArmedKernel::install(KernelMode::Off);
            authority_verdict(pieces, &two, settings)
                .map_err(|error| -> Box<dyn std::error::Error> { error.into() })?
                .contract_only
        };
        let material = pair_material_distance(piece_by_id, &two, 0, 1);
        let kernel_critical = {
            let first = GridSet::of(&anchor_piece.polygon.transformed(
                two[0].rotation_deg,
                two[0].mirrored,
                two[0].translate_short_axis,
                two[0].translate_long_axis,
            )?);
            let second = GridSet::of(&mover_piece.polygon.transformed(
                two[1].rotation_deg,
                two[1].mirrored,
                two[1].translate_short_axis,
                two[1].translate_long_axis,
            )?);
            match (first, second) {
                (Some(first), Some(second)) => {
                    critical_two_r_micron(&first, &second, 8 * 1_000_000)
                        .map(|(value, saturated)| (value as f64 / 1000.0, saturated))
                }
                _ => None,
            }
        };
        // The *pair* question alone, separated from the boundary question the
        // whole-layout verdict also answers. Walking a placement "away" from
        // its neighbour along the box-centre direction can walk it toward a
        // sheet edge, and then the layout verdict flips for a reason that has
        // nothing to do with the pair - which is what a first pass of this
        // battery read as a non-monotone kernel on 2 of 30 sweeps. The summary
        // is taken on this field; the layout verdicts above stay, because a
        // *proposal* is a whole layout and population 2 is classified on them.
        let pair_only = pair_ok(k);
        let miter_pair_only = miter_pair_intersection_area_mm2(pieces, &two, settings, 0, 1)
            .map(|area| area == 0.0)
            .ok();
        let row = json!({
            "source": source,
            "kind": "pair",
            "placementIndices": [first_index, second_index],
            "step": k,
            "contractAccepts": contract.is_ok(),
            "miterAccepts": miter_verdict.is_ok(),
            "kernelAccepts": kernel_verdict.is_ok(),
            "unionAccepts": wired.union.is_ok(),
            "kernelPrimaryAdmissible": pair_only,
            "miterPrimaryAdmissible": miter_pair_only,
            "materialClearanceMm": material,
            "kernelCriticalTwoRMm": kernel_critical.map(|(value, _)| value),
            "twoRMm": two_r,
        });
        record_proposal(&row, two_r, proposals, false_accepts, suspicious);
        steps.push(row);
    }
    Ok(summarise_sweep(
        source,
        "pair",
        json!([first_index, second_index]),
        two_r,
        centre_step,
        steps,
    ))
}

/// The smallest `k >= 0` at which `accepts` is false, by doubling then
/// bisecting; `0` when the predicate is already false there, and `0` when it
/// never turns false inside `cap`.
///
/// Only a *centring* device: whatever it returns, the recorded window
/// re-measures every step of the walk through the real authorities, so a bad
/// centre costs a sweep that does not straddle and never a wrong verdict.
fn locate_flip(accepts: &dyn Fn(i64) -> bool, cap: i64) -> i64 {
    if !accepts(0) {
        return 0;
    }
    let mut high = 1i64;
    while high <= cap && accepts(high) {
        high *= 2;
    }
    if high > cap {
        return 0;
    }
    let mut low = high / 2;
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if accepts(middle) {
            low = middle;
        } else {
            high = middle;
        }
    }
    high
}

/// One placement walked toward the low-long-axis sheet edge in whole
/// micrometres. The sub-layout is the placement alone.
#[allow(clippy::too_many_arguments)]
fn sweep_boundary(
    source: &str,
    piece_by_id: &BTreeMap<String, GeneralFastPiece<'_>>,
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
    index: usize,
    steps_each_way: i64,
    step_mm: f64,
    proposals: &mut Vec<serde_json::Value>,
    false_accepts: &mut Vec<serde_json::Value>,
    suspicious: &mut Vec<serde_json::Value>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let base = placements[index].clone();
    let piece = *piece_by_id
        .get(base.piece_id.as_str())
        .ok_or("unknown piece")?;
    let inset_plus_radius = (settings.sheet_edge_clearance_mm.unwrap_or(settings.total_padding_mm / 2.0)
        - settings.total_padding_mm / 2.0)
        + (settings.total_padding_mm / 2.0
            + settings.clearance_safety_margin_mm
            + settings.search_offset_allowance_mm);
    let radius_micron = ((settings.total_padding_mm / 2.0
        + settings.clearance_safety_margin_mm
        + settings.search_offset_allowance_mm)
        * 1000.0)
        .round() as i64;
    let inset_micron = ((settings.sheet_edge_clearance_mm
        .unwrap_or(settings.total_padding_mm / 2.0)
        - settings.total_padding_mm / 2.0)
        * 1000.0)
        .round() as i64;
    let boundary_ok = |k: i64| -> bool {
        let moved = base.translate_long_axis - (k as f64) * step_mm;
        let Ok(polygon) = piece.polygon.transformed(
            base.rotation_deg,
            base.mirrored,
            base.translate_short_axis,
            moved,
        ) else {
            return false;
        };
        let Some(set) = GridSet::of(&polygon) else {
            return false;
        };
        set.bounds_micron().1 - radius_micron >= inset_micron
    };
    let centre_step = locate_flip(&boundary_ok, 1 << 20);
    let mut steps = Vec::new();
    for k in (centre_step - steps_each_way)..=(centre_step + steps_each_way) {
        let mut moved = base.clone();
        // Negative k moves away from the low edge; positive k moves toward it.
        moved.translate_long_axis = base.translate_long_axis - (k as f64) * step_mm;
        let one = vec![moved];
        let wired = wired_verdicts(pieces, &one, settings);
        let (miter_verdict, kernel_verdict) = (&wired.miter, &wired.exclusive);
        let contract = {
            let _disarmed = ArmedKernel::install(KernelMode::Off);
            authority_verdict(pieces, &one, settings)
                .map_err(|error| -> Box<dyn std::error::Error> { error.into() })?
                .contract_only
        };
        let clearance = material_sheet_clearance_mm(
            &GeneralPlacement {
                piece_id: piece.id,
                polygon: piece.polygon,
                rotation_deg: one[0].rotation_deg,
                mirrored: one[0].mirrored,
                translate_x: one[0].translate_short_axis,
                translate_y: one[0].translate_long_axis,
            },
            settings.sheet_short_axis_mm,
            settings.sheet_long_axis_mm,
        )
        .ok();
        let binding = clearance.map(|values| values.iter().copied().fold(f64::INFINITY, f64::min));
        let row = json!({
            "source": source,
            "kind": "boundary",
            "placementIndices": [index],
            "step": k,
            "contractAccepts": contract.is_ok(),
            "miterAccepts": miter_verdict.is_ok(),
            "kernelAccepts": kernel_verdict.is_ok(),
            "unionAccepts": wired.union.is_ok(),
            // For a one-placement layout the boundary *is* the whole envelope
            // question, so the primary field is the layout verdict.
            "kernelPrimaryAdmissible": kernel_verdict.is_ok(),
            "miterPrimaryAdmissible": miter_verdict.is_ok(),
            "materialClearanceMm": binding,
            "kernelCriticalTwoRMm": serde_json::Value::Null,
            "twoRMm": inset_plus_radius,
        });
        record_proposal(&row, inset_plus_radius, proposals, false_accepts, suspicious);
        steps.push(row);
    }
    Ok(summarise_sweep(
        source,
        "boundary",
        json!([index]),
        inset_plus_radius,
        centre_step,
        steps,
    ))
}

/// Files one sweep step into population 2 when it is a material-valid,
/// canonically-invalid proposal — and into the false-accept list when the
/// kernel admits it and the untouched source-ring measurement says it should
/// not have been admitted.
fn record_proposal(
    row: &serde_json::Value,
    demanded_mm: f64,
    proposals: &mut Vec<serde_json::Value>,
    false_accepts: &mut Vec<serde_json::Value>,
    suspicious: &mut Vec<serde_json::Value>,
) {
    let contract = row["contractAccepts"] == json!(true);
    let miter = row["miterAccepts"] == json!(true);
    let kernel = row["kernelAccepts"] == json!(true);
    if !(contract && !miter) {
        return;
    }
    proposals.push(row.clone());
    if !kernel {
        return;
    }
    let Some(material) = row["materialClearanceMm"].as_f64() else {
        return;
    };
    if material < demanded_mm - CANONICALIZATION_BUDGET_MM {
        false_accepts.push(row.clone());
    } else if material < demanded_mm {
        suspicious.push(row.clone());
    }
}

fn summarise_sweep(
    source: &str,
    kind: &str,
    indices: serde_json::Value,
    demanded_mm: f64,
    centre_step: i64,
    steps: Vec<serde_json::Value>,
) -> serde_json::Value {
    let accepts = steps
        .iter()
        .map(|row| row["kernelPrimaryAdmissible"] == json!(true))
        .collect::<Vec<_>>();
    // Monotone *in the step index* means every accept precedes every refusal,
    // because the walk runs from "far" to "near". It is the weaker of the two
    // readings and it can fail for a reason that has nothing to do with the
    // kernel: the walk direction is the two placements' box-centre difference,
    // and for two interlocking concave pieces a step "away" along that
    // direction can bring a different part of the two boundaries together. When
    // that happens the *contract* and the *miter* flip on the same step, which
    // is what says it is the walk and not the authority.
    let first_refusal = accepts.iter().position(|value| !value);
    let monotone = match first_refusal {
        None => true,
        Some(position) => accepts[position..].iter().all(|value| !value),
    };
    // Monotone *in the material clearance* is the reading Sol's "no flip-flop"
    // actually asks for, and it is direction-independent: order the steps by
    // the untouched source-ring clearance and require every accept to sit above
    // every refusal.
    let mut ordered = steps
        .iter()
        .filter_map(|row| {
            Some((
                row["materialClearanceMm"].as_f64()?,
                row["kernelPrimaryAdmissible"] == json!(true),
            ))
        })
        .collect::<Vec<_>>();
    ordered.sort_by(|first, second| second.0.total_cmp(&first.0));
    let monotone_in_material = ordered
        .iter()
        .position(|(_, accepts)| !accepts)
        .is_none_or(|position| ordered[position..].iter().all(|(_, accepts)| !accepts));
    // And the sharpest form of "the flip point agrees with the exact material
    // distance to the grid step": every step outside the canonicalization band
    // must have the kernel verdict the source-ring measurement implies.
    let disagreeing_outside_budget = steps
        .iter()
        .filter(|row| {
            let Some(material) = row["materialClearanceMm"].as_f64() else {
                return false;
            };
            if (material - demanded_mm).abs() <= CANONICALIZATION_BUDGET_MM {
                return false;
            }
            (row["kernelPrimaryAdmissible"] == json!(true)) != (material >= demanded_mm)
        })
        .count();
    // The step at which the untouched source-ring measurement crosses the
    // demanded clearance. The kernel's own flip must land on it or one step
    // away, which is the grid step.
    let material_flip = steps.iter().position(|row| {
        row["materialClearanceMm"]
            .as_f64()
            .is_some_and(|value| value < demanded_mm)
    });
    let flip_delta = match (first_refusal, material_flip) {
        (Some(kernel), Some(material)) => Some(kernel as i64 - material as i64),
        _ => None,
    };
    let miter_flip = steps
        .iter()
        .position(|row| row["miterPrimaryAdmissible"] != json!(true));
    let contract_flip = steps
        .iter()
        .position(|row| row["contractAccepts"] != json!(true));
    // How far the kernel's own exact critical clearance sits from the
    // source-ring measurement, over the whole walk. Bounded by the
    // canonicalization budget when both are defined.
    let mut worst_gap: Option<f64> = None;
    for row in &steps {
        let (Some(material), Some(critical)) = (
            row["materialClearanceMm"].as_f64(),
            row["kernelCriticalTwoRMm"].as_f64(),
        ) else {
            continue;
        };
        let gap = (material - critical).abs();
        worst_gap = Some(worst_gap.map_or(gap, |current: f64| current.max(gap)));
    }
    json!({
        "source": source,
        "kind": kind,
        "placementIndices": indices,
        "demandedClearanceMm": demanded_mm,
        "centreStep": centre_step,
        "stepCount": steps.len(),
        "kernelMonotone": monotone,
        "kernelMonotoneInMaterial": monotone_in_material,
        "stepsDisagreeingOutsideBudget": disagreeing_outside_budget,
        "kernelFlipIndex": first_refusal,
        "materialFlipIndex": material_flip,
        "miterFlipIndex": miter_flip,
        "contractFlipIndex": contract_flip,
        "kernelMinusMaterialFlipSteps": flip_delta,
        "worstKernelVersusMaterialMm": worst_gap,
        "sweepFloorBudgetMm": SWEEP_FLOOR_BUDGET_MM,
        "insideSweepFloorBudget": worst_gap
            .map(|gap| gap <= SWEEP_FLOOR_BUDGET_MM),
        "steps": steps,
    })
}
