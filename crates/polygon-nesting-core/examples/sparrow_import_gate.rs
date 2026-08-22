//! Gate A's driver: three verdicts on one imported pose set, and their slacks.
//!
//! ```text
//! sparrow_import_gate REQUEST.json POSES.json SHEET_EDGE_CLEARANCE PAIR_CLEARANCE
//!                     ALLOWANCES ARC_TOLERANCE_GRID BISECT_TOP
//! ```
//!
//! `ALLOWANCES` is a comma-separated list of `search_offset_allowance_mm`
//! values; every one of them gets a full three-verdict row, because the
//! allowance is the difference between "the miter *join* rejects" and "the
//! envelope *radius* rejects" and a single number cannot tell those apart.
//!
//! The request loader below is the benchmark example's, reduced to the fields a
//! pose set needs: the same `polygon_set_from_imported_piece`, the same
//! `GeneralFastSettings::deterministic_test` seed, the same
//! `sheet.width >= sheet.height` axis-normalisation rule (which is `false` for
//! the 2000x2700 mixed-61 sheet, so the identity). It carries no search
//! configuration at all - nothing here runs a search.
//!
//! Output is one JSON document on stdout. Nothing is written in place.

use std::collections::BTreeMap;
use std::env;
use std::fs;

use polygon_nesting_core::clipper::offset::JoinType;
use polygon_nesting_core::domain::ImportedPiece;
use polygon_nesting_core::geometry::general_polygon::PolygonSet;
use polygon_nesting_core::geometry::general_source::polygon_set_from_imported_piece;
use polygon_nesting_core::search::general_fast::{
    GeneralFastPiece, GeneralFastPlacement, GeneralFastSettings,
};
use polygon_nesting_core::search::import_gate::{
    authority_verdict, census, Census, EnvelopeSpec,
};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

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

#[derive(Deserialize)]
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

fn census_json(census: &Census) -> serde_json::Value {
    json!({
        "label": census.label,
        "join": census.join,
        "radiusMm": census.radius_mm,
        "miterLimit": census.miter_limit,
        "arcToleranceGridUnits": census.arc_tolerance_grid,
        "roundInwardDeviationMm": census.round_inward_deviation_mm,
        "sheetInsetMm": census.sheet_inset_mm,
        "reproducesProductionOffset": census.reproduces_production_offset,
        "envelopeAdmissible": census.envelope_admissible,
        "boundaryFailureCount": census.boundary_failure_count,
        "pairFailureCount": census.pair_failure_count,
        "pairCount": census.pair_count,
        "saturatedPairRows": census.saturated_pair_rows,
        "saturatedBoundaryRows": census.saturated_boundary_rows,
        "envelopeVertexTotal": census.envelope_vertex_total,
        "pairs": census.pairs.iter().map(|row| json!({
            "placementIndices": [row.first_index, row.second_index],
            "pieceIds": [row.first_piece_id, row.second_piece_id],
            "materialClearanceMm": row.material_clearance_mm,
            "envelopeOverlaps": row.envelope_overlaps,
            "envelopeIntersectionAreaMm2": row.envelope_intersection_area_mm2,
            "criticalRadiusMm": row.critical_radius_mm,
            "criticalRadiusSaturated": row.critical_radius_saturated,
            "radiusSlackMm": row.radius_slack_mm,
            "clearanceSlackMm": row.clearance_slack_mm,
            "joinCostMm": row.join_cost_mm,
        })).collect::<Vec<_>>(),
        "boundaries": census.boundaries.iter().map(|row| json!({
            "placementIndex": row.index,
            "pieceId": row.piece_id,
            "materialClearanceMm": row.material_clearance_mm,
            "envelopeFits": row.envelope_fits,
            "envelopeExcursionMm": row.envelope_excursion_mm,
            "criticalRadiusMm": row.critical_radius_mm,
            "criticalRadiusSaturated": row.critical_radius_saturated,
            "radiusSlackMm": row.radius_slack_mm,
        })).collect::<Vec<_>>(),
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args().skip(1);
    let request_path = arguments
        .next()
        .ok_or("usage: sparrow_import_gate REQUEST POSES EDGE_CLEARANCE PAIR_CLEARANCE ALLOWANCES ARC_TOLERANCE_GRID BISECT_TOP")?;
    let poses_path = arguments.next().ok_or("missing pose fixture")?;
    let edge_clearance_mm: f64 = arguments.next().ok_or("missing edge clearance")?.parse()?;
    let pair_clearance_mm: f64 = arguments.next().ok_or("missing pair clearance")?.parse()?;
    let allowances = arguments
        .next()
        .ok_or("missing allowance list")?
        .split(',')
        .map(|value| value.trim().parse::<f64>())
        .collect::<Result<Vec<_>, _>>()?;
    let arc_tolerance_grid: f64 = arguments.next().ok_or("missing arc tolerance")?.parse()?;
    let bisect_top: usize = arguments.next().ok_or("missing bisect top")?.parse()?;
    if arguments.next().is_some() {
        return Err("no extra arguments are accepted".into());
    }

    let request_bytes = fs::read(&request_path)?;
    let request_sha256 = format!("{:x}", Sha256::digest(&request_bytes));
    let request: Request = serde_json::from_slice(&request_bytes)?;
    let poses_bytes = fs::read(&poses_path)?;
    let poses_sha256 = format!("{:x}", Sha256::digest(&poses_bytes));
    let poses: PoseFixture = serde_json::from_slice(&poses_bytes)?;

    let (request_total_padding_mm, allow_global_rotation, allow_global_mirror, geometry) =
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
    // `sheet.width >= sheet.height` is the benchmark's axis-normalisation rule;
    // mixed-61 is 2000x2700, so this is `false` and the polygons are the source
    // rings untouched. Reproduced rather than assumed.
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
    let placements = poses
        .placements
        .iter()
        .map(|pose| GeneralFastPlacement {
            piece_id: pose.piece_id.clone(),
            rotation_deg: pose.rotation_deg,
            mirrored: pose.mirrored,
            translate_short_axis: pose.translate_short_axis,
            translate_long_axis: pose.translate_long_axis,
        })
        .collect::<Vec<_>>();

    let mut settings = GeneralFastSettings::deterministic_test(
        request.sheet.width.min(request.sheet.height),
        request.sheet.width.max(request.sheet.height),
    );
    settings.total_padding_mm = pair_clearance_mm;
    settings.sheet_edge_clearance_mm = Some(edge_clearance_mm);
    settings.clearance_safety_margin_mm = geometry.clearance_safety_margin_mm;
    settings.flattening_sag_tolerance_mm = geometry.flattening_sag_tolerance_mm;

    let mut rows = Vec::new();
    for allowance in &allowances {
        let mut settings = settings;
        settings.search_offset_allowance_mm = *allowance;
        let verdict = authority_verdict(&pieces, &placements, settings)?;
        let production = EnvelopeSpec::production("composite-miter (HEAD authority)", settings);
        let mut round = production.clone();
        round.arc_tolerance_grid = arc_tolerance_grid;
        let specs = vec![
            production.clone(),
            round.with_join("composite-round (shadow)", JoinType::Round),
            production
                .clone()
                .with_join("composite-square (shadow)", JoinType::Square),
        ];
        let censuses = specs
            .iter()
            .map(|spec| census(&pieces, &placements, settings, spec, bisect_top))
            .collect::<Result<Vec<_>, _>>()?;
        rows.push(json!({
            "searchOffsetAllowanceMm": allowance,
            "expansionMm": verdict.expansion_mm,
            "sheetInsetMm": verdict.sheet_inset_mm,
            "contractPairClearanceMm": verdict.contract_pair_clearance_mm,
            "contractSheetClearanceMm": verdict.contract_sheet_clearance_mm,
            "envelopeImpliedPairClearanceMm": 2.0 * verdict.expansion_mm,
            "envelopeImpliedSheetClearanceMm": verdict.sheet_inset_mm + verdict.expansion_mm,
            "rawSourceDepthMm": verdict.raw_source_depth_mm,
            "contractOnlyVerdict": match &verdict.contract_only {
                Ok(()) => json!({"accepted": true}),
                Err(message) => json!({"accepted": false, "message": message}),
            },
            "compositeMiterVerdict": match &verdict.composite {
                Ok(metrics) => json!({
                    "accepted": true,
                    "usedShortAxisSpanMm": metrics.used_short_axis_span_mm,
                    "usedLongAxisDepthMm": metrics.used_long_axis_depth_mm,
                }),
                Err(message) => json!({"accepted": false, "message": message}),
            },
            "censuses": censuses.iter().map(census_json).collect::<Vec<_>>(),
        }));
    }

    // The instrument's own identity, in its own output. A verdict document that
    // cannot say which binary produced it is not evidence, and the campaign's
    // rule is that every evidence file carries the binary's hash.
    let executable_sha256 = env::current_exe()
        .ok()
        .and_then(|path| fs::read(path).ok())
        .map(|bytes| format!("{:x}", Sha256::digest(&bytes)));

    let document = json!({
        "experiment": "gate-a-sparrow-import",
        "instrument": "crates/polygon-nesting-core/src/search/import_gate.rs",
        "executableSha256": executable_sha256,
        "request": {"path": request_path, "sha256": request_sha256,
                    "requestTotalPaddingMm": request_total_padding_mm,
                    "sheetShortAxisMm": settings.sheet_short_axis_mm,
                    "sheetLongAxisMm": settings.sheet_long_axis_mm,
                    "normalizeAxes": normalize_axes},
        "poses": {"path": poses_path, "sha256": poses_sha256,
                  "placementCount": placements.len()},
        "contract": {
            "pairClearanceMm": settings.total_padding_mm,
            "sheetEdgeClearanceMm": edge_clearance_mm,
            "clearanceSafetyMarginMm": settings.clearance_safety_margin_mm,
            "flatteningSagToleranceMm": settings.flattening_sag_tolerance_mm,
        },
        "bisectTop": bisect_top,
        "canonicalGridStepMm": 0.001,
        "rows": rows,
    });
    println!("{}", serde_json::to_string_pretty(&document)?);
    Ok(())
}
