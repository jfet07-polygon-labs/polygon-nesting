use std::collections::BTreeMap;

use polygon_nesting_core::geometry::predicates::orientation;
use polygon_nesting_protocol::{
    EngineOutcome, EngineProfile, EngineRequest, ExecutionDiagnostics, SourceGeometrySegment,
    SourcePiece,
};
use serde::Serialize;

const REPORT_VERSION: u32 = 1;
const MAX_SIMPLE_RING_PAIR_CHECKS: usize = 10_000_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BenchmarkReport {
    version: u32,
    engine: EngineIdentity,
    instance: InstanceDescriptor,
    run: RunDescriptor,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EngineIdentity {
    name: &'static str,
    version: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstanceDescriptor {
    part_count: u64,
    source_piece_count: u64,
    sheet: SheetDescriptor,
    profile: &'static str,
    padding_mm: f64,
    source_boundary_segment_count: u64,
    instance_boundary_segment_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_polygon_vertex_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instance_polygon_vertex_count: Option<u64>,
    rotation_allowed_part_count: u64,
    mirror_allowed_part_count: u64,
    geometry: GeometryDescriptor,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SheetDescriptor {
    width_mm: f64,
    height_mm: f64,
    area_mm2: f64,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeometryDescriptor {
    convex_part_count: u64,
    concave_part_count: u64,
    curved_or_unknown_part_count: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunDescriptor {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    placed_part_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unplaced_part_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    complete: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    engine_elapsed_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    requested_workers: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    actual_workers: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    placed_polygon_area_mm2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sheet_utilization_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    occupied_envelope_area_mm2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    occupied_envelope_density_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    best_known_sheet_utilization_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gap_to_best_known_percentage_points: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
struct SourceSummary {
    segment_count: u64,
    polygon_vertex_count: Option<u64>,
    polygon_area_mm2: Option<f64>,
    convexity: Convexity,
}

#[derive(Debug, Clone, Copy)]
enum Convexity {
    Convex,
    Concave,
    Unknown,
}

pub(crate) fn build_benchmark_report(
    request: &EngineRequest,
    outcome: &EngineOutcome,
    best_known_utilization_percent: Option<f64>,
) -> BenchmarkReport {
    let mut remaining_simple_ring_pair_checks = MAX_SIMPLE_RING_PAIR_CHECKS;
    let mut source_summaries = BTreeMap::new();
    for source in &request.source_pieces {
        source_summaries.insert(
            source.id.as_str(),
            summarize_source(source, &mut remaining_simple_ring_pair_checks),
        );
    }
    let source_boundary_segment_count = source_summaries
        .values()
        .map(|summary| summary.segment_count)
        .sum();
    let source_polygon_vertex_count = sum_optional_u64(
        source_summaries
            .values()
            .map(|summary| summary.polygon_vertex_count),
    );

    let mut instance_boundary_segment_count = 0;
    let mut instance_polygon_vertex_count = Some(0_u64);
    let mut geometry = GeometryDescriptor::default();
    for piece in &request.pieces {
        let Some(summary) = source_summaries.get(piece.source_piece_id.as_str()) else {
            instance_polygon_vertex_count = None;
            geometry.curved_or_unknown_part_count += 1;
            continue;
        };
        instance_boundary_segment_count += summary.segment_count;
        instance_polygon_vertex_count = instance_polygon_vertex_count
            .zip(summary.polygon_vertex_count)
            .map(|(total, count)| total + count);
        match summary.convexity {
            Convexity::Convex => geometry.convex_part_count += 1,
            Convexity::Concave => geometry.concave_part_count += 1,
            Convexity::Unknown => geometry.curved_or_unknown_part_count += 1,
        }
    }

    let diagnostics = outcome_diagnostics(outcome);
    let run = match outcome {
        EngineOutcome::Success { result, .. } => {
            let placed_polygon_area_mm2 =
                sum_optional(result.placed_collision_geometries.iter().map(|placed| {
                    source_summaries
                        .get(placed.placement.source_piece_id.as_str())
                        .and_then(|summary| summary.polygon_area_mm2)
                }));
            let sheet_area_mm2 = request.sheet.width * request.sheet.height;
            let sheet_utilization_percent =
                placed_polygon_area_mm2.map(|area| percentage(area, sheet_area_mm2));
            let occupied_envelope_area_mm2 = result.score.collision_bounds_area_mm2;
            let occupied_envelope_density_percent = placed_polygon_area_mm2.and_then(|area| {
                (occupied_envelope_area_mm2 > 0.0)
                    .then(|| percentage(area, occupied_envelope_area_mm2))
            });
            let unplaced_part_count = result.unplaced_piece_ids.len() as u64;
            RunDescriptor {
                status: "success",
                placed_part_count: Some(result.placed_collision_geometries.len() as u64),
                unplaced_part_count: Some(unplaced_part_count),
                complete: Some(unplaced_part_count == 0),
                engine_elapsed_ms: diagnostics.elapsed_ms,
                requested_workers: diagnostics.requested_workers,
                actual_workers: diagnostics.actual_workers,
                placed_polygon_area_mm2,
                sheet_utilization_percent,
                occupied_envelope_area_mm2: Some(occupied_envelope_area_mm2),
                occupied_envelope_density_percent,
                best_known_sheet_utilization_percent: best_known_utilization_percent,
                gap_to_best_known_percentage_points: best_known_utilization_percent
                    .zip(sheet_utilization_percent)
                    .map(|(best, actual)| best - actual),
            }
        }
        EngineOutcome::Failure { .. } => failed_run("failure", diagnostics),
        EngineOutcome::ArchiveIneligible { .. } => failed_run("archive-ineligible", diagnostics),
    };

    BenchmarkReport {
        version: REPORT_VERSION,
        engine: EngineIdentity {
            name: "polygon-nesting",
            version: env!("CARGO_PKG_VERSION"),
        },
        instance: InstanceDescriptor {
            part_count: request.pieces.len() as u64,
            source_piece_count: request.source_pieces.len() as u64,
            sheet: SheetDescriptor {
                width_mm: request.sheet.width,
                height_mm: request.sheet.height,
                area_mm2: request.sheet.width * request.sheet.height,
            },
            profile: match request.profile {
                EngineProfile::Compact => "compact",
                EngineProfile::CompactShortSide => "compact-short-side",
            },
            padding_mm: request.settings.padding,
            source_boundary_segment_count,
            instance_boundary_segment_count,
            source_polygon_vertex_count,
            instance_polygon_vertex_count,
            rotation_allowed_part_count: request
                .pieces
                .iter()
                .filter(|piece| request.settings.allow_global_rotation && piece.allow_rotation)
                .count() as u64,
            mirror_allowed_part_count: request
                .pieces
                .iter()
                .filter(|piece| request.settings.allow_global_mirror && piece.allow_mirror)
                .count() as u64,
            geometry,
        },
        run,
    }
}

fn failed_run(status: &'static str, diagnostics: &ExecutionDiagnostics) -> RunDescriptor {
    RunDescriptor {
        status,
        placed_part_count: None,
        unplaced_part_count: None,
        complete: None,
        engine_elapsed_ms: diagnostics.elapsed_ms,
        requested_workers: diagnostics.requested_workers,
        actual_workers: diagnostics.actual_workers,
        placed_polygon_area_mm2: None,
        sheet_utilization_percent: None,
        occupied_envelope_area_mm2: None,
        occupied_envelope_density_percent: None,
        best_known_sheet_utilization_percent: None,
        gap_to_best_known_percentage_points: None,
    }
}

fn outcome_diagnostics(outcome: &EngineOutcome) -> &ExecutionDiagnostics {
    match outcome {
        EngineOutcome::Success { diagnostics, .. }
        | EngineOutcome::Failure { diagnostics, .. }
        | EngineOutcome::ArchiveIneligible { diagnostics, .. } => diagnostics,
    }
}

fn sum_optional(values: impl IntoIterator<Item = Option<f64>>) -> Option<f64> {
    values
        .into_iter()
        .try_fold(0.0, |total, value| value.map(|value| total + value))
}

fn sum_optional_u64(values: impl IntoIterator<Item = Option<u64>>) -> Option<u64> {
    values
        .into_iter()
        .try_fold(0, |total, value| value.map(|value| total + value))
}

fn percentage(numerator: f64, denominator: f64) -> f64 {
    numerator / denominator * 100.0
}

fn summarize_source(source: &SourcePiece, remaining_pair_checks: &mut usize) -> SourceSummary {
    let segment_count = source.geometry.segments.len() as u64;
    let Some(points) = polygon_points(source) else {
        return SourceSummary {
            segment_count,
            polygon_vertex_count: None,
            polygon_area_mm2: None,
            convexity: Convexity::Unknown,
        };
    };
    if !simple_ring_within_budget(&points, remaining_pair_checks) {
        return SourceSummary {
            segment_count,
            polygon_vertex_count: None,
            polygon_area_mm2: None,
            convexity: Convexity::Unknown,
        };
    }
    let twice_area = points
        .iter()
        .copied()
        .zip(points.iter().copied().cycle().skip(1))
        .take(points.len())
        .map(|((x1, y1), (x2, y2))| x1 * y2 - x2 * y1)
        .sum::<f64>();
    let mut positive_cross = false;
    let mut negative_cross = false;
    for index in 0..points.len() {
        let (ax, ay) = points[index];
        let (bx, by) = points[(index + 1) % points.len()];
        let (cx, cy) = points[(index + 2) % points.len()];
        let cross = (bx - ax) * (cy - by) - (by - ay) * (cx - bx);
        positive_cross |= cross > 0.0;
        negative_cross |= cross < 0.0;
    }
    let convexity = match (positive_cross, negative_cross) {
        (true, true) => Convexity::Concave,
        (true, false) | (false, true) => Convexity::Convex,
        (false, false) => Convexity::Unknown,
    };
    SourceSummary {
        segment_count,
        polygon_vertex_count: Some(points.len() as u64),
        polygon_area_mm2: Some(twice_area.abs() / 2.0),
        convexity,
    }
}

fn simple_ring_within_budget(points: &[(f64, f64)], remaining_pair_checks: &mut usize) -> bool {
    let required_checks = points
        .len()
        .checked_mul(points.len().saturating_sub(3))
        .and_then(|value| value.checked_div(2));
    let Some(required_checks) = required_checks else {
        return false;
    };
    if required_checks > *remaining_pair_checks {
        return false;
    }
    *remaining_pair_checks -= required_checks;

    for index in 0..points.len() {
        if points[index] == points[(index + 1) % points.len()] {
            return false;
        }
    }
    if !(2..points.len()).any(|index| point_orientation(points[0], points[1], points[index]) != 0) {
        return false;
    }
    for first_index in 0..points.len() {
        let first = (
            points[first_index],
            points[(first_index + 1) % points.len()],
        );
        for second_index in (first_index + 1)..points.len() {
            if second_index == first_index + 1
                || (first_index == 0 && second_index == points.len() - 1)
            {
                continue;
            }
            let second = (
                points[second_index],
                points[(second_index + 1) % points.len()],
            );
            if segments_intersect(first, second) {
                return false;
            }
        }
    }
    true
}

fn segments_intersect(first: ((f64, f64), (f64, f64)), second: ((f64, f64), (f64, f64))) -> bool {
    let (first_start, first_end) = first;
    let (second_start, second_end) = second;
    let first_start_turn = point_orientation(first_start, first_end, second_start);
    let first_end_turn = point_orientation(first_start, first_end, second_end);
    let second_start_turn = point_orientation(second_start, second_end, first_start);
    let second_end_turn = point_orientation(second_start, second_end, first_end);

    if first_start_turn == 0 && point_is_on_segment(second_start, first) {
        return true;
    }
    if first_end_turn == 0 && point_is_on_segment(second_end, first) {
        return true;
    }
    if second_start_turn == 0 && point_is_on_segment(first_start, second) {
        return true;
    }
    if second_end_turn == 0 && point_is_on_segment(first_end, second) {
        return true;
    }
    first_start_turn != first_end_turn && second_start_turn != second_end_turn
}

fn point_orientation(origin: (f64, f64), first: (f64, f64), second: (f64, f64)) -> i32 {
    orientation(origin.0, origin.1, first.0, first.1, second.0, second.1)
}

fn point_is_on_segment(point: (f64, f64), segment: ((f64, f64), (f64, f64))) -> bool {
    let (start, end) = segment;
    point.0 >= start.0.min(end.0)
        && point.0 <= start.0.max(end.0)
        && point.1 >= start.1.min(end.1)
        && point.1 <= start.1.max(end.1)
}

fn polygon_points(source: &SourcePiece) -> Option<Vec<(f64, f64)>> {
    if !source.geometry.closed || source.geometry.segments.len() < 3 {
        return None;
    }
    let mut points = Vec::with_capacity(source.geometry.segments.len());
    for segment in &source.geometry.segments {
        let SourceGeometrySegment::Line(line) = segment else {
            return None;
        };
        if line.bulge.is_some() || line.source_curve.is_some() {
            return None;
        }
        points.push((line.x1, line.y1));
    }
    for (index, segment) in source.geometry.segments.iter().enumerate() {
        let SourceGeometrySegment::Line(line) = segment else {
            return None;
        };
        if (line.x2, line.y2) != points[(index + 1) % points.len()] {
            return None;
        }
    }
    Some(points)
}

#[cfg(test)]
mod tests {
    use polygon_nesting_protocol::{
        Bounds, CollisionGeometry, CollisionTransform, EngineOutcome, EngineResult,
        ExecutionDiagnostics, IrregularTransformReason, LayoutScore, PlacedCollisionGeometry,
        Placement, PlacementTransform, Polygon,
    };

    use super::{build_benchmark_report, simple_ring_within_budget};
    use crate::polygon_input::{import_polygon_json, PolygonImportOptions};
    use polygon_nesting_protocol::EngineProfile;

    #[test]
    fn reports_instance_shape_mix_and_sheet_utilization() {
        let request = import_polygon_json(
            br#"{
              "version": 1,
              "polygons": [
                {"id": "convex", "points": [[0, 0], [10, 0], [10, 10], [0, 10]]},
                {"id": "concave", "points": [[0, 0], [10, 0], [5, 5], [10, 10], [0, 10]]}
              ]
            }"#,
            &PolygonImportOptions {
                sheet_width: 100.0,
                sheet_height: 100.0,
                padding: 10,
                profile: EngineProfile::Compact,
                allow_mirror: true,
                timeout_ms: 1000.0,
            },
        )
        .expect("request should import");
        let placed = request
            .pieces
            .iter()
            .map(|piece| PlacedCollisionGeometry {
                placement: Placement {
                    piece_id: Some(piece.id.clone()),
                    source_piece_id: piece.source_piece_id.clone(),
                    placement_reference: None,
                    transform: PlacementTransform {
                        translate_x: 0.0,
                        translate_y: 0.0,
                        rotation_deg: 0.0,
                        mirrored: false,
                    },
                },
                collision_geometry: CollisionGeometry {
                    source_piece_id: piece.source_piece_id.clone(),
                    transform: CollisionTransform {
                        index: 0.0,
                        rotation_deg: 0.0,
                        mirrored: false,
                        reason: IrregularTransformReason::Orthogonal,
                    },
                    polygon: Polygon { points: Vec::new() },
                    bounds: Bounds {
                        min_x: 0.0,
                        min_y: 0.0,
                        max_x: 0.0,
                        max_y: 0.0,
                    },
                },
            })
            .collect();
        let outcome = EngineOutcome::Success {
            result: EngineResult {
                placed_collision_geometries: placed,
                score: LayoutScore {
                    collision_bounds_area_mm2: 200.0,
                    ..LayoutScore::default()
                },
                ..EngineResult::default()
            },
            diagnostics: ExecutionDiagnostics {
                elapsed_ms: Some(12.5),
                ..ExecutionDiagnostics::default()
            },
        };

        let value = serde_json::to_value(build_benchmark_report(&request, &outcome, Some(2.0)))
            .expect("report should encode");

        assert_eq!(value["instance"]["geometry"]["convexPartCount"], 1);
        assert_eq!(value["instance"]["geometry"]["concavePartCount"], 1);
        assert_eq!(value["instance"]["instancePolygonVertexCount"], 9);
        assert_eq!(value["run"]["placedPolygonAreaMm2"], 175.0);
        let utilization = value["run"]["sheetUtilizationPercent"]
            .as_f64()
            .expect("utilization should be numeric");
        let gap = value["run"]["gapToBestKnownPercentagePoints"]
            .as_f64()
            .expect("gap should be numeric");
        assert!((utilization - 1.75).abs() < f64::EPSILON * 2.0);
        assert!((gap - 0.25).abs() < f64::EPSILON * 2.0);
        assert_eq!(value["run"]["engineElapsedMs"], 12.5);
    }

    #[test]
    fn omits_polygon_measurements_for_a_self_intersecting_source_ring() {
        let mut request = import_polygon_json(
            br#"{
              "version": 1,
              "polygons": [
                {"id": "part", "points": [[0, 0], [10, 0], [10, 10], [0, 10]]}
              ]
            }"#,
            &PolygonImportOptions {
                sheet_width: 100.0,
                sheet_height: 100.0,
                padding: 10,
                profile: EngineProfile::Compact,
                allow_mirror: true,
                timeout_ms: 1000.0,
            },
        )
        .expect("request should import");
        let points = [(0.0, 0.0), (10.0, 10.0), (0.0, 10.0), (10.0, 0.0)];
        for (segment, (start, end)) in request.source_pieces[0]
            .geometry
            .segments
            .iter_mut()
            .zip(points.into_iter().zip(points.into_iter().cycle().skip(1)))
        {
            let polygon_nesting_protocol::SourceGeometrySegment::Line(line) = segment else {
                panic!("fixture should contain only line segments");
            };
            line.x1 = start.0;
            line.y1 = start.1;
            line.x2 = end.0;
            line.y2 = end.1;
        }
        let outcome = EngineOutcome::Success {
            result: EngineResult {
                placed_collision_geometries: vec![PlacedCollisionGeometry {
                    placement: Placement {
                        piece_id: Some("part#1".to_owned()),
                        source_piece_id: "part".to_owned(),
                        placement_reference: None,
                        transform: PlacementTransform {
                            translate_x: 0.0,
                            translate_y: 0.0,
                            rotation_deg: 0.0,
                            mirrored: false,
                        },
                    },
                    collision_geometry: CollisionGeometry {
                        source_piece_id: "part".to_owned(),
                        transform: CollisionTransform {
                            index: 0.0,
                            rotation_deg: 0.0,
                            mirrored: false,
                            reason: IrregularTransformReason::Orthogonal,
                        },
                        polygon: Polygon { points: Vec::new() },
                        bounds: Bounds {
                            min_x: 0.0,
                            min_y: 0.0,
                            max_x: 10.0,
                            max_y: 10.0,
                        },
                    },
                }],
                score: LayoutScore {
                    collision_bounds_area_mm2: 100.0,
                    ..LayoutScore::default()
                },
                ..EngineResult::default()
            },
            diagnostics: ExecutionDiagnostics::default(),
        };

        let value = serde_json::to_value(build_benchmark_report(&request, &outcome, None))
            .expect("report should encode");

        assert_eq!(value["instance"]["geometry"]["curvedOrUnknownPartCount"], 1);
        assert!(value["instance"].get("sourcePolygonVertexCount").is_none());
        assert!(value["instance"]
            .get("instancePolygonVertexCount")
            .is_none());
        assert!(value["run"].get("placedPolygonAreaMm2").is_none());
        assert!(value["run"].get("sheetUtilizationPercent").is_none());
        assert!(value["run"].get("occupiedEnvelopeDensityPercent").is_none());
    }

    #[test]
    fn rejects_a_collinear_three_edge_ring_for_benchmarking() {
        let mut remaining_pair_checks = 1;

        assert!(!simple_ring_within_budget(
            &[(0.0, 0.0), (2.0, 0.0), (1.0, 0.0)],
            &mut remaining_pair_checks,
        ));
    }
}
