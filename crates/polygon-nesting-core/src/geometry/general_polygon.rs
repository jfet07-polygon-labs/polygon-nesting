//! Topology-preserving polygon geometry for the opt-in general engine path.
//!
//! The legacy engine intentionally keeps its convex, byte-stable geometry
//! pipeline. This module is a separate integer-grid authority for simple
//! concave rings and multi-region offset results. Public source holes are not
//! accepted yet because protocol v1 has no contour grouping; the internal
//! representation includes holes so Clipper output does not lose topology.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};

use crate::canonical_grid::{from_grid, to_grid_mm};
use crate::clipper::core::{
    area, point_in_polygon, ClipType, FillRule, Path64, PathType, Paths64, Point64,
    PointInPolygonResult,
};
use crate::clipper::engine::{Clipper64, PolyTree64};
use crate::clipper::offset::{ClipperOffset, EndType, JoinType};
use crate::domain::{IrregularBounds, IrregularPoint};
use crate::geometry::predicates::orientation;

pub const GENERAL_MAX_RING_VERTICES: usize = 2_048;
pub const GENERAL_MAX_POLYGON_SET_VERTICES: usize = 8_192;
pub const GENERAL_MAX_PAIR_QUERY_VERTICES: usize = 4_096;
pub const GENERAL_MAX_JOB_VERTICES: usize = 131_072;

const CLIPPER_MITER_LIMIT: f64 = 2.0;
const CLIPPER_ARC_TOLERANCE: f64 = 0.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneralPolygonError {
    message: String,
}

impl GeneralPolygonError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn from_message(message: impl Into<String>) -> Self {
        Self::new(message)
    }
}

impl Display for GeneralPolygonError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GeneralPolygonError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RingRole {
    Outer,
    Hole,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PolygonRing {
    source_points: Vec<IrregularPoint>,
    points: Vec<IrregularPoint>,
    path: Path64,
}

impl PolygonRing {
    pub fn new(points: Vec<IrregularPoint>, role: RingRole) -> Result<Self, GeneralPolygonError> {
        if points.len() < 3 {
            return Err(GeneralPolygonError::new(
                "a polygon ring must contain at least three vertices",
            ));
        }
        if points.len() > GENERAL_MAX_RING_VERTICES {
            return Err(GeneralPolygonError::new(format!(
                "a polygon ring may contain at most {GENERAL_MAX_RING_VERTICES} vertices"
            )));
        }

        validate_simple_source_ring(&points)?;
        let mut source_points = points;
        let mut path = Vec::with_capacity(source_points.len());
        let mut unique = HashSet::with_capacity(source_points.len());
        for point in &source_points {
            if !point.x.is_finite() || !point.y.is_finite() {
                return Err(GeneralPolygonError::new(
                    "polygon ring coordinates must be finite",
                ));
            }
            let Some(x) = to_grid_mm(point.x) else {
                return Err(GeneralPolygonError::new(
                    "polygon ring x coordinate is outside the contractual grid",
                ));
            };
            let Some(y) = to_grid_mm(point.y) else {
                return Err(GeneralPolygonError::new(
                    "polygon ring y coordinate is outside the contractual grid",
                ));
            };
            let key = (x as i64, y as i64);
            if !unique.insert(key) {
                return Err(GeneralPolygonError::new(
                    "polygon ring vertices must remain unique after grid snapping",
                ));
            }
            path.push(Point64::new(x, y, 0.0));
        }

        validate_simple_path(&path)?;
        let canonicalization = canonicalize_path(&mut path, role);
        if canonicalization.reversed {
            source_points.reverse();
        }
        source_points.rotate_left(canonicalization.start);
        let points = path
            .iter()
            .map(|point| IrregularPoint::new(from_grid(point.x), from_grid(point.y)))
            .collect();
        Ok(Self {
            source_points,
            points,
            path,
        })
    }

    fn from_path(path: &Path64, role: RingRole) -> Result<Self, GeneralPolygonError> {
        Self::new(
            path.iter()
                .map(|point| IrregularPoint::new(from_grid(point.x), from_grid(point.y)))
                .collect(),
            role,
        )
    }

    pub fn points(&self) -> &[IrregularPoint] {
        &self.points
    }

    pub(crate) fn source_points(&self) -> &[IrregularPoint] {
        &self.source_points
    }

    pub fn signed_area_mm2(&self) -> f64 {
        area(&self.path) / 1_000_000.0
    }

    pub fn bounds(&self) -> IrregularBounds {
        bounds_for_path(&self.path)
    }

    pub fn is_convex(&self) -> bool {
        let mut sign = 0;
        for index in 0..self.path.len() {
            let first = self.path[index];
            let second = self.path[(index + 1) % self.path.len()];
            let third = self.path[(index + 2) % self.path.len()];
            let turn = orientation(first.x, first.y, second.x, second.y, third.x, third.y);
            if turn == 0 {
                continue;
            }
            if sign == 0 {
                sign = turn;
            } else if sign != turn {
                return false;
            }
        }
        true
    }

    fn path(&self) -> &Path64 {
        &self.path
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PolygonRegion {
    pub outer: PolygonRing,
    pub holes: Vec<PolygonRing>,
}

impl PolygonRegion {
    pub fn new(
        outer_points: Vec<IrregularPoint>,
        hole_points: Vec<Vec<IrregularPoint>>,
    ) -> Result<Self, GeneralPolygonError> {
        let outer = PolygonRing::new(outer_points, RingRole::Outer)?;
        let mut holes = hole_points
            .into_iter()
            .map(|points| PolygonRing::new(points, RingRole::Hole))
            .collect::<Result<Vec<_>, _>>()?;
        validate_holes(&outer, &holes)?;
        holes.sort_by(compare_rings);
        Ok(Self { outer, holes })
    }

    pub fn area_mm2(&self) -> f64 {
        self.outer.signed_area_mm2()
            + self
                .holes
                .iter()
                .map(PolygonRing::signed_area_mm2)
                .sum::<f64>()
    }

    pub fn vertex_count(&self) -> usize {
        self.outer.points.len()
            + self
                .holes
                .iter()
                .map(|hole| hole.points.len())
                .sum::<usize>()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PolygonSet {
    pub(crate) regions: Vec<PolygonRegion>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct IntersectionAreaComplexity {
    pub area_mm2: f64,
    pub input_vertices: usize,
    pub output_vertices: usize,
}

impl PolygonSet {
    pub fn from_outer(points: Vec<IrregularPoint>) -> Result<Self, GeneralPolygonError> {
        Self::new(vec![PolygonRegion::new(points, Vec::new())?])
    }

    pub fn new(mut regions: Vec<PolygonRegion>) -> Result<Self, GeneralPolygonError> {
        if regions.is_empty() {
            return Err(GeneralPolygonError::new(
                "a polygon set must contain at least one material region",
            ));
        }
        let vertex_count = regions
            .iter()
            .map(PolygonRegion::vertex_count)
            .sum::<usize>();
        if vertex_count > GENERAL_MAX_POLYGON_SET_VERTICES {
            return Err(GeneralPolygonError::new(format!(
                "a polygon set may contain at most {GENERAL_MAX_POLYGON_SET_VERTICES} vertices"
            )));
        }
        validate_regions(&regions)?;
        regions.sort_by(|first, second| compare_rings(&first.outer, &second.outer));
        Ok(Self { regions })
    }

    pub(crate) fn empty() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    pub fn regions(&self) -> &[PolygonRegion] {
        &self.regions
    }

    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    pub fn vertex_count(&self) -> usize {
        self.regions.iter().map(PolygonRegion::vertex_count).sum()
    }

    pub fn area_mm2(&self) -> f64 {
        self.regions.iter().map(PolygonRegion::area_mm2).sum()
    }

    pub fn bounds(&self) -> Option<IrregularBounds> {
        let mut rings = self.regions.iter().map(|region| region.outer.bounds());
        let first = rings.next()?;
        Some(rings.fold(first, |bounds, current| {
            IrregularBounds::new(
                bounds.min_x.min(current.min_x),
                bounds.min_y.min(current.min_y),
                bounds.max_x.max(current.max_x),
                bounds.max_y.max(current.max_y),
            )
        }))
    }

    pub fn translated(&self, x_mm: f64, y_mm: f64) -> Result<Self, GeneralPolygonError> {
        if !x_mm.is_finite() || !y_mm.is_finite() {
            return Err(GeneralPolygonError::new(
                "polygon translation must be finite",
            ));
        }
        self.transformed(0.0, false, x_mm, y_mm)
    }

    pub fn transformed(
        &self,
        rotation_deg: f64,
        mirrored: bool,
        x_mm: f64,
        y_mm: f64,
    ) -> Result<Self, GeneralPolygonError> {
        if !rotation_deg.is_finite() || !x_mm.is_finite() || !y_mm.is_finite() {
            return Err(GeneralPolygonError::new(
                "polygon transform values must be finite",
            ));
        }
        let radians = rotation_deg.to_radians();
        let (sin, cos) = radians.sin_cos();
        let transform_ring =
            |ring: &PolygonRing| -> Result<Vec<IrregularPoint>, GeneralPolygonError> {
                ring.source_points
                    .iter()
                    .map(|point| {
                        let mirror_x = if mirrored { -point.x } else { point.x };
                        let x = mirror_x * cos - point.y * sin + x_mm;
                        let y = mirror_x * sin + point.y * cos + y_mm;
                        if !x.is_finite() || !y.is_finite() {
                            return Err(GeneralPolygonError::new(
                                "polygon transform produced a non-finite coordinate",
                            ));
                        }
                        Ok(IrregularPoint::new(x, y))
                    })
                    .collect()
            };

        if self.regions.is_empty() {
            return Ok(Self::empty());
        }
        Self::new(
            self.regions
                .iter()
                .map(|region| {
                    PolygonRegion::new(
                        transform_ring(&region.outer)?,
                        region
                            .holes
                            .iter()
                            .map(transform_ring)
                            .collect::<Result<Vec<_>, _>>()?,
                    )
                })
                .collect::<Result<Vec<_>, GeneralPolygonError>>()?,
        )
    }

    pub fn offset(&self, distance_mm: f64) -> Result<Self, GeneralPolygonError> {
        if !distance_mm.is_finite() {
            return Err(GeneralPolygonError::new("offset distance must be finite"));
        }
        let Some(distance_grid) = to_grid_mm(distance_mm) else {
            return Err(GeneralPolygonError::new(
                "offset distance is outside the contractual grid",
            ));
        };
        if distance_grid == 0.0 {
            return Ok(self.clone());
        }

        let paths = self.paths();
        let mut offset =
            ClipperOffset::new(CLIPPER_MITER_LIMIT, CLIPPER_ARC_TOLERANCE, false, false);
        offset.add_paths(&paths, JoinType::Miter, EndType::Polygon);
        let tree = offset.execute_poly_tree(distance_grid).map_err(|error| {
            GeneralPolygonError::new(format!("Clipper offset failed: {error:?}"))
        })?;
        let result = polygon_set_from_tree(&tree)?;
        if distance_grid > 0.0 && !self.regions.is_empty() && result.regions.is_empty() {
            return Err(GeneralPolygonError::new(
                "an outward offset of non-empty material returned no geometry",
            ));
        }
        Ok(result)
    }

    pub fn intersection_area_mm2(&self, other: &Self) -> Result<f64, GeneralPolygonError> {
        Ok(self.intersection_area_with_complexity(other)?.area_mm2)
    }

    pub(crate) fn intersection_area_with_complexity(
        &self,
        other: &Self,
    ) -> Result<IntersectionAreaComplexity, GeneralPolygonError> {
        let input_vertices = self.vertex_count().saturating_add(other.vertex_count());
        if input_vertices > GENERAL_MAX_PAIR_QUERY_VERTICES {
            return Err(GeneralPolygonError::new(format!(
                "an exact pair query may contain at most {GENERAL_MAX_PAIR_QUERY_VERTICES} combined vertices"
            )));
        }
        if self.regions.is_empty() || other.regions.is_empty() {
            return Ok(IntersectionAreaComplexity {
                area_mm2: 0.0,
                input_vertices,
                output_vertices: 0,
            });
        }
        let subject = self.paths();
        let clip = other.paths();
        let mut engine = Clipper64::new();
        engine.add_paths(&subject, PathType::Subject);
        engine.add_paths(&clip, PathType::Clip);
        let mut intersection = Paths64::new();
        if !engine.execute_paths(
            ClipType::Intersection,
            FillRule::NonZero,
            &mut intersection,
            None,
        ) {
            return Err(GeneralPolygonError::new("Clipper intersection failed"));
        }
        let output_vertices = intersection.iter().map(|path| path.len()).sum();
        Ok(IntersectionAreaComplexity {
            area_mm2: net_paths_area(&intersection).abs() / 1_000_000.0,
            input_vertices,
            output_vertices,
        })
    }

    pub fn fits_sheet(&self, width_mm: f64, height_mm: f64) -> bool {
        if !width_mm.is_finite() || !height_mm.is_finite() || width_mm < 0.0 || height_mm < 0.0 {
            return false;
        }
        self.fits_rect(0.0, 0.0, width_mm, height_mm)
    }

    pub fn fits_rect(&self, min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> bool {
        let (Some(min_x), Some(min_y), Some(max_x), Some(max_y)) = (
            to_grid_mm(min_x),
            to_grid_mm(min_y),
            to_grid_mm(max_x),
            to_grid_mm(max_y),
        ) else {
            return false;
        };
        if min_x > max_x || min_y > max_y {
            return false;
        }
        !self.regions.is_empty()
            && self.regions.iter().all(|region| {
                region.outer.path.iter().all(|point| {
                    point.x >= min_x && point.y >= min_y && point.x <= max_x && point.y <= max_y
                })
            })
    }

    pub fn contains_point(&self, point: IrregularPoint) -> PointInPolygonResult {
        let Some(x) = to_grid_mm(point.x) else {
            return PointInPolygonResult::IsOutside;
        };
        let Some(y) = to_grid_mm(point.y) else {
            return PointInPolygonResult::IsOutside;
        };
        let point = Point64::new(x, y, 0.0);
        for region in &self.regions {
            match point_in_polygon(point, region.outer.path()) {
                PointInPolygonResult::IsOutside => continue,
                PointInPolygonResult::IsOn => return PointInPolygonResult::IsOn,
                PointInPolygonResult::IsInside => {
                    let mut in_material = true;
                    for hole in &region.holes {
                        match point_in_polygon(point, hole.path()) {
                            PointInPolygonResult::IsOn => return PointInPolygonResult::IsOn,
                            PointInPolygonResult::IsInside => in_material = false,
                            PointInPolygonResult::IsOutside => {}
                        }
                    }
                    if in_material {
                        return PointInPolygonResult::IsInside;
                    }
                }
            }
        }
        PointInPolygonResult::IsOutside
    }

    fn paths(&self) -> Paths64 {
        let mut paths = Vec::new();
        for region in &self.regions {
            paths.push(region.outer.path.clone());
            paths.extend(region.holes.iter().map(|hole| hole.path.clone()));
        }
        paths
    }
}

fn polygon_set_from_tree(tree: &PolyTree64) -> Result<PolygonSet, GeneralPolygonError> {
    let mut regions = Vec::new();
    collect_regions(tree, PolyTree64::ROOT, &mut regions)?;
    if regions.is_empty() {
        Ok(PolygonSet::empty())
    } else {
        PolygonSet::new(regions)
    }
}

fn collect_regions(
    tree: &PolyTree64,
    parent: usize,
    regions: &mut Vec<PolygonRegion>,
) -> Result<(), GeneralPolygonError> {
    for index in 0..tree.count(parent) {
        let child = tree.child(parent, index);
        if !tree.is_hole(child) {
            let outer_path = tree
                .poly(child)
                .ok_or_else(|| GeneralPolygonError::new("Clipper returned an empty outer node"))?;
            let outer = PolygonRing::from_path(outer_path, RingRole::Outer)?;
            let mut holes = Vec::new();
            for hole_index in 0..tree.count(child) {
                let hole = tree.child(child, hole_index);
                if tree.is_hole(hole) {
                    let path = tree.poly(hole).ok_or_else(|| {
                        GeneralPolygonError::new("Clipper returned an empty hole node")
                    })?;
                    holes.push(PolygonRing::from_path(path, RingRole::Hole)?);
                }
            }
            validate_holes(&outer, &holes)?;
            holes.sort_by(compare_rings);
            regions.push(PolygonRegion { outer, holes });
        }
        collect_regions(tree, child, regions)?;
    }
    Ok(())
}

fn validate_simple_source_ring(points: &[IrregularPoint]) -> Result<(), GeneralPolygonError> {
    let signed_double_area = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(first, second)| first.x * second.y - second.x * first.y)
        .sum::<f64>();
    if !signed_double_area.is_finite() || signed_double_area == 0.0 {
        return Err(GeneralPolygonError::new(
            "polygon ring must have finite non-zero source area",
        ));
    }
    for first_index in 0..points.len() {
        let first_next = (first_index + 1) % points.len();
        for second_index in (first_index + 1)..points.len() {
            let second_next = (second_index + 1) % points.len();
            if first_index == second_index
                || first_next == second_index
                || second_next == first_index
            {
                continue;
            }
            if source_segments_intersect_or_overlap(
                points[first_index],
                points[first_next],
                points[second_index],
                points[second_next],
            ) {
                return Err(GeneralPolygonError::new(
                    "polygon ring source must not self-intersect",
                ));
            }
        }
    }
    Ok(())
}

fn validate_simple_path(path: &Path64) -> Result<(), GeneralPolygonError> {
    if area(path) == 0.0 {
        return Err(GeneralPolygonError::new(
            "polygon ring must have non-zero area after grid snapping",
        ));
    }
    for first_index in 0..path.len() {
        let first_next = (first_index + 1) % path.len();
        for second_index in (first_index + 1)..path.len() {
            let second_next = (second_index + 1) % path.len();
            if first_index == second_index
                || first_next == second_index
                || second_next == first_index
            {
                continue;
            }
            if segments_intersect_or_overlap(
                path[first_index],
                path[first_next],
                path[second_index],
                path[second_next],
            ) {
                return Err(GeneralPolygonError::new(
                    "polygon ring must not self-intersect after grid snapping",
                ));
            }
        }
    }
    Ok(())
}

fn validate_holes(outer: &PolygonRing, holes: &[PolygonRing]) -> Result<(), GeneralPolygonError> {
    for (index, hole) in holes.iter().enumerate() {
        if point_in_polygon(hole.path[0], outer.path()) != PointInPolygonResult::IsInside {
            return Err(GeneralPolygonError::new(
                "every hole must lie strictly inside its outer ring",
            ));
        }
        if rings_intersect(outer, hole) {
            return Err(GeneralPolygonError::new(
                "a hole boundary must not touch or cross its outer ring",
            ));
        }
        for previous in &holes[..index] {
            if rings_intersect(previous, hole)
                || point_in_polygon(hole.path[0], previous.path())
                    != PointInPolygonResult::IsOutside
                || point_in_polygon(previous.path[0], hole.path())
                    != PointInPolygonResult::IsOutside
            {
                return Err(GeneralPolygonError::new(
                    "hole boundaries must not overlap or contain each other",
                ));
            }
        }
    }
    Ok(())
}

fn validate_regions(regions: &[PolygonRegion]) -> Result<(), GeneralPolygonError> {
    for region in regions {
        if region.outer.signed_area_mm2() <= 0.0
            || region
                .holes
                .iter()
                .any(|hole| hole.signed_area_mm2() >= 0.0)
        {
            return Err(GeneralPolygonError::new(
                "polygon region rings must use canonical outer and hole winding",
            ));
        }
        validate_holes(&region.outer, &region.holes)?;
    }
    for first_index in 0..regions.len() {
        for second in &regions[(first_index + 1)..] {
            let first = &regions[first_index];
            if region_boundaries_intersect(first, second)
                || point_in_region(first.outer.path[0], second) != PointInPolygonResult::IsOutside
                || point_in_region(second.outer.path[0], first) != PointInPolygonResult::IsOutside
            {
                return Err(GeneralPolygonError::new(
                    "polygon material regions must not overlap or touch",
                ));
            }
        }
    }
    Ok(())
}

fn region_boundaries_intersect(first: &PolygonRegion, second: &PolygonRegion) -> bool {
    std::iter::once(&first.outer)
        .chain(first.holes.iter())
        .any(|first_ring| {
            std::iter::once(&second.outer)
                .chain(second.holes.iter())
                .any(|second_ring| rings_intersect(first_ring, second_ring))
        })
}

fn point_in_region(point: Point64, region: &PolygonRegion) -> PointInPolygonResult {
    match point_in_polygon(point, region.outer.path()) {
        PointInPolygonResult::IsOutside => PointInPolygonResult::IsOutside,
        PointInPolygonResult::IsOn => PointInPolygonResult::IsOn,
        PointInPolygonResult::IsInside => {
            for hole in &region.holes {
                match point_in_polygon(point, hole.path()) {
                    PointInPolygonResult::IsOn => return PointInPolygonResult::IsOn,
                    PointInPolygonResult::IsInside => return PointInPolygonResult::IsOutside,
                    PointInPolygonResult::IsOutside => {}
                }
            }
            PointInPolygonResult::IsInside
        }
    }
}

fn rings_intersect(first: &PolygonRing, second: &PolygonRing) -> bool {
    for first_index in 0..first.path.len() {
        let first_next = (first_index + 1) % first.path.len();
        for second_index in 0..second.path.len() {
            let second_next = (second_index + 1) % second.path.len();
            if segments_intersect_or_overlap(
                first.path[first_index],
                first.path[first_next],
                second.path[second_index],
                second.path[second_next],
            ) {
                return true;
            }
        }
    }
    false
}

fn segments_intersect_or_overlap(a: Point64, b: Point64, c: Point64, d: Point64) -> bool {
    let abc = orientation(a.x, a.y, b.x, b.y, c.x, c.y);
    let abd = orientation(a.x, a.y, b.x, b.y, d.x, d.y);
    let cda = orientation(c.x, c.y, d.x, d.y, a.x, a.y);
    let cdb = orientation(c.x, c.y, d.x, d.y, b.x, b.y);

    if abc != abd && cda != cdb {
        return true;
    }
    (abc == 0 && point_on_segment(c, a, b))
        || (abd == 0 && point_on_segment(d, a, b))
        || (cda == 0 && point_on_segment(a, c, d))
        || (cdb == 0 && point_on_segment(b, c, d))
}

fn point_on_segment(point: Point64, start: Point64, end: Point64) -> bool {
    point.x >= start.x.min(end.x)
        && point.x <= start.x.max(end.x)
        && point.y >= start.y.min(end.y)
        && point.y <= start.y.max(end.y)
}

fn source_segments_intersect_or_overlap(
    a: IrregularPoint,
    b: IrregularPoint,
    c: IrregularPoint,
    d: IrregularPoint,
) -> bool {
    let abc = orientation(a.x, a.y, b.x, b.y, c.x, c.y);
    let abd = orientation(a.x, a.y, b.x, b.y, d.x, d.y);
    let cda = orientation(c.x, c.y, d.x, d.y, a.x, a.y);
    let cdb = orientation(c.x, c.y, d.x, d.y, b.x, b.y);

    if abc != abd && cda != cdb {
        return true;
    }
    (abc == 0 && source_point_on_segment(c, a, b))
        || (abd == 0 && source_point_on_segment(d, a, b))
        || (cda == 0 && source_point_on_segment(a, c, d))
        || (cdb == 0 && source_point_on_segment(b, c, d))
}

fn source_point_on_segment(
    point: IrregularPoint,
    start: IrregularPoint,
    end: IrregularPoint,
) -> bool {
    point.x >= start.x.min(end.x)
        && point.x <= start.x.max(end.x)
        && point.y >= start.y.min(end.y)
        && point.y <= start.y.max(end.y)
}

#[derive(Clone, Copy)]
struct Canonicalization {
    reversed: bool,
    start: usize,
}

fn canonicalize_path(path: &mut Path64, role: RingRole) -> Canonicalization {
    let should_be_positive = role == RingRole::Outer;
    let reversed = (area(path) > 0.0) != should_be_positive;
    if reversed {
        path.reverse();
    }
    let start = path
        .iter()
        .enumerate()
        .min_by(|(_, first), (_, second)| compare_points(first, second))
        .map(|(index, _)| index)
        .unwrap_or(0);
    path.rotate_left(start);
    Canonicalization { reversed, start }
}

fn compare_points(first: &Point64, second: &Point64) -> Ordering {
    first
        .y
        .total_cmp(&second.y)
        .then_with(|| first.x.total_cmp(&second.x))
}

fn compare_rings(first: &PolygonRing, second: &PolygonRing) -> Ordering {
    compare_points(&first.path[0], &second.path[0]).then_with(|| {
        first
            .path
            .iter()
            .zip(&second.path)
            .find_map(|(first, second)| {
                let ordering = compare_points(first, second);
                (ordering != Ordering::Equal).then_some(ordering)
            })
            .unwrap_or_else(|| first.path.len().cmp(&second.path.len()))
    })
}

fn bounds_for_path(path: &Path64) -> IrregularBounds {
    let mut min_x = path[0].x;
    let mut min_y = path[0].y;
    let mut max_x = path[0].x;
    let mut max_y = path[0].y;
    for point in &path[1..] {
        min_x = min_x.min(point.x);
        min_y = min_y.min(point.y);
        max_x = max_x.max(point.x);
        max_y = max_y.max(point.y);
    }
    IrregularBounds::new(
        from_grid(min_x),
        from_grid(min_y),
        from_grid(max_x),
        from_grid(max_y),
    )
}

fn net_paths_area(paths: &Paths64) -> f64 {
    paths.iter().map(|path| area(path)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64) -> IrregularPoint {
        IrregularPoint::new(x, y)
    }

    fn l_shape() -> PolygonSet {
        PolygonSet::from_outer(vec![
            point(0.0, 0.0),
            point(4.0, 0.0),
            point(4.0, 1.0),
            point(1.0, 1.0),
            point(1.0, 4.0),
            point(0.0, 4.0),
        ])
        .unwrap()
    }

    #[test]
    fn preserves_concavity_and_contractual_grid_area() {
        let shape = l_shape();
        assert_eq!(shape.area_mm2(), 7.0);
        assert!(!shape.regions[0].outer.is_convex());
        assert_eq!(shape.regions[0].outer.points().len(), 6);
    }

    #[test]
    fn retains_unsnapped_source_coordinates_for_independent_validation() {
        let shape = PolygonSet::from_outer(vec![
            point(0.0, 0.0),
            point(1.0004, 0.0),
            point(1.0004, 1.0),
            point(0.0, 1.0),
        ])
        .unwrap();
        let ring = &shape.regions[0].outer;
        assert_eq!(ring.points()[1].x, 1.0);
        assert_eq!(ring.source_points()[1].x, 1.0004);
    }

    #[test]
    fn transforms_preserve_unsnapped_source_coordinates() {
        let shape = PolygonSet::from_outer(vec![
            point(0.0004, 0.0004),
            point(1.0004, 0.0004),
            point(1.0004, 2.0004),
            point(0.0004, 2.0004),
        ])
        .unwrap();
        let transformed = shape.transformed(270.0, false, 0.0, 0.0).unwrap();
        let source_points = transformed.regions[0].outer.source_points();

        assert!(source_points
            .iter()
            .any(|point| (point.x - 0.0004).abs() < f64::EPSILON));
        assert!(source_points
            .iter()
            .any(|point| (point.y + 1.0004).abs() < f64::EPSILON));
    }

    #[test]
    fn fits_an_explicit_sheet_rectangle() {
        let shape = l_shape().translated(2.0, 3.0).unwrap();

        assert!(shape.fits_rect(2.0, 3.0, 6.0, 7.0));
        assert!(!shape.fits_rect(2.001, 3.0, 6.0, 7.0));
        assert!(!shape.fits_rect(2.0, 3.0, 5.999, 7.0));
    }

    #[test]
    fn rejects_self_intersections_and_snap_collapses() {
        let bow_tie = PolygonSet::from_outer(vec![
            point(0.0, 0.0),
            point(2.0, 2.0),
            point(0.0, 2.0),
            point(2.0, 0.0),
        ]);
        assert!(bow_tie.is_err());

        let collapsed = PolygonSet::from_outer(vec![
            point(0.0, 0.0),
            point(0.0004, 0.0004),
            point(1.0, 0.0),
            point(0.0, 1.0),
        ]);
        assert_eq!(
            collapsed.unwrap_err().message(),
            "polygon ring vertices must remain unique after grid snapping"
        );
    }

    #[test]
    fn classifies_internal_hole_topology() {
        let donut = PolygonSet::new(vec![PolygonRegion::new(
            vec![
                point(0.0, 0.0),
                point(10.0, 0.0),
                point(10.0, 10.0),
                point(0.0, 10.0),
            ],
            vec![vec![
                point(3.0, 3.0),
                point(3.0, 7.0),
                point(7.0, 7.0),
                point(7.0, 3.0),
            ]],
        )
        .unwrap()])
        .unwrap();
        assert_eq!(donut.area_mm2(), 84.0);
        assert_eq!(
            donut.contains_point(point(1.0, 1.0)),
            PointInPolygonResult::IsInside
        );
        assert_eq!(
            donut.contains_point(point(5.0, 5.0)),
            PointInPolygonResult::IsOutside
        );
        assert_eq!(
            donut.contains_point(point(3.0, 4.0)),
            PointInPolygonResult::IsOn
        );
    }

    #[test]
    fn intersection_uses_real_concavity_instead_of_the_hull() {
        let pocket_piece = PolygonSet::from_outer(vec![
            point(1.0, 1.0),
            point(3.5, 1.0),
            point(3.5, 3.5),
            point(1.0, 3.5),
        ])
        .unwrap();
        assert_eq!(l_shape().intersection_area_mm2(&pocket_piece).unwrap(), 0.0);
    }

    #[test]
    fn intersection_complexity_reports_the_same_area_and_exact_vertex_work() {
        let first = PolygonSet::from_outer(vec![
            point(0.0, 0.0),
            point(4.0, 0.0),
            point(4.0, 4.0),
            point(0.0, 4.0),
        ])
        .unwrap();
        let second = PolygonSet::from_outer(vec![
            point(2.0, 1.0),
            point(6.0, 1.0),
            point(6.0, 3.0),
            point(2.0, 3.0),
        ])
        .unwrap();
        let result = first.intersection_area_with_complexity(&second).unwrap();

        assert_eq!(
            result.area_mm2,
            first.intersection_area_mm2(&second).unwrap()
        );
        assert_eq!(result.area_mm2, 4.0);
        assert_eq!(result.input_vertices, 8);
        assert_eq!(result.output_vertices, 4);
    }

    #[test]
    fn offset_preserves_holes_instead_of_flattening_paths() {
        let donut = PolygonSet::new(vec![PolygonRegion::new(
            vec![
                point(0.0, 0.0),
                point(10.0, 0.0),
                point(10.0, 10.0),
                point(0.0, 10.0),
            ],
            vec![vec![
                point(3.0, 3.0),
                point(3.0, 7.0),
                point(7.0, 7.0),
                point(7.0, 3.0),
            ]],
        )
        .unwrap()])
        .unwrap();
        let expanded = donut.offset(0.5).unwrap();
        assert_eq!(expanded.regions.len(), 1);
        assert_eq!(expanded.regions[0].holes.len(), 1);
        assert!(expanded.area_mm2() > donut.area_mm2());
    }

    #[test]
    fn inward_offset_preserves_split_components() {
        let dumbbell = PolygonSet::from_outer(vec![
            point(0.0, 0.0),
            point(3.0, 0.0),
            point(3.0, 0.9),
            point(5.0, 0.9),
            point(5.0, 0.0),
            point(8.0, 0.0),
            point(8.0, 3.0),
            point(5.0, 3.0),
            point(5.0, 2.1),
            point(3.0, 2.1),
            point(3.0, 3.0),
            point(0.0, 3.0),
        ])
        .unwrap();

        let eroded = dumbbell.offset(-0.7).unwrap();
        assert_eq!(eroded.regions.len(), 2);
    }

    #[test]
    fn outward_offset_can_merge_regions_and_collapse_holes() {
        let region = |origin_x: f64| {
            PolygonRegion::new(
                vec![
                    point(origin_x, 0.0),
                    point(origin_x + 2.0, 0.0),
                    point(origin_x + 2.0, 2.0),
                    point(origin_x, 2.0),
                ],
                Vec::new(),
            )
            .unwrap()
        };
        let separated = PolygonSet::new(vec![region(0.0), region(2.5)]).unwrap();
        assert_eq!(separated.offset(0.3).unwrap().regions.len(), 1);

        let donut = PolygonSet::new(vec![PolygonRegion::new(
            vec![
                point(0.0, 0.0),
                point(10.0, 0.0),
                point(10.0, 10.0),
                point(0.0, 10.0),
            ],
            vec![vec![
                point(3.0, 3.0),
                point(3.0, 7.0),
                point(7.0, 7.0),
                point(7.0, 3.0),
            ]],
        )
        .unwrap()])
        .unwrap();
        assert!(donut.offset(2.1).unwrap().regions[0].holes.is_empty());
    }

    #[test]
    fn rejects_overlapping_and_nested_material_regions() {
        let square_region = |origin_x: f64, origin_y: f64, side: f64| {
            PolygonRegion::new(
                vec![
                    point(origin_x, origin_y),
                    point(origin_x + side, origin_y),
                    point(origin_x + side, origin_y + side),
                    point(origin_x, origin_y + side),
                ],
                Vec::new(),
            )
            .unwrap()
        };
        assert!(PolygonSet::new(vec![
            square_region(0.0, 0.0, 4.0),
            square_region(3.0, 0.0, 4.0),
        ])
        .is_err());
        assert!(PolygonSet::new(vec![
            square_region(0.0, 0.0, 10.0),
            square_region(2.0, 2.0, 1.0),
        ])
        .is_err());
    }

    #[test]
    fn permits_an_island_region_inside_a_hole() {
        let containing = PolygonRegion::new(
            vec![
                point(0.0, 0.0),
                point(10.0, 0.0),
                point(10.0, 10.0),
                point(0.0, 10.0),
            ],
            vec![vec![
                point(2.0, 2.0),
                point(2.0, 8.0),
                point(8.0, 8.0),
                point(8.0, 2.0),
            ]],
        )
        .unwrap();
        let island = PolygonRegion::new(
            vec![
                point(4.0, 4.0),
                point(6.0, 4.0),
                point(6.0, 6.0),
                point(4.0, 6.0),
            ],
            Vec::new(),
        )
        .unwrap();
        assert!(PolygonSet::new(vec![containing, island]).is_ok());
    }

    #[test]
    fn signed_zero_vertices_collapse_to_one_grid_point() {
        let result = PolygonSet::from_outer(vec![
            point(0.0, 0.0),
            point(-0.0, 0.0),
            point(1.0, 0.0),
            point(0.0, 1.0),
        ]);
        assert!(result.is_err());
    }
}
