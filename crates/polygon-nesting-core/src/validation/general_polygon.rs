//! Independent publication validation for general polygon layouts.
//!
//! This module deliberately does not call Clipper, consume offset paths, or
//! reuse the search kernel's intersection result. It independently transforms
//! flattened source rings, checks robust boundary intersections and winding
//! containment, and measures explicit segment distances.

use std::fmt::{Display, Formatter};

use crate::domain::IrregularPoint;
use crate::geometry::general_polygon::{PolygonRing, PolygonSet};
use crate::geometry::predicates::orientation;

#[derive(Clone, Copy, Debug)]
pub struct GeneralPlacement<'a> {
    pub piece_id: &'a str,
    pub polygon: &'a PolygonSet,
    pub rotation_deg: f64,
    pub mirrored: bool,
    pub translate_x: f64,
    pub translate_y: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct PublicationValidationSettings {
    pub sheet_width_mm: f64,
    pub sheet_height_mm: f64,
    pub total_padding_mm: f64,
    pub sheet_edge_clearance_mm: Option<f64>,
    pub flattening_sag_tolerance_mm: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PublicationValidationError {
    message: String,
}

impl PublicationValidationError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for PublicationValidationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for PublicationValidationError {}

#[derive(Clone)]
struct MaterialRegion {
    outer: Vec<IrregularPoint>,
    holes: Vec<Vec<IrregularPoint>>,
    material_sample: Option<IrregularPoint>,
}

#[derive(Clone)]
struct MaterialSet {
    regions: Vec<MaterialRegion>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PointLocation {
    Outside,
    Boundary,
    Inside,
}

pub fn validate_publication(
    placements: &[GeneralPlacement<'_>],
    settings: PublicationValidationSettings,
) -> Result<(), PublicationValidationError> {
    validate_settings(settings)?;
    let transformed = placements
        .iter()
        .map(transform_placement)
        .collect::<Result<Vec<_>, _>>()?;
    let sheet_clearance = settings
        .sheet_edge_clearance_mm
        .unwrap_or(settings.total_padding_mm / 2.0)
        + settings.flattening_sag_tolerance_mm;
    for (placement, geometry) in placements.iter().zip(&transformed) {
        validate_sheet(placement.piece_id, geometry, settings, sheet_clearance)?;
    }

    let pair_clearance = settings.total_padding_mm + 2.0 * settings.flattening_sag_tolerance_mm;
    for first_index in 0..transformed.len() {
        for second_index in (first_index + 1)..transformed.len() {
            let first = &transformed[first_index];
            let second = &transformed[second_index];
            if material_sets_overlap(first, second) {
                return Err(PublicationValidationError::new(format!(
                    "pieces {} and {} overlap",
                    placements[first_index].piece_id, placements[second_index].piece_id
                )));
            }
            let distance = minimum_boundary_distance(first, second);
            if !distance.is_finite() || distance < pair_clearance {
                return Err(PublicationValidationError::new(format!(
                    "pieces {} and {} violate the required clearance",
                    placements[first_index].piece_id, placements[second_index].piece_id
                )));
            }
        }
    }
    Ok(())
}

fn validate_settings(
    settings: PublicationValidationSettings,
) -> Result<(), PublicationValidationError> {
    for (name, value) in [
        ("sheet width", settings.sheet_width_mm),
        ("sheet height", settings.sheet_height_mm),
        ("total padding", settings.total_padding_mm),
        (
            "flattening sag tolerance",
            settings.flattening_sag_tolerance_mm,
        ),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(PublicationValidationError::new(format!(
                "{name} must be finite and non-negative"
            )));
        }
    }
    if settings
        .sheet_edge_clearance_mm
        .is_some_and(|value| !value.is_finite() || value < 0.0)
    {
        return Err(PublicationValidationError::new(
            "sheet edge clearance must be finite and non-negative",
        ));
    }
    if settings.sheet_width_mm == 0.0 || settings.sheet_height_mm == 0.0 {
        return Err(PublicationValidationError::new(
            "sheet dimensions must be positive",
        ));
    }
    Ok(())
}

fn transform_placement(
    placement: &GeneralPlacement<'_>,
) -> Result<MaterialSet, PublicationValidationError> {
    if placement.polygon.is_empty() {
        return Err(PublicationValidationError::new(format!(
            "piece {} has empty material geometry",
            placement.piece_id
        )));
    }
    for value in [
        placement.rotation_deg,
        placement.translate_x,
        placement.translate_y,
    ] {
        if !value.is_finite() {
            return Err(PublicationValidationError::new(format!(
                "piece {} has a non-finite transform",
                placement.piece_id
            )));
        }
    }
    let radians = placement.rotation_deg.to_radians();
    let (sin, cos) = radians.sin_cos();
    let transform_ring =
        |ring: &PolygonRing| -> Result<Vec<IrregularPoint>, PublicationValidationError> {
            ring.source_points()
                .iter()
                .map(|point| {
                    let mirror_x = if placement.mirrored {
                        -point.x
                    } else {
                        point.x
                    };
                    let transformed = IrregularPoint::new(
                        mirror_x * cos - point.y * sin + placement.translate_x,
                        mirror_x * sin + point.y * cos + placement.translate_y,
                    );
                    if !transformed.x.is_finite() || !transformed.y.is_finite() {
                        return Err(PublicationValidationError::new(format!(
                            "piece {} transform produced a non-finite coordinate",
                            placement.piece_id
                        )));
                    }
                    Ok(transformed)
                })
                .collect()
        };

    Ok(MaterialSet {
        regions: placement
            .polygon
            .regions
            .iter()
            .map(|region| {
                let mut transformed = MaterialRegion {
                    outer: transform_ring(&region.outer)?,
                    holes: region
                        .holes
                        .iter()
                        .map(transform_ring)
                        .collect::<Result<Vec<_>, _>>()?,
                    material_sample: None,
                };
                transformed.material_sample = interior_sample(&transformed);
                if transformed.material_sample.is_none() {
                    return Err(PublicationValidationError::new(format!(
                        "piece {} has no independently discoverable material interior",
                        placement.piece_id
                    )));
                }
                Ok(transformed)
            })
            .collect::<Result<Vec<_>, PublicationValidationError>>()?,
    })
}

fn validate_sheet(
    piece_id: &str,
    geometry: &MaterialSet,
    settings: PublicationValidationSettings,
    clearance: f64,
) -> Result<(), PublicationValidationError> {
    for region in &geometry.regions {
        for point in &region.outer {
            if point.x < clearance
                || point.y < clearance
                || point.x > settings.sheet_width_mm - clearance
                || point.y > settings.sheet_height_mm - clearance
            {
                return Err(PublicationValidationError::new(format!(
                    "piece {piece_id} crosses the sheet clearance boundary"
                )));
            }
        }
    }
    Ok(())
}

fn material_sets_overlap(first: &MaterialSet, second: &MaterialSet) -> bool {
    for first_region in &first.regions {
        for second_region in &second.regions {
            if regions_overlap(first_region, second_region) {
                return true;
            }
        }
    }
    false
}

fn regions_overlap(first: &MaterialRegion, second: &MaterialRegion) -> bool {
    for first_ring in region_rings(first) {
        for second_ring in region_rings(second) {
            if rings_properly_cross(first_ring, second_ring) {
                return true;
            }
        }
    }

    has_material_sample_inside(first, second) || has_material_sample_inside(second, first)
}

fn has_material_sample_inside(source: &MaterialRegion, target: &MaterialRegion) -> bool {
    for ring in region_rings(source) {
        for index in 0..ring.len() {
            let start = ring[index];
            let end = ring[(index + 1) % ring.len()];
            let midpoint = IrregularPoint::new((start.x + end.x) / 2.0, (start.y + end.y) / 2.0);
            if classify_point_in_region(start, source) != PointLocation::Outside
                && classify_point_in_region(start, target) == PointLocation::Inside
            {
                return true;
            }
            if classify_point_in_region(midpoint, source) != PointLocation::Outside
                && classify_point_in_region(midpoint, target) == PointLocation::Inside
            {
                return true;
            }
        }
    }
    source
        .material_sample
        .is_some_and(|sample| classify_point_in_region(sample, target) == PointLocation::Inside)
}

fn interior_sample(region: &MaterialRegion) -> Option<IrregularPoint> {
    let mut y_levels = region_rings(region)
        .flat_map(|ring| ring.iter().map(|point| point.y))
        .collect::<Vec<_>>();
    y_levels.sort_by(f64::total_cmp);
    y_levels.dedup_by(|first, second| *first == *second);

    for levels in y_levels.windows(2) {
        let scan_y = levels[0] / 2.0 + levels[1] / 2.0;
        let mut intersections = Vec::new();
        for ring in region_rings(region) {
            for index in 0..ring.len() {
                let start = ring[index];
                let end = ring[(index + 1) % ring.len()];
                if (start.y < scan_y && end.y > scan_y) || (end.y < scan_y && start.y > scan_y) {
                    let parameter = (scan_y - start.y) / (end.y - start.y);
                    intersections.push(start.x + parameter * (end.x - start.x));
                }
            }
        }
        intersections.sort_by(f64::total_cmp);
        intersections.dedup_by(|first, second| *first == *second);
        for interval in intersections.windows(2) {
            let candidate = IrregularPoint::new(interval[0] / 2.0 + interval[1] / 2.0, scan_y);
            if classify_point_in_region(candidate, region) == PointLocation::Inside {
                return Some(candidate);
            }
        }
    }
    None
}

fn classify_point_in_region(point: IrregularPoint, region: &MaterialRegion) -> PointLocation {
    match classify_point_in_ring(point, &region.outer) {
        PointLocation::Outside => PointLocation::Outside,
        PointLocation::Boundary => PointLocation::Boundary,
        PointLocation::Inside => {
            for hole in &region.holes {
                match classify_point_in_ring(point, hole) {
                    PointLocation::Boundary => return PointLocation::Boundary,
                    PointLocation::Inside => return PointLocation::Outside,
                    PointLocation::Outside => {}
                }
            }
            PointLocation::Inside
        }
    }
}

fn classify_point_in_ring(point: IrregularPoint, ring: &[IrregularPoint]) -> PointLocation {
    let mut winding = 0i32;
    for index in 0..ring.len() {
        let start = ring[index];
        let end = ring[(index + 1) % ring.len()];
        let turn = orientation(start.x, start.y, end.x, end.y, point.x, point.y);
        if turn == 0 && point_on_segment(point, start, end) {
            return PointLocation::Boundary;
        }
        if start.y <= point.y {
            if end.y > point.y && turn > 0 {
                winding += 1;
            }
        } else if end.y <= point.y && turn < 0 {
            winding -= 1;
        }
    }
    if winding == 0 {
        PointLocation::Outside
    } else {
        PointLocation::Inside
    }
}

fn rings_properly_cross(first: &[IrregularPoint], second: &[IrregularPoint]) -> bool {
    for first_index in 0..first.len() {
        let first_start = first[first_index];
        let first_end = first[(first_index + 1) % first.len()];
        for second_index in 0..second.len() {
            let second_start = second[second_index];
            let second_end = second[(second_index + 1) % second.len()];
            let first_side_start = orientation(
                first_start.x,
                first_start.y,
                first_end.x,
                first_end.y,
                second_start.x,
                second_start.y,
            );
            let first_side_end = orientation(
                first_start.x,
                first_start.y,
                first_end.x,
                first_end.y,
                second_end.x,
                second_end.y,
            );
            let second_side_start = orientation(
                second_start.x,
                second_start.y,
                second_end.x,
                second_end.y,
                first_start.x,
                first_start.y,
            );
            let second_side_end = orientation(
                second_start.x,
                second_start.y,
                second_end.x,
                second_end.y,
                first_end.x,
                first_end.y,
            );
            if first_side_start * first_side_end < 0 && second_side_start * second_side_end < 0 {
                return true;
            }
        }
    }
    false
}

fn minimum_boundary_distance(first: &MaterialSet, second: &MaterialSet) -> f64 {
    let mut minimum = f64::INFINITY;
    for first_region in &first.regions {
        for first_ring in region_rings(first_region) {
            for second_region in &second.regions {
                for second_ring in region_rings(second_region) {
                    for first_index in 0..first_ring.len() {
                        let first_start = first_ring[first_index];
                        let first_end = first_ring[(first_index + 1) % first_ring.len()];
                        for second_index in 0..second_ring.len() {
                            let second_start = second_ring[second_index];
                            let second_end = second_ring[(second_index + 1) % second_ring.len()];
                            minimum = minimum.min(segment_distance(
                                first_start,
                                first_end,
                                second_start,
                                second_end,
                            ));
                        }
                    }
                }
            }
        }
    }
    minimum
}

fn segment_distance(
    first_start: IrregularPoint,
    first_end: IrregularPoint,
    second_start: IrregularPoint,
    second_end: IrregularPoint,
) -> f64 {
    if segments_touch_or_cross(first_start, first_end, second_start, second_end) {
        return 0.0;
    }
    point_segment_distance(first_start, second_start, second_end)
        .min(point_segment_distance(first_end, second_start, second_end))
        .min(point_segment_distance(second_start, first_start, first_end))
        .min(point_segment_distance(second_end, first_start, first_end))
}

fn segments_touch_or_cross(
    first_start: IrregularPoint,
    first_end: IrregularPoint,
    second_start: IrregularPoint,
    second_end: IrregularPoint,
) -> bool {
    let first_side_start = orientation(
        first_start.x,
        first_start.y,
        first_end.x,
        first_end.y,
        second_start.x,
        second_start.y,
    );
    let first_side_end = orientation(
        first_start.x,
        first_start.y,
        first_end.x,
        first_end.y,
        second_end.x,
        second_end.y,
    );
    let second_side_start = orientation(
        second_start.x,
        second_start.y,
        second_end.x,
        second_end.y,
        first_start.x,
        first_start.y,
    );
    let second_side_end = orientation(
        second_start.x,
        second_start.y,
        second_end.x,
        second_end.y,
        first_end.x,
        first_end.y,
    );
    if first_side_start * first_side_end < 0 && second_side_start * second_side_end < 0 {
        return true;
    }
    (first_side_start == 0 && point_on_segment(second_start, first_start, first_end))
        || (first_side_end == 0 && point_on_segment(second_end, first_start, first_end))
        || (second_side_start == 0 && point_on_segment(first_start, second_start, second_end))
        || (second_side_end == 0 && point_on_segment(first_end, second_start, second_end))
}

fn point_on_segment(point: IrregularPoint, start: IrregularPoint, end: IrregularPoint) -> bool {
    point.x >= start.x.min(end.x)
        && point.x <= start.x.max(end.x)
        && point.y >= start.y.min(end.y)
        && point.y <= start.y.max(end.y)
}

fn point_segment_distance(
    point: IrregularPoint,
    start: IrregularPoint,
    end: IrregularPoint,
) -> f64 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    if length_squared == 0.0 {
        return (point.x - start.x).hypot(point.y - start.y);
    }
    let projection =
        (((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared).clamp(0.0, 1.0);
    let closest_x = start.x + projection * dx;
    let closest_y = start.y + projection * dy;
    (point.x - closest_x).hypot(point.y - closest_y)
}

fn region_rings(region: &MaterialRegion) -> impl Iterator<Item = &[IrregularPoint]> {
    std::iter::once(region.outer.as_slice()).chain(region.holes.iter().map(Vec::as_slice))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64) -> IrregularPoint {
        IrregularPoint::new(x, y)
    }

    fn square(side: f64) -> PolygonSet {
        PolygonSet::from_outer(vec![
            point(0.0, 0.0),
            point(side, 0.0),
            point(side, side),
            point(0.0, side),
        ])
        .unwrap()
    }

    fn settings() -> PublicationValidationSettings {
        PublicationValidationSettings {
            sheet_width_mm: 20.0,
            sheet_height_mm: 20.0,
            total_padding_mm: 0.0,
            sheet_edge_clearance_mm: None,
            flattening_sag_tolerance_mm: 0.0,
        }
    }

    #[test]
    fn boundary_contact_is_legal_but_positive_overlap_is_not() {
        let piece = square(2.0);
        let touching = [
            GeneralPlacement {
                piece_id: "a",
                polygon: &piece,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 0.0,
                translate_y: 0.0,
            },
            GeneralPlacement {
                piece_id: "b",
                polygon: &piece,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 2.0,
                translate_y: 0.0,
            },
        ];
        assert!(validate_publication(&touching, settings()).is_ok());

        let overlapping = [GeneralPlacement {
            translate_x: 1.5,
            ..touching[1]
        }];
        assert!(validate_publication(&[touching[0], overlapping[0]], settings()).is_err());
    }

    #[test]
    fn arbitrary_rotation_is_validated_without_clipper() {
        let piece = square(2.0);
        let placements = [GeneralPlacement {
            piece_id: "rotated",
            polygon: &piece,
            rotation_deg: 17.5,
            mirrored: false,
            translate_x: 5.0,
            translate_y: 5.0,
        }];
        assert!(validate_publication(&placements, settings()).is_ok());
    }

    #[test]
    fn explicit_clearance_is_enforced() {
        let piece = square(2.0);
        let placements = [
            GeneralPlacement {
                piece_id: "a",
                polygon: &piece,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 1.0,
                translate_y: 1.0,
            },
            GeneralPlacement {
                piece_id: "b",
                polygon: &piece,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 3.5,
                translate_y: 1.0,
            },
        ];
        let clearance_settings = PublicationValidationSettings {
            total_padding_mm: 1.0,
            ..settings()
        };
        assert!(validate_publication(&placements, clearance_settings).is_err());
    }

    #[test]
    fn sheet_edge_clearance_is_independent_from_pair_clearance() {
        let piece = square(2.0);
        let placement = [GeneralPlacement {
            piece_id: "edge",
            polygon: &piece,
            rotation_deg: 0.0,
            mirrored: false,
            translate_x: 2.0,
            translate_y: 2.0,
        }];
        let explicit_edge = PublicationValidationSettings {
            total_padding_mm: 1.0,
            sheet_edge_clearance_mm: Some(3.0),
            ..settings()
        };

        assert!(validate_publication(&placement, explicit_edge).is_err());
        assert!(validate_publication(
            &placement,
            PublicationValidationSettings {
                sheet_edge_clearance_mm: None,
                ..explicit_edge
            }
        )
        .is_ok());
    }

    #[test]
    fn sub_grid_source_overlap_is_not_hidden_by_search_snapping() {
        let piece = PolygonSet::from_outer(vec![
            point(0.0, 0.0),
            point(1.0004, 0.0),
            point(1.0004, 1.0),
            point(0.0, 1.0),
        ])
        .unwrap();
        let placements = [
            GeneralPlacement {
                piece_id: "a",
                polygon: &piece,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 0.0,
                translate_y: 0.0,
            },
            GeneralPlacement {
                piece_id: "b",
                polygon: &piece,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 1.0,
                translate_y: 0.0,
            },
        ];
        assert!(validate_publication(&placements, settings()).is_err());
    }

    #[test]
    fn rectangle_inside_l_shape_notch_is_legal() {
        let l_shape = PolygonSet::from_outer(vec![
            point(0.0, 0.0),
            point(4.0, 0.0),
            point(4.0, 1.0),
            point(1.0, 1.0),
            point(1.0, 4.0),
            point(0.0, 4.0),
        ])
        .unwrap();
        let pocket = square(2.5);
        let placements = [
            GeneralPlacement {
                piece_id: "l",
                polygon: &l_shape,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 0.0,
                translate_y: 0.0,
            },
            GeneralPlacement {
                piece_id: "pocket",
                polygon: &pocket,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 1.0,
                translate_y: 1.0,
            },
        ];
        assert!(validate_publication(&placements, settings()).is_ok());
    }

    #[test]
    fn coincident_mirrored_material_is_rejected() {
        let piece = square(2.0);
        let placements = [
            GeneralPlacement {
                piece_id: "normal",
                polygon: &piece,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 2.0,
                translate_y: 2.0,
            },
            GeneralPlacement {
                piece_id: "mirrored",
                polygon: &piece,
                rotation_deg: 0.0,
                mirrored: true,
                translate_x: 4.0,
                translate_y: 2.0,
            },
        ];
        assert!(validate_publication(&placements, settings()).is_err());
    }

    #[test]
    fn coincident_holed_material_is_rejected() {
        let donut = PolygonSet::new(vec![crate::geometry::general_polygon::PolygonRegion::new(
            vec![
                point(0.0, 0.0),
                point(6.0, 0.0),
                point(6.0, 6.0),
                point(0.0, 6.0),
            ],
            vec![vec![
                point(2.0, 2.0),
                point(2.0, 4.0),
                point(4.0, 4.0),
                point(4.0, 2.0),
            ]],
        )
        .unwrap()])
        .unwrap();
        let placements = [
            GeneralPlacement {
                piece_id: "first",
                polygon: &donut,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 2.0,
                translate_y: 2.0,
            },
            GeneralPlacement {
                piece_id: "second",
                polygon: &donut,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 2.0,
                translate_y: 2.0,
            },
        ];
        assert!(validate_publication(&placements, settings()).is_err());
    }

    #[test]
    fn empty_material_is_rejected() {
        let empty = PolygonSet::empty();
        let placements = [GeneralPlacement {
            piece_id: "empty",
            polygon: &empty,
            rotation_deg: 0.0,
            mirrored: false,
            translate_x: 0.0,
            translate_y: 0.0,
        }];
        assert!(validate_publication(&placements, settings()).is_err());
    }
}
