//! Ordered source-contour ingress for the opt-in general polygon path.
//!
//! Unlike the legacy hull builder, this assembler never globally deduplicates
//! or reorders source points. Protocol-v1 input must describe exactly one
//! closed cycle whose adjacent endpoints agree after contractual-grid
//! snapping. Public multi-contour and hole ingress remains a later protocol
//! addition.

use std::collections::BTreeMap;

use crate::canonical_grid::to_grid_mm;
use crate::domain::{DxfEllipseSource, DxfGeometrySegment, ImportedPiece, IrregularPoint};
use crate::transforms::flattening::{arc, ellipse};

use super::general_polygon::{GeneralPolygonError, PolygonSet, GENERAL_MAX_RING_VERTICES};

pub fn polygon_set_from_imported_piece(
    piece: &ImportedPiece,
    flattening_sag_tolerance_mm: f64,
) -> Result<PolygonSet, GeneralPolygonError> {
    // zero sag is exact for pure segment geometry and is only rejected when
    // a curved segment actually needs sampling.
    if !flattening_sag_tolerance_mm.is_finite() || flattening_sag_tolerance_mm < 0.0 {
        return Err(error(
            "flattening sag tolerance must be finite and non-negative",
        ));
    }
    if !piece.geometry.closed {
        return Err(error(
            "general polygon source geometry must declare one closed cycle",
        ));
    }
    if piece.geometry.segments.len() > GENERAL_MAX_RING_VERTICES {
        return Err(error(format!(
            "source cycle exceeds the {GENERAL_MAX_RING_VERTICES}-segment limit"
        )));
    }

    let mut sampled_curves = BTreeMap::<String, DxfEllipseSource>::new();
    let mut analytic_segments = Vec::new();
    let mut chains = Vec::<Vec<IrregularPoint>>::new();
    let mut sampled_vertices = 0usize;
    for segment in &piece.geometry.segments {
        let max_segment_points = GENERAL_MAX_RING_VERTICES
            .saturating_sub(sampled_vertices)
            .saturating_add(1);
        if max_segment_points < 2 {
            return Err(error(format!(
                "sampled source cycle exceeds the {GENERAL_MAX_RING_VERTICES}-vertex limit"
            )));
        }
        let points = match segment {
            DxfGeometrySegment::Line(line) => {
                if let Some(source_curve) = &line.source_curve {
                    if let Some(previous) = sampled_curves.get(&source_curve.source_id) {
                        if previous != source_curve {
                            return Err(error(
                                "one source-curve id must not describe different curves",
                            ));
                        }
                        continue;
                    }
                    analytic_segments.push(segment.clone());
                    sampled_curves.insert(source_curve.source_id.clone(), source_curve.clone());
                    if flattening_sag_tolerance_mm <= 0.0 {
                        return Err(error(
                            "flattening sag tolerance must be positive when curved segments are present",
                        ));
                    }
                    ellipse::sample_points_bounded(
                        source_curve,
                        flattening_sag_tolerance_mm,
                        max_segment_points,
                    )
                    .ok_or_else(|| {
                        error(format!(
                            "sampled source cycle exceeds the {GENERAL_MAX_RING_VERTICES}-vertex limit"
                        ))
                    })?
                } else if line.bulge.is_some_and(|bulge| bulge != 0.0) {
                    analytic_segments.push(segment.clone());
                    if flattening_sag_tolerance_mm <= 0.0 {
                        return Err(error(
                            "flattening sag tolerance must be positive when curved segments are present",
                        ));
                    }
                    arc::sample_bulge_points_bounded(
                        line,
                        flattening_sag_tolerance_mm,
                        max_segment_points,
                    )
                    .ok_or_else(|| {
                        error(format!(
                            "sampled source cycle exceeds the {GENERAL_MAX_RING_VERTICES}-vertex limit"
                        ))
                    })?
                } else {
                    analytic_segments.push(segment.clone());
                    vec![
                        IrregularPoint::new(line.x1, line.y1),
                        IrregularPoint::new(line.x2, line.y2),
                    ]
                }
            }
            DxfGeometrySegment::Arc(segment_arc) => {
                analytic_segments.push(segment.clone());
                if flattening_sag_tolerance_mm <= 0.0 {
                    return Err(error(
                        "flattening sag tolerance must be positive when curved segments are present",
                    ));
                }
                arc::sample_points_bounded(
                    *segment_arc,
                    flattening_sag_tolerance_mm,
                    max_segment_points,
                )
                .ok_or_else(|| {
                    error(format!(
                        "sampled source cycle exceeds the {GENERAL_MAX_RING_VERTICES}-vertex limit"
                    ))
                })?
            }
        };
        if points.len() < 2 {
            return Err(error(
                "every source segment must produce at least two contour points",
            ));
        }
        sampled_vertices = sampled_vertices.saturating_add(points.len() - 1);
        chains.push(points);
    }
    if chains.is_empty() {
        return Err(error(
            "general polygon source geometry has no contour segments",
        ));
    }

    let forward = assemble_cycle(chains.clone(), false).and_then(PolygonSet::from_outer);
    let reversed = assemble_cycle(chains, true).and_then(PolygonSet::from_outer);
    match (forward, reversed) {
        (Ok(forward), Ok(reversed)) if forward != reversed => Err(error(
            "source cycle has an ambiguous first-segment orientation after snapping",
        )),
        (Ok(polygon), _) | (_, Ok(polygon)) => {
            Ok(polygon.with_analytic_segments(analytic_segments))
        }
        (Err(error), Err(_)) => Err(error),
    }
}

fn assemble_cycle(
    mut chains: Vec<Vec<IrregularPoint>>,
    reverse_first: bool,
) -> Result<Vec<IrregularPoint>, GeneralPolygonError> {
    let mut points = chains.remove(0);
    if reverse_first {
        points.reverse();
    }
    for mut chain in chains {
        let tail = *points
            .last()
            .ok_or_else(|| error("general polygon source cycle is empty"))?;
        let starts_at_tail = same_grid_point(tail, chain[0])?;
        let ends_at_tail = same_grid_point(tail, *chain.last().expect("chain has two points"))?;
        match (starts_at_tail, ends_at_tail) {
            (true, false) => {}
            (false, true) => chain.reverse(),
            (true, true) => {
                return Err(error(
                    "source cycle has an ambiguous segment orientation after snapping",
                ));
            }
            (false, false) => {
                return Err(error(
                    "consecutive source endpoints must be identical after grid snapping",
                ));
            }
        }
        points.extend(chain.into_iter().skip(1));
        if points.len() > GENERAL_MAX_RING_VERTICES + 1 {
            return Err(error(format!(
                "sampled source cycle exceeds the {GENERAL_MAX_RING_VERTICES}-vertex ring limit"
            )));
        }
    }

    let first = points[0];
    let last = *points
        .last()
        .ok_or_else(|| error("general polygon source cycle is empty"))?;
    if !same_grid_point(first, last)? {
        return Err(error(
            "closing source endpoints must be identical after grid snapping",
        ));
    }
    points.pop();
    Ok(points)
}

fn same_grid_point(
    first: IrregularPoint,
    second: IrregularPoint,
) -> Result<bool, GeneralPolygonError> {
    let first = grid_point(first)?;
    let second = grid_point(second)?;
    Ok(first == second)
}

fn grid_point(point: IrregularPoint) -> Result<(i64, i64), GeneralPolygonError> {
    let x = to_grid_mm(point.x).ok_or_else(|| error("source x is outside the contractual grid"))?;
    let y = to_grid_mm(point.y).ok_or_else(|| error("source y is outside the contractual grid"))?;
    Ok((x as i64, y as i64))
}

fn error(message: impl Into<String>) -> GeneralPolygonError {
    GeneralPolygonError::from_message(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        DxfGeometryEntityType, DxfGeometrySummary, DxfLineSegment, PieceId, Rect, SourceFileId,
    };

    fn line(x1: f64, y1: f64, x2: f64, y2: f64) -> DxfGeometrySegment {
        DxfGeometrySegment::Line(DxfLineSegment {
            x1,
            y1,
            x2,
            y2,
            bulge: None,
            source_curve: None,
        })
    }

    fn piece(segments: Vec<DxfGeometrySegment>) -> ImportedPiece {
        ImportedPiece {
            id: PieceId::new("piece"),
            source_file_id: SourceFileId::new("source"),
            source_layer: None,
            label: "piece".to_owned(),
            real_bounds: Rect {
                x: 0.0,
                y: 0.0,
                width: 4.0,
                height: 4.0,
            },
            geometry: DxfGeometrySummary {
                entity_type: DxfGeometryEntityType::Lwpolyline,
                closed: true,
                segments,
            },
            warnings: Vec::new(),
        }
    }

    #[test]
    fn preserves_an_ordered_concave_cycle() {
        let source = piece(vec![
            line(0.0, 0.0, 4.0, 0.0),
            line(4.0, 0.0, 4.0, 1.0),
            line(4.0, 1.0, 1.0, 1.0),
            line(1.0, 1.0, 1.0, 4.0),
            line(1.0, 4.0, 0.0, 4.0),
            line(0.0, 4.0, 0.0, 0.0),
        ]);

        let polygon = polygon_set_from_imported_piece(&source, 0.01).unwrap();
        assert_eq!(polygon.area_mm2(), 7.0);
        assert!(!polygon.regions[0].outer.is_convex());
    }

    #[test]
    fn accepts_zero_sag_for_segment_only_geometry() {
        let source = piece(vec![
            line(0.0, 0.0, 4.0, 0.0),
            line(4.0, 0.0, 4.0, 4.0),
            line(4.0, 4.0, 0.0, 4.0),
            line(0.0, 4.0, 0.0, 0.0),
        ]);

        assert_eq!(
            polygon_set_from_imported_piece(&source, 0.0)
                .unwrap()
                .area_mm2(),
            16.0
        );
    }

    #[test]
    fn rejects_zero_sag_when_curved_geometry_requires_sampling() {
        let source = piece(vec![DxfGeometrySegment::Arc(
            crate::domain::DxfArcSegment {
                x1: 1.0,
                y1: 0.0,
                x2: 0.0,
                y2: 1.0,
                cx: 0.0,
                cy: 0.0,
                radius: 1.0,
                start_angle: 0.0,
                end_angle: 90.0,
            },
        )]);

        assert_eq!(
            polygon_set_from_imported_piece(&source, 0.0)
                .unwrap_err()
                .message(),
            "flattening sag tolerance must be positive when curved segments are present"
        );
    }

    #[test]
    fn reverses_individual_segments_without_global_reordering() {
        let source = piece(vec![
            line(0.0, 0.0, 2.0, 0.0),
            line(2.0, 2.0, 2.0, 0.0),
            line(0.0, 2.0, 2.0, 2.0),
            line(0.0, 0.0, 0.0, 2.0),
        ]);

        assert_eq!(
            polygon_set_from_imported_piece(&source, 0.01)
                .unwrap()
                .area_mm2(),
            4.0
        );
    }

    #[test]
    fn rejects_a_one_grid_unit_gap_instead_of_inventing_an_edge() {
        let source = piece(vec![
            line(0.0, 0.0, 2.0, 0.0),
            line(2.001, 0.0, 2.0, 2.0),
            line(2.0, 2.0, 0.0, 2.0),
            line(0.0, 2.0, 0.0, 0.0),
        ]);

        assert_eq!(
            polygon_set_from_imported_piece(&source, 0.01)
                .unwrap_err()
                .message(),
            "consecutive source endpoints must be identical after grid snapping"
        );
    }

    #[test]
    fn reverses_the_first_segment_when_that_is_the_only_connected_cycle() {
        let source = piece(vec![
            line(2.0, 0.0, 0.0, 0.0),
            line(2.0, 0.0, 2.0, 2.0),
            line(2.0, 2.0, 0.0, 2.0),
            line(0.0, 2.0, 0.0, 0.0),
        ]);

        assert_eq!(
            polygon_set_from_imported_piece(&source, 0.01)
                .unwrap()
                .area_mm2(),
            4.0
        );
    }

    #[test]
    fn rejects_an_arc_before_sampling_beyond_the_ring_limit() {
        let source = piece(vec![DxfGeometrySegment::Arc(
            crate::domain::DxfArcSegment {
                x1: 1_000_000.0,
                y1: 0.0,
                x2: 1_000_000.0,
                y2: 0.0,
                cx: 0.0,
                cy: 0.0,
                radius: 1_000_000.0,
                start_angle: 0.0,
                end_angle: 0.0,
            },
        )]);

        assert_eq!(
            polygon_set_from_imported_piece(&source, 0.000_001)
                .unwrap_err()
                .message(),
            "sampled source cycle exceeds the 2048-vertex limit"
        );
    }
}
