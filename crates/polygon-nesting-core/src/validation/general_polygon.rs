//! Independent publication validation for general polygon layouts.
//!
//! This module deliberately does not call Clipper, consume offset paths, or
//! reuse the search kernel's intersection result. It independently transforms
//! flattened source rings, checks robust boundary intersections and winding
//! containment, and measures explicit segment distances.

use std::fmt::{Display, Formatter};

use crate::canonical_grid::{from_grid, to_grid_mm};
use crate::domain::{DxfGeometrySegment, IrregularPoint};
use crate::geometry::general_polygon::{PolygonRing, PolygonSet};
use crate::geometry::predicates::orientation;
use crate::transforms::flattening::{arc, ellipse};

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
    boundaries: Vec<BoundarySegment>,
}

#[derive(Clone)]
struct BoundarySegment {
    points: Vec<IrregularPoint>,
    point_sag_bounds_mm: Vec<f64>,
    interior_sag_bound_mm: f64,
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
        .map(|placement| transform_placement(placement, settings.flattening_sag_tolerance_mm))
        .collect::<Result<Vec<_>, _>>()?;
    // the requested edge clearance is a public geometric contract. Curve
    // flattening tolerance is kept as an internal source-ingress diagnostic;
    // it must not silently increase the distance requested by the caller.
    let sheet_clearance = settings
        .sheet_edge_clearance_mm
        .unwrap_or(settings.total_padding_mm / 2.0);
    for (placement, geometry) in placements.iter().zip(&transformed) {
        validate_sheet(placement.piece_id, geometry, settings, sheet_clearance)?;
    }

    let pair_clearance = settings.total_padding_mm;
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
            if !boundaries_meet_clearance(first, second, pair_clearance) {
                return Err(PublicationValidationError::new(format!(
                    "pieces {} and {} violate the required clearance",
                    placements[first_index].piece_id, placements[second_index].piece_id
                )));
            }
        }
    }
    Ok(())
}

fn boundaries_meet_clearance(first: &MaterialSet, second: &MaterialSet, clearance_mm: f64) -> bool {
    if !clearance_mm.is_finite() || clearance_mm < 0.0 {
        return false;
    }
    for first_segment in &first.boundaries {
        for second_segment in &second.boundaries {
            for (first_index, first_pair) in first_segment.points.windows(2).enumerate() {
                for (second_index, second_pair) in second_segment.points.windows(2).enumerate() {
                    if !segment_meets_clearance_with_sag(
                        first_pair[0],
                        first_pair[1],
                        second_pair[0],
                        second_pair[1],
                        clearance_mm,
                        first_segment.point_sag_bounds_mm[first_index],
                        first_segment.point_sag_bounds_mm[first_index + 1],
                        first_segment.interior_sag_bound_mm,
                        second_segment.point_sag_bounds_mm[second_index],
                        second_segment.point_sag_bounds_mm[second_index + 1],
                        second_segment.interior_sag_bound_mm,
                    ) {
                        return false;
                    }
                }
            }
        }
    }
    true
}

pub(crate) fn transformed_material_sets_meet_clearance(
    first: GeneralPlacement<'_>,
    second: GeneralPlacement<'_>,
    clearance_mm: f64,
    flattening_sag_tolerance_mm: f64,
) -> Result<bool, PublicationValidationError> {
    if !clearance_mm.is_finite() || clearance_mm < 0.0 {
        return Ok(false);
    }
    let first = transform_placement(&first, flattening_sag_tolerance_mm)?;
    let second = transform_placement(&second, flattening_sag_tolerance_mm)?;
    if material_sets_overlap(&first, &second) {
        return Ok(false);
    }
    Ok(boundaries_meet_clearance(&first, &second, clearance_mm))
}

pub(crate) fn transformed_material_set_fits_sheet(
    placement: GeneralPlacement<'_>,
    sheet_width_mm: f64,
    sheet_height_mm: f64,
    clearance_mm: f64,
    flattening_sag_tolerance_mm: f64,
) -> Result<bool, PublicationValidationError> {
    let geometry = transform_placement(&placement, flattening_sag_tolerance_mm)?;
    Ok(material_set_fits_sheet(
        &geometry,
        sheet_width_mm,
        sheet_height_mm,
        clearance_mm,
    ))
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
    flattening_sag_tolerance_mm: f64,
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

    let regions = placement
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
        .collect::<Result<Vec<_>, PublicationValidationError>>()?;
    let boundaries = if let Some(segments) = &placement.polygon.analytic_segments {
        let sag = flattening_sag_tolerance_mm.max(1e-9);
        segments
            .iter()
            .map(|segment| {
                let (points, sag_bound_mm, endpoint_sag_bound_mm) = match segment {
                    DxfGeometrySegment::Line(line) => {
                        if let Some(source_curve) = &line.source_curve {
                            (ellipse::sample_points(source_curve, sag), sag, 0.0)
                        } else if line.bulge.is_some_and(|bulge| bulge != 0.0) {
                            (arc::sample_bulge_points(line, sag), sag, 0.0)
                        } else {
                            (
                                vec![
                                    IrregularPoint::new(line.x1, line.y1),
                                    IrregularPoint::new(line.x2, line.y2),
                                ],
                                0.0,
                                0.0,
                            )
                        }
                    }
                    DxfGeometrySegment::Arc(arc_segment) => (
                        arc::sample_points(*arc_segment, sag),
                        arc::certified_sag_bound_mm(*arc_segment, sag),
                        arc::endpoint_mismatch_mm(*arc_segment),
                    ),
                };
                if points.len() < 2 {
                    return Err(PublicationValidationError::new(format!(
                        "piece {} has an invalid analytic boundary segment",
                        placement.piece_id
                    )));
                }
                let transformed = points
                    .into_iter()
                    .map(|point| transform_point(point, placement, sin, cos))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut point_sag_bounds_mm = vec![sag_bound_mm; transformed.len()];
                if let Some(first) = point_sag_bounds_mm.first_mut() {
                    *first = endpoint_sag_bound_mm;
                }
                if let Some(last) = point_sag_bounds_mm.last_mut() {
                    *last = endpoint_sag_bound_mm;
                }
                Ok(BoundarySegment {
                    points: transformed,
                    point_sag_bounds_mm,
                    interior_sag_bound_mm: sag_bound_mm,
                })
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        regions
            .iter()
            .flat_map(|region| region_rings(region))
            .map(|ring| BoundarySegment {
                points: ring.to_vec(),
                point_sag_bounds_mm: vec![0.0; ring.len()],
                interior_sag_bound_mm: 0.0,
            })
            .collect()
    };

    Ok(MaterialSet {
        regions,
        boundaries,
    })
}

fn transform_point(
    point: IrregularPoint,
    placement: &GeneralPlacement<'_>,
    sin: f64,
    cos: f64,
) -> Result<IrregularPoint, PublicationValidationError> {
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
}

fn validate_sheet(
    piece_id: &str,
    geometry: &MaterialSet,
    settings: PublicationValidationSettings,
    clearance: f64,
) -> Result<(), PublicationValidationError> {
    if !outer_points_fit_sheet(
        geometry,
        settings.sheet_width_mm,
        settings.sheet_height_mm,
        clearance,
    ) {
        return Err(PublicationValidationError::new(format!(
            "piece {piece_id} crosses the sheet clearance boundary"
        )));
    }
    if !analytic_boundaries_fit_sheet(
        geometry,
        settings.sheet_width_mm,
        settings.sheet_height_mm,
        clearance,
    ) {
        return Err(PublicationValidationError::new(format!(
            "piece {piece_id} crosses the analytic sheet clearance boundary"
        )));
    }
    Ok(())
}

fn material_set_fits_sheet(
    geometry: &MaterialSet,
    sheet_width_mm: f64,
    sheet_height_mm: f64,
    clearance: f64,
) -> bool {
    outer_points_fit_sheet(geometry, sheet_width_mm, sheet_height_mm, clearance)
        && analytic_boundaries_fit_sheet(geometry, sheet_width_mm, sheet_height_mm, clearance)
}

fn outer_points_fit_sheet(
    geometry: &MaterialSet,
    sheet_width_mm: f64,
    sheet_height_mm: f64,
    clearance: f64,
) -> bool {
    geometry.regions.iter().all(|region| {
        region.outer.iter().all(|point| {
            point.x >= clearance
                && point.y >= clearance
                && point.x <= sheet_width_mm - clearance
                && point.y <= sheet_height_mm - clearance
        })
    })
}

fn analytic_boundaries_fit_sheet(
    geometry: &MaterialSet,
    sheet_width_mm: f64,
    sheet_height_mm: f64,
    clearance: f64,
) -> bool {
    geometry.boundaries.iter().all(|segment| {
        segment
            .points
            .iter()
            .zip(&segment.point_sag_bounds_mm)
            .all(|(point, sag_bound_mm)| {
                point.x - sag_bound_mm >= clearance
                    && point.y - sag_bound_mm >= clearance
                    && point.x + sag_bound_mm <= sheet_width_mm - clearance
                    && point.y + sag_bound_mm <= sheet_height_mm - clearance
            })
    })
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

#[cfg(test)]
fn segment_meets_clearance(
    first_start: IrregularPoint,
    first_end: IrregularPoint,
    second_start: IrregularPoint,
    second_end: IrregularPoint,
    clearance_mm: f64,
) -> bool {
    segment_meets_clearance_with_sag(
        first_start,
        first_end,
        second_start,
        second_end,
        clearance_mm,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    )
}

#[allow(clippy::too_many_arguments)]
fn segment_meets_clearance_with_sag(
    first_start: IrregularPoint,
    first_end: IrregularPoint,
    second_start: IrregularPoint,
    second_end: IrregularPoint,
    clearance_mm: f64,
    first_start_sag_mm: f64,
    first_end_sag_mm: f64,
    first_interior_sag_mm: f64,
    second_start_sag_mm: f64,
    second_end_sag_mm: f64,
    second_interior_sag_mm: f64,
) -> bool {
    if [
        clearance_mm,
        first_start_sag_mm,
        first_end_sag_mm,
        first_interior_sag_mm,
        second_start_sag_mm,
        second_end_sag_mm,
        second_interior_sag_mm,
    ]
    .iter()
    .any(|value| !value.is_finite() || *value < 0.0)
    {
        return false;
    }
    if segments_touch_or_cross(first_start, first_end, second_start, second_end) {
        return clearance_mm == 0.0;
    }
    point_segment_meets_clearance_with_sag(
        first_start,
        first_start_sag_mm,
        second_start,
        second_end,
        second_start_sag_mm,
        second_end_sag_mm,
        second_interior_sag_mm,
        clearance_mm,
    ) && point_segment_meets_clearance_with_sag(
        first_end,
        first_end_sag_mm,
        second_start,
        second_end,
        second_start_sag_mm,
        second_end_sag_mm,
        second_interior_sag_mm,
        clearance_mm,
    ) && point_segment_meets_clearance_with_sag(
        second_start,
        second_start_sag_mm,
        first_start,
        first_end,
        first_start_sag_mm,
        first_end_sag_mm,
        first_interior_sag_mm,
        clearance_mm,
    ) && point_segment_meets_clearance_with_sag(
        second_end,
        second_end_sag_mm,
        first_start,
        first_end,
        first_start_sag_mm,
        first_end_sag_mm,
        first_interior_sag_mm,
        clearance_mm,
    )
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

fn point_segment_meets_clearance_with_sag(
    point: IrregularPoint,
    point_sag_mm: f64,
    start: IrregularPoint,
    end: IrregularPoint,
    start_sag_mm: f64,
    end_sag_mm: f64,
    interior_sag_mm: f64,
    clearance_mm: f64,
) -> bool {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx.mul_add(dx, dy * dy);
    if length_squared == 0.0 {
        return squared_distance_meets_clearance(
            point,
            start,
            clearance_mm,
            &[point_sag_mm, start_sag_mm],
            exact_grid_point_distance(point, start, clearance_mm),
        );
    }
    let projection =
        (((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared).clamp(0.0, 1.0);
    if projection == 0.0 {
        return squared_distance_meets_clearance(
            point,
            start,
            clearance_mm,
            &[point_sag_mm, start_sag_mm],
            exact_grid_point_distance(point, start, clearance_mm),
        );
    }
    if projection == 1.0 {
        return squared_distance_meets_clearance(
            point,
            end,
            clearance_mm,
            &[point_sag_mm, end_sag_mm],
            exact_grid_point_distance(point, end, clearance_mm),
        );
    }
    let closest_x = start.x + projection * dx;
    let closest_y = start.y + projection * dy;
    let closest = IrregularPoint::new(closest_x, closest_y);
    let exact = exact_grid_point_segment_distance(point, start, end, clearance_mm);
    squared_distance_meets_clearance(
        point,
        closest,
        clearance_mm,
        &[point_sag_mm, interior_sag_mm],
        exact,
    )
}

fn squared_distance_meets_clearance(
    first: IrregularPoint,
    second: IrregularPoint,
    clearance_mm: f64,
    additional_clearance_terms: &[f64],
    exact_grid_result: Option<bool>,
) -> bool {
    if additional_clearance_terms.iter().all(|term| *term == 0.0) {
        if let Some(exact) = exact_grid_result {
            return exact;
        }
    }
    if !clearance_mm.is_finite()
        || clearance_mm < 0.0
        || additional_clearance_terms
            .iter()
            .any(|term| !term.is_finite() || *term < 0.0)
    {
        return false;
    }
    let mut clearance_terms = Vec::with_capacity(additional_clearance_terms.len() + 1);
    clearance_terms.push(clearance_mm);
    clearance_terms.extend_from_slice(additional_clearance_terms);
    let Some(exact) = exact_squared_distance_meets_clearance(first, second, &clearance_terms)
    else {
        return false;
    };
    exact
}

fn exact_squared_distance_meets_clearance(
    first: IrregularPoint,
    second: IrregularPoint,
    clearance_terms: &[f64],
) -> Option<bool> {
    let dx = exact_difference(first.x, second.x)?;
    let dy = exact_difference(first.y, second.y)?;
    let dx_squared = exact_product(&dx, &dx)?;
    let dy_squared = exact_product(&dy, &dy)?;
    let distance_squared = exact_sum_expansions(&[&dx_squared, &dy_squared])?;
    let required = exact_sum(clearance_terms)?;
    let required_squared = exact_product(&required, &required)?;
    let difference = exact_difference_expansions(&distance_squared, &required_squared)?;
    Some(expansion_sign(&difference) >= 0)
}

fn exact_difference(first: f64, second: f64) -> Option<Vec<f64>> {
    if !first.is_finite() || !second.is_finite() {
        return None;
    }
    let difference = first - second;
    if !difference.is_finite() {
        return None;
    }
    let second_virtual = first - difference;
    let first_virtual = difference + second_virtual;
    let second_roundoff = second_virtual - second;
    let first_roundoff = first - first_virtual;
    let error = first_roundoff + second_roundoff;
    exact_sum(&[difference, error])
}

fn exact_product(first: &[f64], second: &[f64]) -> Option<Vec<f64>> {
    let mut terms = Vec::with_capacity(first.len() * second.len() * 2);
    for &left in first {
        for &right in second {
            let product = left * right;
            if !product.is_finite() {
                return None;
            }
            let error = left.mul_add(right, -product);
            if !error.is_finite() {
                return None;
            }
            terms.push(error);
            terms.push(product);
        }
    }
    exact_sum(&terms)
}

fn exact_difference_expansions(first: &[f64], second: &[f64]) -> Option<Vec<f64>> {
    let mut terms = Vec::with_capacity(first.len() + second.len());
    terms.extend_from_slice(first);
    terms.extend(second.iter().map(|term| -*term));
    exact_sum(&terms)
}

fn exact_sum_expansions(expansions: &[&[f64]]) -> Option<Vec<f64>> {
    let terms = expansions
        .iter()
        .flat_map(|expansion| expansion.iter().copied())
        .collect::<Vec<_>>();
    exact_sum(&terms)
}

fn exact_sum(terms: &[f64]) -> Option<Vec<f64>> {
    if terms.iter().any(|term| !term.is_finite()) {
        return None;
    }
    let mut sorted = terms
        .iter()
        .copied()
        .filter(|term| *term != 0.0)
        .collect::<Vec<_>>();
    sorted.sort_by(|first, second| first.abs().total_cmp(&second.abs()));

    let mut expansion = Vec::new();
    for term in sorted {
        let mut next = Vec::with_capacity(expansion.len() + 1);
        let mut accumulator = term;
        for component in expansion {
            let (sum, error) = two_sum(accumulator, component);
            if error != 0.0 {
                next.push(error);
            }
            accumulator = sum;
        }
        if accumulator != 0.0 {
            next.push(accumulator);
        }
        expansion = next;
    }
    Some(expansion)
}

fn two_sum(first: f64, second: f64) -> (f64, f64) {
    let sum = first + second;
    let second_virtual = sum - first;
    let first_virtual = sum - second_virtual;
    let first_roundoff = first - first_virtual;
    let second_roundoff = second - second_virtual;
    (sum, first_roundoff + second_roundoff)
}

fn expansion_sign(expansion: &[f64]) -> i8 {
    expansion
        .iter()
        .rev()
        .find(|term| **term != 0.0)
        .map_or(0, |term| if *term > 0.0 { 1 } else { -1 })
}

fn exact_grid_coordinate(value: f64) -> Option<i128> {
    let grid = to_grid_mm(value)?;
    if from_grid(grid) == value {
        Some(grid as i128)
    } else {
        None
    }
}

fn exact_grid_point_distance(
    first: IrregularPoint,
    second: IrregularPoint,
    clearance_mm: f64,
) -> Option<bool> {
    let first_x = exact_grid_coordinate(first.x)?;
    let first_y = exact_grid_coordinate(first.y)?;
    let second_x = exact_grid_coordinate(second.x)?;
    let second_y = exact_grid_coordinate(second.y)?;
    let dx = first_x.checked_sub(second_x)?;
    let dy = first_y.checked_sub(second_y)?;
    let numerator = dx.checked_mul(dx)?.checked_add(dy.checked_mul(dy)?)?;
    let required = exact_grid_coordinate(clearance_mm)?;
    let required_squared = required.checked_mul(required)?;
    Some(numerator >= required_squared)
}

fn exact_grid_point_segment_distance(
    point: IrregularPoint,
    start: IrregularPoint,
    end: IrregularPoint,
    clearance_mm: f64,
) -> Option<bool> {
    let px = exact_grid_coordinate(point.x)?;
    let py = exact_grid_coordinate(point.y)?;
    let ax = exact_grid_coordinate(start.x)?;
    let ay = exact_grid_coordinate(start.y)?;
    let bx = exact_grid_coordinate(end.x)?;
    let by = exact_grid_coordinate(end.y)?;
    let required = exact_grid_coordinate(clearance_mm)?;
    let dx = bx.checked_sub(ax)?;
    let dy = by.checked_sub(ay)?;
    let point_dx = px.checked_sub(ax)?;
    let point_dy = py.checked_sub(ay)?;
    let length_squared = dx.checked_mul(dx)?.checked_add(dy.checked_mul(dy)?)?;
    let dot = point_dx
        .checked_mul(dx)?
        .checked_add(point_dy.checked_mul(dy)?)?;
    let distance_numerator = if dot <= 0 {
        point_dx
            .checked_mul(point_dx)?
            .checked_add(point_dy.checked_mul(point_dy)?)?
    } else if dot >= length_squared {
        let end_dx = px.checked_sub(bx)?;
        let end_dy = py.checked_sub(by)?;
        end_dx
            .checked_mul(end_dx)?
            .checked_add(end_dy.checked_mul(end_dy)?)?
    } else {
        let cross = point_dx
            .checked_mul(dy)?
            .checked_sub(point_dy.checked_mul(dx)?)?;
        cross.checked_mul(cross)?
    };
    let required_squared = required.checked_mul(required)?;
    if dot > 0 && dot < length_squared {
        required_squared
            .checked_mul(length_squared)
            .map(|threshold| distance_numerator >= threshold)
    } else {
        Some(distance_numerator >= required_squared)
    }
}

fn region_rings(region: &MaterialRegion) -> impl Iterator<Item = &[IrregularPoint]> {
    std::iter::once(region.outer.as_slice()).chain(region.holes.iter().map(Vec::as_slice))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DxfArcSegment, DxfLineSegment};

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
    fn requested_five_millimetres_is_exactly_accepted_for_non_grid_coordinates() {
        let piece = square(2.0);
        let placements = [
            GeneralPlacement {
                piece_id: "a",
                polygon: &piece,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 5.0004,
                translate_y: 5.0,
            },
            GeneralPlacement {
                piece_id: "b",
                polygon: &piece,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 12.0004,
                translate_y: 5.0,
            },
        ];
        let exact_settings = PublicationValidationSettings {
            total_padding_mm: 5.0,
            sheet_edge_clearance_mm: Some(5.0),
            ..settings()
        };

        assert!(validate_publication(&placements, exact_settings).is_ok());
    }

    #[test]
    fn candidate_sheet_check_includes_analytic_boundary_points() {
        let piece =
            square(2.0).with_analytic_segments(vec![DxfGeometrySegment::Line(DxfLineSegment {
                x1: -1.0,
                y1: 0.0,
                x2: -1.0,
                y2: 2.0,
                bulge: None,
                source_curve: None,
            })]);
        let placement = GeneralPlacement {
            piece_id: "analytic-point",
            polygon: &piece,
            rotation_deg: 0.0,
            mirrored: false,
            translate_x: 1.0,
            translate_y: 1.0,
        };
        let settings = PublicationValidationSettings {
            sheet_width_mm: 10.0,
            sheet_height_mm: 10.0,
            sheet_edge_clearance_mm: Some(0.5),
            ..settings()
        };

        assert!(!transformed_material_set_fits_sheet(
            placement,
            settings.sheet_width_mm,
            settings.sheet_height_mm,
            settings.sheet_edge_clearance_mm.unwrap(),
            settings.flattening_sag_tolerance_mm,
        )
        .unwrap());
        assert!(validate_publication(&[placement], settings).is_err());
    }

    #[test]
    fn candidate_sheet_check_includes_analytic_sag_bounds() {
        let piece =
            square(2.0).with_analytic_segments(vec![DxfGeometrySegment::Arc(DxfArcSegment {
                x1: 0.0,
                y1: 2.0,
                x2: 0.0,
                y2: 0.0,
                cx: 0.0,
                cy: 1.0,
                radius: 1.0,
                start_angle: 90.0,
                end_angle: 270.0,
            })]);
        let placement = GeneralPlacement {
            piece_id: "analytic-sag",
            polygon: &piece,
            rotation_deg: 0.0,
            mirrored: false,
            translate_x: 1.0,
            translate_y: 1.0,
        };
        let settings = PublicationValidationSettings {
            sheet_width_mm: 10.0,
            sheet_height_mm: 10.0,
            sheet_edge_clearance_mm: Some(0.75),
            flattening_sag_tolerance_mm: 0.5,
            ..settings()
        };

        assert!(!transformed_material_set_fits_sheet(
            placement,
            settings.sheet_width_mm,
            settings.sheet_height_mm,
            settings.sheet_edge_clearance_mm.unwrap(),
            settings.flattening_sag_tolerance_mm,
        )
        .unwrap());
        assert!(validate_publication(&[placement], settings).is_err());
    }

    #[test]
    fn exact_analytic_curve_endpoint_at_sheet_clearance_is_accepted() {
        let start_angle = -90.0;
        let end_angle = 90.0;
        let start_radians = (start_angle * std::f64::consts::PI) / 180.0;
        let end_radians = (end_angle * std::f64::consts::PI) / 180.0;
        let piece =
            square(2.0).with_analytic_segments(vec![DxfGeometrySegment::Arc(DxfArcSegment {
                x1: libm::cos(start_radians),
                y1: 1.0 + libm::sin(start_radians),
                x2: libm::cos(end_radians),
                y2: 1.0 + libm::sin(end_radians),
                cx: 0.0,
                cy: 1.0,
                radius: 1.0,
                start_angle,
                end_angle,
            })]);
        let placement = GeneralPlacement {
            piece_id: "analytic-endpoint",
            polygon: &piece,
            rotation_deg: 0.0,
            mirrored: false,
            translate_x: 1.0,
            translate_y: 1.0,
        };
        let settings = PublicationValidationSettings {
            sheet_width_mm: 10.0,
            sheet_height_mm: 10.0,
            sheet_edge_clearance_mm: Some(1.0),
            flattening_sag_tolerance_mm: 0.5,
            ..settings()
        };

        assert!(transformed_material_set_fits_sheet(
            placement,
            settings.sheet_width_mm,
            settings.sheet_height_mm,
            settings.sheet_edge_clearance_mm.unwrap(),
            settings.flattening_sag_tolerance_mm,
        )
        .unwrap());
        assert!(validate_publication(&[placement], settings).is_ok());
    }

    #[test]
    fn pair_clearance_just_below_five_millimetres_is_rejected() {
        let piece = square(2.0);
        let placements = [
            GeneralPlacement {
                piece_id: "a",
                polygon: &piece,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 5.0,
                translate_y: 5.0,
            },
            GeneralPlacement {
                piece_id: "b",
                polygon: &piece,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 11.999,
                translate_y: 5.0,
            },
        ];
        let exact_settings = PublicationValidationSettings {
            total_padding_mm: 5.0,
            sheet_edge_clearance_mm: Some(5.0),
            ..settings()
        };

        assert!(validate_publication(&placements, exact_settings).is_err());
    }

    #[test]
    fn sheet_clearance_just_below_five_millimetres_is_rejected() {
        let piece = square(2.0);
        let placement = [GeneralPlacement {
            piece_id: "edge",
            polygon: &piece,
            rotation_deg: 0.0,
            mirrored: false,
            translate_x: 4.999,
            translate_y: 5.0,
        }];
        let exact_settings = PublicationValidationSettings {
            total_padding_mm: 5.0,
            sheet_edge_clearance_mm: Some(5.0),
            ..settings()
        };

        assert!(validate_publication(&placement, exact_settings).is_err());
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
    fn exact_grid_three_four_five_clearance_accepts_equal_and_rejects_true_sub_five() {
        let first_start = point(0.0, 0.0);
        let first_end = first_start;
        let second_start = point(3.0, 4.0);
        let second_end = second_start;

        assert!(segment_meets_clearance(
            first_start,
            first_end,
            second_start,
            second_end,
            5.0
        ));
        let sub_five = point(3.0, f64::from_bits(4.0f64.to_bits() - 1));
        assert!(!segment_meets_clearance(
            first_start,
            first_end,
            sub_five,
            sub_five,
            5.0
        ));
    }

    #[test]
    fn non_grid_squared_distance_rounding_cannot_accept_a_true_under_clearance() {
        let first = point(0.0, 0.0);
        let second = point(4.940746071465481, 0.7674817634956626);

        assert!(!segment_meets_clearance(first, first, second, second, 5.0));
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
