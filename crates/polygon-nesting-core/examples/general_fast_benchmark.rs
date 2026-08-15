use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use polygon_nesting_core::domain::IrregularPoint;
use polygon_nesting_core::geometry::convex::compute_convex_hull;
use polygon_nesting_core::geometry::general_polygon::{PolygonRegion, PolygonSet};
use polygon_nesting_core::search::general_fast::{
    construct_short_side_first, GeneralFastPiece, GeneralFastResult, GeneralFastSettings,
};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    name: String,
    sheet_short_axis_mm: f64,
    sheet_long_axis_mm: f64,
    total_padding_mm: f64,
    #[serde(default)]
    sheet_edge_clearance_mm: Option<f64>,
    clearance_safety_margin_mm: f64,
    flattening_sag_tolerance_mm: f64,
    best_known_depth_mm: Option<f64>,
    expected_placed: usize,
    #[serde(default)]
    expect_general_beats_hull: bool,
    pieces: Vec<FixturePiece>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixturePiece {
    id: String,
    allow_rotation: bool,
    allow_mirror: bool,
    outer: Vec<[f64; 2]>,
    #[serde(default)]
    holes: Vec<Vec<[f64; 2]>>,
}

#[derive(Clone, Copy)]
struct BenchmarkSearchConfig {
    exploratory_evaluations: usize,
    order_variants: usize,
    repair_targets: usize,
    repair_evaluations: usize,
    local_angle_evaluations: usize,
    catalog_variants: usize,
    catalog_evaluations: usize,
    pairing_evaluations: usize,
    pairing_band_variants: usize,
    partial_layouts: usize,
    beam_evaluations: usize,
    tightening_passes: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args().skip(1);
    let fixture_path = arguments.next().ok_or(
        "usage: general_fast_benchmark FIXTURE.json [runs] [exploratory-evaluations-per-piece] [order-variants] [repair-targets] [repair-evaluations-per-piece] [local-angle-evaluations-per-piece] [catalog-variants] [catalog-evaluations-per-piece] [pairing-evaluations-per-piece] [pairing-band-variants] [partial-layouts] [beam-evaluations-per-state] [tightening-passes]",
    )?;
    let runs = arguments
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(10);
    let exploratory_evaluations = arguments
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(0);
    let order_variants = arguments
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(1);
    let repair_targets = arguments
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(0);
    let repair_evaluations = arguments
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(0);
    let local_angle_evaluations = arguments
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(0);
    let catalog_variants = arguments
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(1);
    let catalog_evaluations = arguments
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(0);
    let pairing_evaluations = arguments
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(0);
    let pairing_band_variants = arguments
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(1);
    let partial_layouts = arguments
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(1);
    let beam_evaluations = arguments
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(0);
    let tightening_passes = arguments
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(0);
    if runs == 0 || arguments.next().is_some() {
        return Err("runs must be positive and no extra arguments are accepted".into());
    }

    let report = run_benchmark(
        Path::new(&fixture_path),
        runs,
        BenchmarkSearchConfig {
            exploratory_evaluations,
            order_variants,
            repair_targets,
            repair_evaluations,
            local_angle_evaluations,
            catalog_variants,
            catalog_evaluations,
            pairing_evaluations,
            pairing_band_variants,
            partial_layouts,
            beam_evaluations,
            tightening_passes,
        },
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn run_benchmark(
    fixture_path: &Path,
    runs: usize,
    config: BenchmarkSearchConfig,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let fixture_bytes = fs::read(fixture_path)?;
    let fixture: Fixture = serde_json::from_slice(&fixture_bytes)?;
    let polygons = build_polygons(&fixture)?;
    let hulls = fixture
        .pieces
        .iter()
        .map(|piece| {
            let points = piece
                .outer
                .iter()
                .map(|point| IrregularPoint::new(point[0], point[1]))
                .collect::<Vec<_>>();
            PolygonSet::from_outer(compute_convex_hull(&points).points)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let settings = settings(&fixture, config);
    let general = measure(&fixture, &polygons, settings, runs)?;
    let hull_ablation = measure(&fixture, &hulls, settings, runs)?;
    if general.result.placements.len() != fixture.expected_placed {
        return Err(format!(
            "fixture expected {} placements but general constructor produced {}",
            fixture.expected_placed,
            general.result.placements.len()
        )
        .into());
    }
    if fixture
        .best_known_depth_mm
        .is_some_and(|best_known| general.result.used_long_axis_depth_mm > best_known)
    {
        return Err(format!(
            "fixture best-known depth is {} but general constructor produced {}",
            fixture.best_known_depth_mm.unwrap(),
            general.result.used_long_axis_depth_mm
        )
        .into());
    }
    if fixture.expect_general_beats_hull
        && general.result.used_long_axis_depth_mm >= hull_ablation.result.used_long_axis_depth_mm
    {
        return Err(format!(
            "fixture requires the concave constructor depth {} to beat hull ablation depth {}",
            general.result.used_long_axis_depth_mm, hull_ablation.result.used_long_axis_depth_mm
        )
        .into());
    }

    let descriptor = json!({
        "name": fixture.name,
        "pieceCount": fixture.pieces.len(),
        "totalVertices": polygons.iter().map(PolygonSet::vertex_count).sum::<usize>(),
        "concavePieceCount": polygons.iter().filter(|polygon| {
            polygon.regions().iter().any(|region| !region.outer.is_convex())
        }).count(),
        "holedPieceCount": polygons.iter().filter(|polygon| {
            polygon.regions().iter().any(|region| !region.holes.is_empty())
        }).count(),
        "sheetShortAxisMm": fixture.sheet_short_axis_mm,
        "sheetLongAxisMm": fixture.sheet_long_axis_mm,
        "totalPaddingMm": fixture.total_padding_mm,
        "sheetEdgeClearanceMm": fixture.sheet_edge_clearance_mm.unwrap_or(fixture.total_padding_mm / 2.0),
        "bestKnownDepthMm": fixture.best_known_depth_mm,
    });
    Ok(json!({
        "identity": {
            "requestSha256": format!("{:x}", Sha256::digest(&fixture_bytes)),
            "engineCommit": engine_commit(),
            "profile": "general-fast-v1-internal",
            "seed": null,
            "workerCount": 1,
            "budgetMode": "deterministic-exact-evaluation-quota",
            "hardware": hardware_descriptor(),
            "compiler": {
                "rustc": command_output("rustc", &["-Vv"]),
                "rustFlags": option_env!("RUSTFLAGS").unwrap_or(""),
                "releaseAssertions": !cfg!(debug_assertions),
            },
        },
        "fixture": descriptor,
        "workQuota": {
            "runs": runs,
            "primaryEvaluationsPerPiece": settings.max_evaluations_per_piece,
            "exploratoryEvaluationsPerPiece": config.exploratory_evaluations,
            "orderVariants": config.order_variants,
            "repairTargets": config.repair_targets,
            "repairEvaluationsPerPiece": config.repair_evaluations,
            "localAngleEvaluationsPerPiece": config.local_angle_evaluations,
            "catalogVariants": config.catalog_variants,
            "catalogEvaluationsPerPiece": config.catalog_evaluations,
            "pairingEvaluationsPerPiece": config.pairing_evaluations,
            "pairingBandVariants": config.pairing_band_variants,
            "partialLayouts": config.partial_layouts,
            "beamEvaluationsPerState": config.beam_evaluations,
            "tighteningPasses": config.tightening_passes,
        },
        "general": report(&fixture, &polygons, &general),
        "convexHullAblation": report(&fixture, &hulls, &hull_ablation),
    }))
}

fn engine_commit() -> String {
    option_env!("POLYGON_NESTING_SOURCE_REVISION")
        .filter(|revision| !revision.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| command_output("git", &["rev-parse", "HEAD"]))
}

fn hardware_descriptor() -> serde_json::Value {
    let cpu_model = if cfg!(target_os = "macos") {
        command_output("sysctl", &["-n", "machdep.cpu.brand_string"])
    } else if cfg!(target_os = "linux") {
        fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|contents| {
                contents.lines().find_map(|line| {
                    line.strip_prefix("model name\t:")
                        .or_else(|| line.strip_prefix("Model\t\t:"))
                        .map(str::trim)
                        .map(str::to_owned)
                })
            })
            .unwrap_or_else(|| "unknown".to_owned())
    } else {
        "unknown".to_owned()
    };
    json!({
        "os": env::consts::OS,
        "architecture": env::consts::ARCH,
        "cpuModel": cpu_model,
        "availableParallelism": std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get),
    })
}

fn command_output(program: &str, arguments: &[&str]) -> String {
    Command::new(program)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|output| output.trim().to_owned())
        .filter(|output| !output.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn build_polygons(fixture: &Fixture) -> Result<Vec<PolygonSet>, Box<dyn std::error::Error>> {
    fixture
        .pieces
        .iter()
        .map(|piece| {
            let outer = piece
                .outer
                .iter()
                .map(|point| IrregularPoint::new(point[0], point[1]))
                .collect::<Vec<_>>();
            let holes = piece
                .holes
                .iter()
                .map(|hole| {
                    hole.iter()
                        .map(|point| IrregularPoint::new(point[0], point[1]))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            Ok(PolygonSet::new(vec![PolygonRegion::new(outer, holes)?])?)
        })
        .collect()
}

fn settings(fixture: &Fixture, config: BenchmarkSearchConfig) -> GeneralFastSettings {
    let mut settings = GeneralFastSettings::deterministic_test(
        fixture.sheet_short_axis_mm,
        fixture.sheet_long_axis_mm,
    );
    settings.total_padding_mm = fixture.total_padding_mm;
    settings.sheet_edge_clearance_mm = fixture.sheet_edge_clearance_mm;
    settings.clearance_safety_margin_mm = fixture.clearance_safety_margin_mm;
    settings.flattening_sag_tolerance_mm = fixture.flattening_sag_tolerance_mm;
    settings.max_exploratory_evaluations_per_piece = config.exploratory_evaluations;
    settings.max_order_variants = config.order_variants;
    settings.max_repair_targets = config.repair_targets;
    settings.max_repair_evaluations_per_piece = config.repair_evaluations;
    settings.max_local_angle_refinement_evaluations_per_piece = config.local_angle_evaluations;
    settings.max_catalog_variants = config.catalog_variants;
    settings.max_catalog_evaluations_per_piece = config.catalog_evaluations;
    settings.max_pairing_evaluations_per_piece = config.pairing_evaluations;
    settings.max_pairing_band_variants = config.pairing_band_variants;
    settings.max_partial_layouts = config.partial_layouts;
    settings.max_beam_evaluations_per_state = config.beam_evaluations;
    settings.max_tightening_passes = config.tightening_passes;
    settings
}

struct Measurement {
    result: GeneralFastResult,
    elapsed_ms: Vec<f64>,
}

fn measure(
    fixture: &Fixture,
    polygons: &[PolygonSet],
    settings: GeneralFastSettings,
    runs: usize,
) -> Result<Measurement, Box<dyn std::error::Error>> {
    let pieces = fixture
        .pieces
        .iter()
        .zip(polygons)
        .map(|(piece, polygon)| GeneralFastPiece {
            id: &piece.id,
            polygon,
            allow_rotation: piece.allow_rotation,
            allow_mirror: piece.allow_mirror,
        })
        .collect::<Vec<_>>();
    let mut result = None;
    let mut elapsed_ms = Vec::with_capacity(runs);
    for _ in 0..runs {
        let started = Instant::now();
        let current = construct_short_side_first(&pieces, settings)?;
        elapsed_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
        if let Some(reference) = &result {
            if reference != &current {
                return Err("deterministic work-quota replay produced different results".into());
            }
        } else {
            result = Some(current);
        }
    }
    elapsed_ms.sort_by(f64::total_cmp);
    Ok(Measurement {
        result: result.expect("positive run count produces one result"),
        elapsed_ms,
    })
}

fn report(
    fixture: &Fixture,
    polygons: &[PolygonSet],
    measurement: &Measurement,
) -> serde_json::Value {
    let area_by_id = fixture
        .pieces
        .iter()
        .zip(polygons)
        .map(|(piece, polygon)| (piece.id.as_str(), polygon.area_mm2()))
        .collect::<BTreeMap<_, _>>();
    let placed_area_mm2 = measurement
        .result
        .placements
        .iter()
        .map(|placement| area_by_id[placement.piece_id.as_str()])
        .sum::<f64>();
    let used_strip_area_mm2 =
        fixture.sheet_short_axis_mm * measurement.result.used_long_axis_depth_mm;
    let utilization_percent = if used_strip_area_mm2 > 0.0 {
        placed_area_mm2 / used_strip_area_mm2 * 100.0
    } else {
        0.0
    };
    let mut report = json!({
        "placed": measurement.result.placements.len(),
        "unplaced": measurement.result.unplaced_piece_ids.len(),
        "usedLongAxisDepthMm": measurement.result.used_long_axis_depth_mm,
        "usedShortAxisSpanMm": measurement.result.used_short_axis_span_mm,
        "occupiedEnvelopeAreaMm2": measurement.result.occupied_envelope_area_mm2,
        "placedMaterialAreaMm2": placed_area_mm2,
        "usedStripUtilizationPercent": utilization_percent,
        "independentGeometryScore": {
            "placedMaterialAreaMm2": placed_area_mm2,
            "usedStripAreaMm2": used_strip_area_mm2,
            "usedStripUtilizationPercent": utilization_percent,
        },
        "exactEvaluations": measurement.result.exact_evaluations,
        "primaryExactEvaluations": measurement.result.primary_exact_evaluations,
        "orderPortfolioExactEvaluations": measurement.result.order_portfolio_exact_evaluations,
        "catalogPortfolioExactEvaluations": measurement.result.catalog_portfolio_exact_evaluations,
        "pairingExactEvaluations": measurement.result.pairing_exact_evaluations,
        "beamExactEvaluations": measurement.result.beam_exact_evaluations,
        "catalogCandidatePlacedCount": measurement.result.catalog_candidate_placed_count,
        "catalogCandidateDepthMm": measurement.result.catalog_candidate_depth_mm,
        "pairingCandidatePlacedCount": measurement.result.pairing_candidate_placed_count,
        "pairingCandidateDepthMm": measurement.result.pairing_candidate_depth_mm,
        "beamCandidatePlacedCount": measurement.result.beam_candidate_placed_count,
        "beamCandidateDepthMm": measurement.result.beam_candidate_depth_mm,
        "exploratoryExactEvaluations": measurement.result.exploratory_exact_evaluations,
        "orderVariantsAttempted": measurement.result.order_variants_attempted,
        "catalogVariantsAttempted": measurement.result.catalog_variants_attempted,
        "orderPortfolioFailed": measurement.result.order_portfolio_failed,
        "catalogPortfolioFailed": measurement.result.catalog_portfolio_failed,
        "pairingFailed": measurement.result.pairing_failed,
        "beamFailed": measurement.result.beam_failed,
        "repairExactEvaluations": measurement.result.repair_exact_evaluations,
        "localAngleRefinementExactEvaluations": measurement.result.local_angle_refinement_exact_evaluations,
        "repairTargetsConsidered": measurement.result.repair_targets_considered,
        "repairFailed": measurement.result.repair_failed,
        "exploratoryFailed": measurement.result.exploratory_failed,
        "elapsedMs": {
            "min": measurement.elapsed_ms[0],
            "median": quantile(&measurement.elapsed_ms, 0.5),
            "p25": quantile(&measurement.elapsed_ms, 0.25),
            "p75": quantile(&measurement.elapsed_ms, 0.75),
            "max": measurement.elapsed_ms[measurement.elapsed_ms.len() - 1],
        },
        "placements": measurement.result.placements.iter().map(|placement| json!({
            "pieceId": placement.piece_id,
            "rotationDeg": placement.rotation_deg,
            "mirrored": placement.mirrored,
            "translateShortAxis": placement.translate_short_axis,
            "translateLongAxis": placement.translate_long_axis,
        })).collect::<Vec<_>>(),
    });
    let report_object = report
        .as_object_mut()
        .expect("benchmark report is constructed as an object");
    report_object.insert(
        "tighteningExactEvaluations".to_owned(),
        json!(measurement.result.tightening_exact_evaluations),
    );
    report_object.insert(
        "tighteningPassesAttempted".to_owned(),
        json!(measurement.result.tightening_passes_attempted),
    );
    report_object.insert(
        "tighteningPassesImproved".to_owned(),
        json!(measurement.result.tightening_passes_improved),
    );
    report
}

fn quantile(sorted: &[f64], probability: f64) -> f64 {
    let index = ((sorted.len() - 1) as f64 * probability).round() as usize;
    sorted[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> BenchmarkSearchConfig {
        BenchmarkSearchConfig {
            exploratory_evaluations: 0,
            order_variants: 1,
            repair_targets: 0,
            repair_evaluations: 0,
            local_angle_evaluations: 0,
            catalog_variants: 1,
            catalog_evaluations: 0,
            pairing_evaluations: 0,
            pairing_band_variants: 1,
            partial_layouts: 1,
            beam_evaluations: 0,
            tightening_passes: 0,
        }
    }

    #[test]
    fn protected_concave_fixture_meets_best_known_contract() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/general-concave/constructor-v1.json");
        run_benchmark(&fixture, 2, config()).unwrap();
    }

    #[test]
    fn protected_concave_fixture_exercises_experimental_arms() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/general-concave/constructor-v1.json");
        let report = run_benchmark(
            &fixture,
            1,
            BenchmarkSearchConfig {
                catalog_variants: 2,
                catalog_evaluations: 32,
                pairing_evaluations: 32,
                pairing_band_variants: 2,
                ..config()
            },
        )
        .unwrap();
        assert_eq!(report["general"]["catalogPortfolioFailed"], false);
        assert_eq!(report["general"]["pairingFailed"], false);
    }
}
