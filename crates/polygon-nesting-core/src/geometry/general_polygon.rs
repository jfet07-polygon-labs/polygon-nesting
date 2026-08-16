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
use std::mem::size_of;

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
pub(crate) const GENERAL_EXACT_ARRANGEMENT_SCRATCH_CAP_BYTES: usize = 4 * 1024 * 1024;

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

    fn heap_bytes(&self) -> usize {
        self.source_points
            .capacity()
            .saturating_mul(size_of::<IrregularPoint>())
            .saturating_add(
                self.points
                    .capacity()
                    .saturating_mul(size_of::<IrregularPoint>()),
            )
            .saturating_add(self.path.capacity().saturating_mul(size_of::<Point64>()))
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

/// A rational point whose coordinates are exact integer-grid fractions.
///
/// The articulation probe uses these points as component witnesses. Keeping
/// the denominator explicit avoids converting a witness to `f64` and then
/// making a tolerance-based containment decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactRationalPoint {
    pub x_num: i128,
    pub y_num: i128,
    pub den: i128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExactRationalPointLocation {
    Outside,
    On,
    Inside,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactGridBounds {
    pub min_x: i64,
    pub min_y: i64,
    pub max_x: i64,
    pub max_y: i64,
}

impl ExactGridBounds {
    pub(crate) fn may_overlap_or_touch(self, other: Self) -> bool {
        self.min_x <= other.max_x
            && other.min_x <= self.max_x
            && self.min_y <= other.max_y
            && other.min_y <= self.max_y
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RectangularFreeSpaceRegion {
    pub doubled_area_grid2: i128,
    pub frontier_contact_grid: i64,
    pub frontier_point_contact_only: bool,
    pub polygon: PolygonSet,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RectangularFreeSpaceTopology {
    pub regions: Vec<RectangularFreeSpaceRegion>,
    pub input_vertices: usize,
    pub output_vertices: usize,
}

impl RectangularFreeSpaceTopology {
    pub(crate) fn heap_bytes(&self) -> usize {
        self.regions
            .capacity()
            .saturating_mul(size_of::<RectangularFreeSpaceRegion>())
            .saturating_add(
                self.regions
                    .iter()
                    .map(|region| region.polygon.heap_bytes())
                    .sum::<usize>(),
            )
    }
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

    pub(crate) fn exact_doubled_area_grid2(&self) -> Result<i128, GeneralPolygonError> {
        self.regions.iter().try_fold(0_i128, |total, region| {
            let outer = exact_path_doubled_area_grid2(region.outer.path())?.abs();
            let region_area = region.holes.iter().try_fold(outer, |area, hole| {
                area.checked_sub(exact_path_doubled_area_grid2(hole.path())?.abs())
                    .ok_or_else(|| GeneralPolygonError::new("exact grid area overflow"))
            })?;
            total
                .checked_add(region_area)
                .ok_or_else(|| GeneralPolygonError::new("exact grid area overflow"))
        })
    }

    pub(crate) fn exact_grid_bounds(&self) -> Result<ExactGridBounds, GeneralPolygonError> {
        let mut bounds = None::<ExactGridBounds>;
        for region in &self.regions {
            for point in &region.outer.path {
                let x = exact_grid_coordinate(point.x)?;
                let y = exact_grid_coordinate(point.y)?;
                bounds = Some(match bounds {
                    Some(current) => ExactGridBounds {
                        min_x: current.min_x.min(x),
                        min_y: current.min_y.min(y),
                        max_x: current.max_x.max(x),
                        max_y: current.max_y.max(y),
                    },
                    None => ExactGridBounds {
                        min_x: x,
                        min_y: y,
                        max_x: x,
                        max_y: y,
                    },
                });
            }
        }
        bounds.ok_or_else(|| GeneralPolygonError::new("exact grid bounds require material"))
    }

    /// Reports whether two material sets have a positive-area overlap or a
    /// shared positive-length boundary segment.
    ///
    /// This is a connectivity relation for the exact material-node graph. It
    /// deliberately excludes point-only contact: a zero-width corner does not
    /// provide a vacancy route. The test uses only integer-grid containment
    /// and segment predicates, so it does not reconstruct or re-round either
    /// input polygon.
    pub(crate) fn material_overlap_or_shared_segment(
        &self,
        other: &Self,
    ) -> Result<(bool, usize), GeneralPolygonError> {
        for region in &self.regions {
            for point in &region.outer.path {
                if other.exact_rational_location(ExactRationalPoint {
                    x_num: i128::from(exact_grid_coordinate(point.x)?),
                    y_num: i128::from(exact_grid_coordinate(point.y)?),
                    den: 1,
                }) == ExactRationalPointLocation::Inside
                {
                    return Ok((true, 0));
                }
            }
        }
        for region in &other.regions {
            for point in &region.outer.path {
                if self.exact_rational_location(ExactRationalPoint {
                    x_num: i128::from(exact_grid_coordinate(point.x)?),
                    y_num: i128::from(exact_grid_coordinate(point.y)?),
                    den: 1,
                }) == ExactRationalPointLocation::Inside
                {
                    return Ok((true, 0));
                }
            }
        }

        let first_edges = polygon_set_edges(self, polygon_set_edge_count(self)?);
        let second_edges = polygon_set_edges(other, polygon_set_edge_count(other)?);
        let mut edge_checks = 0_usize;
        for (first_start, first_end) in &first_edges {
            for (second_start, second_end) in &second_edges {
                edge_checks = edge_checks
                    .checked_add(1)
                    .ok_or_else(|| GeneralPolygonError::new("material graph edge overflow"))?;
                // point64 coordinates are canonical integer-grid values held
                // exactly in f64. reject disjoint segment boxes before the
                // more expensive checked i128 predicates.
                if first_start.x.max(first_end.x) < second_start.x.min(second_end.x)
                    || second_start.x.max(second_end.x) < first_start.x.min(first_end.x)
                    || first_start.y.max(first_end.y) < second_start.y.min(second_end.y)
                    || second_start.y.max(second_end.y) < first_start.y.min(first_end.y)
                {
                    continue;
                }
                if exact_segments_have_proper_crossing(
                    *first_start,
                    *first_end,
                    *second_start,
                    *second_end,
                )? || exact_segments_have_positive_collinear_overlap(
                    *first_start,
                    *first_end,
                    *second_start,
                    *second_end,
                )? {
                    return Ok((true, edge_checks));
                }
            }
        }
        Ok((false, edge_checks))
    }

    pub(crate) fn heap_bytes(&self) -> usize {
        self.regions
            .capacity()
            .saturating_mul(size_of::<PolygonRegion>())
            .saturating_add(
                self.regions
                    .iter()
                    .map(|region| {
                        region
                            .outer
                            .heap_bytes()
                            .saturating_add(
                                region
                                    .holes
                                    .capacity()
                                    .saturating_mul(size_of::<PolygonRing>()),
                            )
                            .saturating_add(
                                region
                                    .holes
                                    .iter()
                                    .map(PolygonRing::heap_bytes)
                                    .sum::<usize>(),
                            )
                    })
                    .sum::<usize>(),
            )
    }

    /// Returns a strictly interior witness for a single material region.
    ///
    /// Each candidate starts at the midpoint of an outer boundary edge and
    /// moves into the canonical positive-winding side by an exact rational
    /// amount. The offset is reduced geometrically until it is inside the
    /// region. A bounded failure is preferable to silently classifying a
    /// boundary point: callers use this only for read-only diagnostics and
    /// must fail closed when the geometry cannot provide a witness.
    pub(crate) fn strict_interior_witness(
        &self,
    ) -> Result<ExactRationalPoint, GeneralPolygonError> {
        if self.regions.len() != 1 {
            return Err(GeneralPolygonError::new(
                "an exact component witness requires exactly one region",
            ));
        }
        let outer = &self.regions[0].outer.path;
        if outer.len() < 3 {
            return Err(GeneralPolygonError::new(
                "an exact component witness requires a non-degenerate ring",
            ));
        }

        // the coordinate contract is integer grid space. with integer
        // segment endpoints, 2^62 subdivisions are sufficient for the
        // bounded witness search under the i64 coordinate contract; all
        // arithmetic remains inside i128 after checked operations.
        for edge_index in 0..outer.len() {
            let first = outer[edge_index];
            let second = outer[(edge_index + 1) % outer.len()];
            let first_x = exact_grid_coordinate(first.x)? as i128;
            let first_y = exact_grid_coordinate(first.y)? as i128;
            let second_x = exact_grid_coordinate(second.x)? as i128;
            let second_y = exact_grid_coordinate(second.y)? as i128;
            let dx = second_x
                .checked_sub(first_x)
                .ok_or_else(|| GeneralPolygonError::new("exact witness edge overflow"))?;
            let dy = second_y
                .checked_sub(first_y)
                .ok_or_else(|| GeneralPolygonError::new("exact witness edge overflow"))?;
            if dx == 0 && dy == 0 {
                continue;
            }

            let mut subdivisions = 1_i128;
            for _ in 0..62 {
                let den = subdivisions.checked_mul(2).ok_or_else(|| {
                    GeneralPolygonError::new("exact witness denominator overflow")
                })?;
                let midpoint_x = first_x
                    .checked_add(second_x)
                    .and_then(|sum| sum.checked_mul(subdivisions))
                    .ok_or_else(|| GeneralPolygonError::new("exact witness x overflow"))?;
                let midpoint_y = first_y
                    .checked_add(second_y)
                    .and_then(|sum| sum.checked_mul(subdivisions))
                    .ok_or_else(|| GeneralPolygonError::new("exact witness y overflow"))?;
                // outer rings are canonicalized to positive winding, so the
                // left normal points into the material region.
                let x_num = midpoint_x
                    .checked_sub(dy)
                    .ok_or_else(|| GeneralPolygonError::new("exact witness x overflow"))?;
                let y_num = midpoint_y
                    .checked_add(dx)
                    .ok_or_else(|| GeneralPolygonError::new("exact witness y overflow"))?;
                let candidate = ExactRationalPoint { x_num, y_num, den };
                if self.exact_rational_location(candidate) == ExactRationalPointLocation::Inside {
                    return Ok(candidate);
                }
                subdivisions = subdivisions.checked_mul(2).ok_or_else(|| {
                    GeneralPolygonError::new("exact witness subdivision overflow")
                })?;
            }
        }

        Err(GeneralPolygonError::new(
            "bounded exact component witness search found no strict interior point",
        ))
    }

    pub(crate) fn exact_rational_location(
        &self,
        point: ExactRationalPoint,
    ) -> ExactRationalPointLocation {
        if point.den <= 0 {
            return ExactRationalPointLocation::Outside;
        }
        let mut on_boundary = false;
        for region in &self.regions {
            match exact_rational_ring_location(point, region.outer.path()) {
                ExactRationalPointLocation::Outside => continue,
                ExactRationalPointLocation::On => on_boundary = true,
                ExactRationalPointLocation::Inside => {
                    let mut in_hole = false;
                    for hole in &region.holes {
                        match exact_rational_ring_location(point, hole.path()) {
                            ExactRationalPointLocation::On => on_boundary = true,
                            ExactRationalPointLocation::Inside => in_hole = true,
                            ExactRationalPointLocation::Outside => {}
                        }
                    }
                    if !in_hole && !on_boundary {
                        return ExactRationalPointLocation::Inside;
                    }
                }
            }
        }
        if on_boundary {
            ExactRationalPointLocation::On
        } else {
            ExactRationalPointLocation::Outside
        }
    }

    /// Proves material inclusion using the exact integer-grid arrangement.
    ///
    /// A witness only identifies one candidate region; it does not prove that
    /// the candidate region is wholly contained. This predicate therefore
    /// checks every boundary interval of both polygon sets. Every baseline
    /// boundary interval must be in the counterfactual material closure, and
    /// no counterfactual boundary interval may run through baseline material.
    /// The latter rejects an enclosed counterfactual hole or a disconnected
    /// counterfactual region hidden inside the baseline material. All interval
    /// endpoints and midpoints are exact rationals; no epsilon or clipped
    /// floating-point area is involved.
    #[allow(dead_code)]
    pub(crate) fn exact_material_subset_of(
        &self,
        container: &Self,
    ) -> Result<bool, GeneralPolygonError> {
        self.exact_material_subset_of_with_complexity(container)
            .map(|(subset, _)| subset)
    }

    pub(crate) fn exact_material_subset_of_with_complexity(
        &self,
        container: &Self,
    ) -> Result<(bool, usize), GeneralPolygonError> {
        self.exact_material_subset_of_with_complexity_and_scratch(container)
            .map(|(subset, boundary_edge_checks, _)| (subset, boundary_edge_checks))
    }

    pub(crate) fn exact_material_subset_of_with_complexity_and_scratch(
        &self,
        container: &Self,
    ) -> Result<(bool, usize, usize), GeneralPolygonError> {
        if self.regions.is_empty() {
            return Ok((true, 0, 0));
        }
        if container.regions.is_empty() {
            return Ok((false, 0, 0));
        }
        let combined_vertices = self
            .vertex_count()
            .checked_add(container.vertex_count())
            .ok_or_else(|| GeneralPolygonError::new("exact subset vertex-count overflow"))?;
        if combined_vertices > GENERAL_MAX_PAIR_QUERY_VERTICES {
            return Err(GeneralPolygonError::new(
                "exact material subset query exceeds the contractual vertex cap",
            ));
        }
        let scratch_peak_bytes = exact_material_subset_scratch_bytes(self, container)?;
        if scratch_peak_bytes > GENERAL_EXACT_ARRANGEMENT_SCRATCH_CAP_BYTES {
            return Err(GeneralPolygonError::new(format!(
                "exact material subset arrangement scratch requires {scratch_peak_bytes} bytes, exceeding the {GENERAL_EXACT_ARRANGEMENT_SCRATCH_CAP_BYTES}-byte cap"
            )));
        }

        let mut boundary_edge_checks = 0_usize;
        for region in &self.regions {
            let witness = PolygonSet {
                regions: vec![region.clone()],
            }
            .strict_interior_witness()?;
            if container.exact_rational_location(witness) != ExactRationalPointLocation::Inside {
                return Ok((false, boundary_edge_checks, scratch_peak_bytes));
            }
            for ring in std::iter::once(&region.outer).chain(region.holes.iter()) {
                let (in_closure, edge_checks) =
                    exact_boundary_is_in_material_closure(ring.path(), container)?;
                boundary_edge_checks = boundary_edge_checks
                    .checked_add(edge_checks)
                    .ok_or_else(|| GeneralPolygonError::new("exact subset work overflow"))?;
                if !in_closure {
                    return Ok((false, boundary_edge_checks, scratch_peak_bytes));
                }
            }
        }

        // a counterfactual boundary strictly inside baseline material would
        // split baseline material into material and non-material sides. it is
        // therefore evidence that the whole baseline is not contained, even
        // when the baseline's own witness happens to be inside the container.
        for region in &container.regions {
            for ring in std::iter::once(&region.outer).chain(region.holes.iter()) {
                let (runs_through, edge_checks) =
                    exact_boundary_runs_through_material(ring.path(), self)?;
                boundary_edge_checks = boundary_edge_checks
                    .checked_add(edge_checks)
                    .ok_or_else(|| GeneralPolygonError::new("exact subset work overflow"))?;
                if runs_through {
                    return Ok((false, boundary_edge_checks, scratch_peak_bytes));
                }
            }
        }
        Ok((true, boundary_edge_checks, scratch_peak_bytes))
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
        self.offset_with_vertex_counts(distance_mm)
            .map(|(polygon, _, _)| polygon)
    }

    pub(crate) fn offset_with_vertex_counts(
        &self,
        distance_mm: f64,
    ) -> Result<(Self, usize, usize), GeneralPolygonError> {
        if !distance_mm.is_finite() {
            return Err(GeneralPolygonError::new("offset distance must be finite"));
        }
        let Some(distance_grid) = to_grid_mm(distance_mm) else {
            return Err(GeneralPolygonError::new(
                "offset distance is outside the contractual grid",
            ));
        };
        if distance_grid == 0.0 {
            return Ok((self.clone(), self.vertex_count(), self.vertex_count()));
        }

        let paths = self.paths();
        let input_vertices = self.vertex_count();
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
        let output_vertices = result.vertex_count();
        Ok((result, input_vertices, output_vertices))
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

    /// derives exact connected free-space regions inside one grid-aligned rectangle.
    pub(crate) fn rectangular_free_space_topology(
        occupied: &[&Self],
        min_x_mm: f64,
        min_y_mm: f64,
        max_x_mm: f64,
        max_y_mm: f64,
    ) -> Result<RectangularFreeSpaceTopology, GeneralPolygonError> {
        let (Some(min_x), Some(min_y), Some(max_x), Some(max_y)) = (
            to_grid_mm(min_x_mm),
            to_grid_mm(min_y_mm),
            to_grid_mm(max_x_mm),
            to_grid_mm(max_y_mm),
        ) else {
            return Err(GeneralPolygonError::new(
                "free-space rectangle is outside the contractual grid",
            ));
        };
        if min_x >= max_x || min_y >= max_y {
            return Err(GeneralPolygonError::new(
                "free-space rectangle must have positive width and height",
            ));
        }
        let rectangle = vec![
            Point64::new(min_x, min_y, 0.0),
            Point64::new(max_x, min_y, 0.0),
            Point64::new(max_x, max_y, 0.0),
            Point64::new(min_x, max_y, 0.0),
        ];
        let input_vertices = occupied
            .iter()
            .try_fold(rectangle.len(), |total, polygon| {
                total
                    .checked_add(polygon.vertex_count())
                    .ok_or_else(|| GeneralPolygonError::new("free-space input-vertex overflow"))
            })?;
        let mut engine = Clipper64::new();
        let subject = vec![rectangle];
        engine.add_paths(&subject, PathType::Subject);
        for polygon in occupied {
            engine.add_paths(&polygon.paths(), PathType::Clip);
        }
        let mut free_tree = PolyTree64::new();
        if !engine.execute_poly_tree(
            ClipType::Difference,
            FillRule::NonZero,
            &mut free_tree,
            None,
        ) {
            return Err(GeneralPolygonError::new(
                "Clipper free-space difference failed",
            ));
        }
        let frontier_y = exact_grid_coordinate(max_y)?;
        let mut regions = Vec::new();
        let mut output_vertices = 0;
        collect_rectangular_free_space_regions(
            &free_tree,
            PolyTree64::ROOT,
            frontier_y,
            &mut regions,
            &mut output_vertices,
        )?;
        regions.sort_by(|first, second| {
            second
                .frontier_contact_grid
                .cmp(&first.frontier_contact_grid)
                .then_with(|| second.doubled_area_grid2.cmp(&first.doubled_area_grid2))
                .then_with(|| {
                    first
                        .frontier_point_contact_only
                        .cmp(&second.frontier_point_contact_only)
                })
        });
        Ok(RectangularFreeSpaceTopology {
            regions,
            input_vertices,
            output_vertices,
        })
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

fn exact_path_doubled_area_grid2(path: &Path64) -> Result<i128, GeneralPolygonError> {
    path.iter()
        .zip(path.iter().cycle().skip(1))
        .take(path.len())
        .try_fold(0_i128, |total, (first, second)| {
            let first_x = exact_grid_coordinate(first.x)? as i128;
            let first_y = exact_grid_coordinate(first.y)? as i128;
            let second_x = exact_grid_coordinate(second.x)? as i128;
            let second_y = exact_grid_coordinate(second.y)? as i128;
            let term = first_x
                .checked_mul(second_y)
                .and_then(|left| {
                    second_x
                        .checked_mul(first_y)
                        .and_then(|right| left.checked_sub(right))
                })
                .ok_or_else(|| GeneralPolygonError::new("exact grid area overflow"))?;
            total
                .checked_add(term)
                .ok_or_else(|| GeneralPolygonError::new("exact grid area overflow"))
        })
}

fn exact_grid_coordinate(value: f64) -> Result<i64, GeneralPolygonError> {
    if !value.is_finite()
        || value.fract() != 0.0
        || value < i64::MIN as f64
        || value > i64::MAX as f64
    {
        return Err(GeneralPolygonError::new(
            "Clipper output left the exact integer grid",
        ));
    }
    Ok(value as i64)
}

fn collect_rectangular_free_space_regions(
    tree: &PolyTree64,
    parent: usize,
    frontier_y: i64,
    regions: &mut Vec<RectangularFreeSpaceRegion>,
    output_vertices: &mut usize,
) -> Result<(), GeneralPolygonError> {
    for index in 0..tree.count(parent) {
        let child = tree.child(parent, index);
        if let Some(path) = tree.poly(child) {
            *output_vertices = output_vertices
                .checked_add(path.len())
                .ok_or_else(|| GeneralPolygonError::new("free-space output-vertex overflow"))?;
        }
        if !tree.is_hole(child) {
            let outer = tree
                .poly(child)
                .ok_or_else(|| GeneralPolygonError::new("Clipper returned an empty free region"))?;
            let mut doubled_area_grid2 = exact_path_doubled_area_grid2(outer)?.abs();
            let (frontier_contact_grid, frontier_touches) = frontier_contact(outer, frontier_y)?;
            let outer_ring = PolygonRing::from_path(outer, RingRole::Outer)?;
            let mut hole_rings = Vec::new();
            for hole_index in 0..tree.count(child) {
                let hole = tree.child(child, hole_index);
                if tree.is_hole(hole) {
                    let path = tree.poly(hole).ok_or_else(|| {
                        GeneralPolygonError::new("Clipper returned an empty free-space hole")
                    })?;
                    doubled_area_grid2 = doubled_area_grid2
                        .checked_sub(exact_path_doubled_area_grid2(path)?.abs())
                        .ok_or_else(|| GeneralPolygonError::new("exact grid area overflow"))?;
                    hole_rings.push(PolygonRing::from_path(path, RingRole::Hole)?);
                }
            }
            if doubled_area_grid2 <= 0 {
                return Err(GeneralPolygonError::new(
                    "free-space region must have positive exact grid area",
                ));
            }
            regions.push(RectangularFreeSpaceRegion {
                doubled_area_grid2,
                frontier_contact_grid,
                frontier_point_contact_only: frontier_touches && frontier_contact_grid == 0,
                polygon: PolygonSet {
                    regions: vec![PolygonRegion {
                        outer: outer_ring,
                        holes: hole_rings,
                    }],
                },
            });
        }
        collect_rectangular_free_space_regions(tree, child, frontier_y, regions, output_vertices)?;
    }
    Ok(())
}

fn frontier_contact(path: &Path64, frontier_y: i64) -> Result<(i64, bool), GeneralPolygonError> {
    let mut contact = 0_i64;
    let mut touches = false;
    for (first, second) in path
        .iter()
        .zip(path.iter().cycle().skip(1))
        .take(path.len())
    {
        let first_x = exact_grid_coordinate(first.x)?;
        let first_y = exact_grid_coordinate(first.y)?;
        let second_x = exact_grid_coordinate(second.x)?;
        let second_y = exact_grid_coordinate(second.y)?;
        touches |= first_y == frontier_y || second_y == frontier_y;
        if first_y == frontier_y && second_y == frontier_y {
            let length = i64::try_from(first_x.abs_diff(second_x))
                .map_err(|_| GeneralPolygonError::new("free-space frontier-contact overflow"))?;
            contact = contact
                .checked_add(length)
                .ok_or_else(|| GeneralPolygonError::new("free-space frontier-contact overflow"))?;
        }
    }
    Ok((contact, touches))
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExactRationalScalar {
    numerator: i128,
    denominator: i128,
}

impl ExactRationalScalar {
    fn new(numerator: i128, denominator: i128) -> Result<Self, GeneralPolygonError> {
        if denominator == 0 {
            return Err(GeneralPolygonError::new(
                "exact arrangement produced a zero denominator",
            ));
        }
        let (numerator, denominator) = if denominator < 0 {
            (
                numerator
                    .checked_neg()
                    .ok_or_else(|| GeneralPolygonError::new("exact arrangement overflow"))?,
                denominator
                    .checked_neg()
                    .ok_or_else(|| GeneralPolygonError::new("exact arrangement overflow"))?,
            )
        } else {
            (numerator, denominator)
        };
        let gcd = exact_i128_gcd(numerator, denominator);
        Ok(Self {
            numerator: numerator / gcd,
            denominator: denominator / gcd,
        })
    }

    fn is_between_zero_and_one(self) -> bool {
        self.numerator >= 0 && self.numerator <= self.denominator
    }

    fn cmp(self, other: Self) -> Result<Ordering, GeneralPolygonError> {
        let left = self
            .numerator
            .checked_mul(other.denominator)
            .ok_or_else(|| GeneralPolygonError::new("exact arrangement ratio overflow"))?;
        let right = other
            .numerator
            .checked_mul(self.denominator)
            .ok_or_else(|| GeneralPolygonError::new("exact arrangement ratio overflow"))?;
        Ok(left.cmp(&right))
    }
}

fn exact_i128_gcd(first: i128, second: i128) -> i128 {
    let mut first = first.checked_abs().unwrap_or(i128::MAX);
    let mut second = second.checked_abs().unwrap_or(i128::MAX);
    while second != 0 {
        let remainder = first % second;
        first = second;
        second = remainder;
    }
    first.max(1)
}

fn exact_cross(
    first_x: i128,
    first_y: i128,
    second_x: i128,
    second_y: i128,
) -> Result<i128, GeneralPolygonError> {
    first_x
        .checked_mul(second_y)
        .and_then(|left| {
            second_x
                .checked_mul(first_y)
                .and_then(|right| left.checked_sub(right))
        })
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement cross-product overflow"))
}

fn exact_point_coordinates(point: Point64) -> (i128, i128) {
    (point.x as i64 as i128, point.y as i64 as i128)
}

fn exact_segment_parameter(
    start: Point64,
    end: Point64,
    point: Point64,
) -> Result<Option<ExactRationalScalar>, GeneralPolygonError> {
    if !point_on_segment(point, start, end) {
        return Ok(None);
    }
    let (start_x, start_y) = exact_point_coordinates(start);
    let (end_x, end_y) = exact_point_coordinates(end);
    let (point_x, point_y) = exact_point_coordinates(point);
    let dx = end_x
        .checked_sub(start_x)
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement edge overflow"))?;
    let dy = end_y
        .checked_sub(start_y)
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement edge overflow"))?;
    let numerator = if dx.abs() >= dy.abs() {
        point_x
            .checked_sub(start_x)
            .ok_or_else(|| GeneralPolygonError::new("exact arrangement parameter overflow"))?
    } else {
        point_y
            .checked_sub(start_y)
            .ok_or_else(|| GeneralPolygonError::new("exact arrangement parameter overflow"))?
    };
    let denominator = if dx.abs() >= dy.abs() { dx } else { dy };
    if denominator == 0 {
        return Ok(Some(ExactRationalScalar::new(0, 1)?));
    }
    Ok(Some(ExactRationalScalar::new(numerator, denominator)?))
}

fn exact_segment_intersection_parameters(
    first_start: Point64,
    first_end: Point64,
    second_start: Point64,
    second_end: Point64,
) -> Result<Vec<ExactRationalScalar>, GeneralPolygonError> {
    let (first_x, first_y) = exact_point_coordinates(first_start);
    let (second_x, second_y) = exact_point_coordinates(first_end);
    let (third_x, third_y) = exact_point_coordinates(second_start);
    let (fourth_x, fourth_y) = exact_point_coordinates(second_end);
    let first_dx = second_x
        .checked_sub(first_x)
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement edge overflow"))?;
    let first_dy = second_y
        .checked_sub(first_y)
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement edge overflow"))?;
    let second_dx = fourth_x
        .checked_sub(third_x)
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement edge overflow"))?;
    let second_dy = fourth_y
        .checked_sub(third_y)
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement edge overflow"))?;
    let offset_x = third_x
        .checked_sub(first_x)
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement edge overflow"))?;
    let offset_y = third_y
        .checked_sub(first_y)
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement edge overflow"))?;
    let denominator = exact_cross(first_dx, first_dy, second_dx, second_dy)?;
    if denominator != 0 {
        let parameter_numerator = exact_cross(offset_x, offset_y, second_dx, second_dy)?;
        let second_parameter_numerator = exact_cross(offset_x, offset_y, first_dx, first_dy)?;
        let parameter = ExactRationalScalar::new(parameter_numerator, denominator)?;
        let second_parameter = ExactRationalScalar::new(second_parameter_numerator, denominator)?;
        if parameter.is_between_zero_and_one() && second_parameter.is_between_zero_and_one() {
            return Ok(vec![parameter]);
        }
        return Ok(Vec::new());
    }

    if exact_cross(offset_x, offset_y, first_dx, first_dy)? != 0 {
        return Ok(Vec::new());
    }
    let mut parameters = Vec::with_capacity(2);
    for point in [second_start, second_end] {
        if let Some(parameter) = exact_segment_parameter(first_start, first_end, point)? {
            parameters.push(parameter);
        }
    }
    Ok(parameters)
}

fn exact_segments_have_proper_crossing(
    first_start: Point64,
    first_end: Point64,
    second_start: Point64,
    second_end: Point64,
) -> Result<bool, GeneralPolygonError> {
    let (first_x, first_y) = exact_point_coordinates(first_start);
    let (first_end_x, first_end_y) = exact_point_coordinates(first_end);
    let (second_x, second_y) = exact_point_coordinates(second_start);
    let (second_end_x, second_end_y) = exact_point_coordinates(second_end);
    let first_dx = first_end_x
        .checked_sub(first_x)
        .ok_or_else(|| GeneralPolygonError::new("material graph edge overflow"))?;
    let first_dy = first_end_y
        .checked_sub(first_y)
        .ok_or_else(|| GeneralPolygonError::new("material graph edge overflow"))?;
    let second_dx = second_end_x
        .checked_sub(second_x)
        .ok_or_else(|| GeneralPolygonError::new("material graph edge overflow"))?;
    let second_dy = second_end_y
        .checked_sub(second_y)
        .ok_or_else(|| GeneralPolygonError::new("material graph edge overflow"))?;
    if exact_cross(first_dx, first_dy, second_dx, second_dy)? == 0 {
        return Ok(false);
    }
    let first_parameters =
        exact_segment_intersection_parameters(first_start, first_end, second_start, second_end)?;
    let second_parameters =
        exact_segment_intersection_parameters(second_start, second_end, first_start, first_end)?;
    let first_has_interior_parameter = first_parameters
        .iter()
        .any(|parameter| parameter.numerator > 0 && parameter.numerator < parameter.denominator);
    let second_has_interior_parameter = second_parameters
        .iter()
        .any(|parameter| parameter.numerator > 0 && parameter.numerator < parameter.denominator);
    Ok(first_has_interior_parameter && second_has_interior_parameter)
}

fn exact_segments_have_positive_collinear_overlap(
    first_start: Point64,
    first_end: Point64,
    second_start: Point64,
    second_end: Point64,
) -> Result<bool, GeneralPolygonError> {
    let (first_x, first_y) = exact_point_coordinates(first_start);
    let (first_end_x, first_end_y) = exact_point_coordinates(first_end);
    let (second_x, second_y) = exact_point_coordinates(second_start);
    let (second_end_x, second_end_y) = exact_point_coordinates(second_end);
    let first_dx = first_end_x
        .checked_sub(first_x)
        .ok_or_else(|| GeneralPolygonError::new("material graph edge overflow"))?;
    let first_dy = first_end_y
        .checked_sub(first_y)
        .ok_or_else(|| GeneralPolygonError::new("material graph edge overflow"))?;
    let second_dx = second_end_x
        .checked_sub(second_x)
        .ok_or_else(|| GeneralPolygonError::new("material graph edge overflow"))?;
    let second_dy = second_end_y
        .checked_sub(second_y)
        .ok_or_else(|| GeneralPolygonError::new("material graph edge overflow"))?;
    if first_dx == 0 && first_dy == 0 || second_dx == 0 && second_dy == 0 {
        return Ok(false);
    }
    let offset_x = second_x
        .checked_sub(first_x)
        .ok_or_else(|| GeneralPolygonError::new("material graph edge overflow"))?;
    let offset_y = second_y
        .checked_sub(first_y)
        .ok_or_else(|| GeneralPolygonError::new("material graph edge overflow"))?;
    if exact_cross(first_dx, first_dy, offset_x, offset_y)? != 0 {
        return Ok(false);
    }
    if first_dx.abs() >= first_dy.abs() {
        let first_low = first_x.min(first_end_x);
        let first_high = first_x.max(first_end_x);
        let second_low = second_x.min(second_end_x);
        let second_high = second_x.max(second_end_x);
        Ok(first_low.max(second_low) < first_high.min(second_high))
    } else {
        let first_low = first_y.min(first_end_y);
        let first_high = first_y.max(first_end_y);
        let second_low = second_y.min(second_end_y);
        let second_high = second_y.max(second_end_y);
        Ok(first_low.max(second_low) < first_high.min(second_high))
    }
}

fn exact_point_on_segment_parameter(
    start: Point64,
    end: Point64,
    parameter: ExactRationalScalar,
) -> Result<ExactRationalPoint, GeneralPolygonError> {
    let (start_x, start_y) = exact_point_coordinates(start);
    let (end_x, end_y) = exact_point_coordinates(end);
    let dx = end_x
        .checked_sub(start_x)
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement edge overflow"))?;
    let dy = end_y
        .checked_sub(start_y)
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement edge overflow"))?;
    let x_num = start_x
        .checked_mul(parameter.denominator)
        .and_then(|value| {
            dx.checked_mul(parameter.numerator)
                .and_then(|delta| value.checked_add(delta))
        })
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement point overflow"))?;
    let y_num = start_y
        .checked_mul(parameter.denominator)
        .and_then(|value| {
            dy.checked_mul(parameter.numerator)
                .and_then(|delta| value.checked_add(delta))
        })
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement point overflow"))?;
    Ok(ExactRationalPoint {
        x_num,
        y_num,
        den: parameter.denominator,
    })
}

fn exact_midpoint_parameter(
    first: ExactRationalScalar,
    second: ExactRationalScalar,
) -> Result<ExactRationalScalar, GeneralPolygonError> {
    let numerator = first
        .numerator
        .checked_mul(second.denominator)
        .and_then(|left| {
            second
                .numerator
                .checked_mul(first.denominator)
                .and_then(|right| left.checked_add(right))
        })
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement midpoint overflow"))?;
    let denominator = first
        .denominator
        .checked_mul(second.denominator)
        .and_then(|value| value.checked_mul(2))
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement midpoint overflow"))?;
    ExactRationalScalar::new(numerator, denominator)
}

fn exact_material_subset_scratch_bytes(
    baseline: &PolygonSet,
    container: &PolygonSet,
) -> Result<usize, GeneralPolygonError> {
    let container_edges = polygon_set_edge_count(container)?;
    let baseline_edges = polygon_set_edge_count(baseline)?;
    let mut peak = 0_usize;
    for ring in polygon_set_rings(baseline) {
        peak = peak.max(exact_boundary_interval_scratch_bytes(
            ring.path.len(),
            container_edges,
        )?);
    }
    for ring in polygon_set_rings(container) {
        peak = peak.max(exact_boundary_interval_scratch_bytes(
            ring.path.len(),
            baseline_edges,
        )?);
    }
    Ok(peak)
}

fn exact_boundary_interval_scratch_bytes(
    path_vertices: usize,
    other_edges: usize,
) -> Result<usize, GeneralPolygonError> {
    let intersection_parameters_per_edge = other_edges
        .checked_mul(2)
        .and_then(|count| count.checked_add(2))
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement scratch overflow"))?;
    let midpoint_capacity = path_vertices
        .checked_mul(
            intersection_parameters_per_edge
                .checked_sub(1)
                .ok_or_else(|| GeneralPolygonError::new("exact arrangement scratch overflow"))?,
        )
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement scratch overflow"))?;
    let vector_header = size_of::<Vec<()>>();
    let edge_bytes = vector_header
        .checked_add(
            other_edges
                .checked_mul(size_of::<(Point64, Point64)>())
                .ok_or_else(|| GeneralPolygonError::new("exact arrangement scratch overflow"))?,
        )
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement scratch overflow"))?;
    let midpoint_bytes = vector_header
        .checked_add(
            midpoint_capacity
                .checked_mul(size_of::<ExactRationalPoint>())
                .ok_or_else(|| GeneralPolygonError::new("exact arrangement scratch overflow"))?,
        )
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement scratch overflow"))?;
    let scalar_vector_bytes = vector_header
        .checked_add(
            intersection_parameters_per_edge
                .checked_mul(size_of::<ExactRationalScalar>())
                .ok_or_else(|| GeneralPolygonError::new("exact arrangement scratch overflow"))?,
        )
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement scratch overflow"))?;
    let intersection_vector_bytes = vector_header
        .checked_add(
            2usize
                .checked_mul(size_of::<ExactRationalScalar>())
                .ok_or_else(|| GeneralPolygonError::new("exact arrangement scratch overflow"))?,
        )
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement scratch overflow"))?;
    edge_bytes
        .checked_add(midpoint_bytes)
        .and_then(|bytes| bytes.checked_add(scalar_vector_bytes))
        .and_then(|bytes| bytes.checked_add(scalar_vector_bytes))
        .and_then(|bytes| bytes.checked_add(intersection_vector_bytes))
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement scratch overflow"))
}

fn exact_boundary_intervals(
    path: &Path64,
    other: &PolygonSet,
) -> Result<(Vec<ExactRationalPoint>, usize), GeneralPolygonError> {
    let other_edge_count = polygon_set_edge_count(other)?;
    let scratch_bytes = exact_boundary_interval_scratch_bytes(path.len(), other_edge_count)?;
    if scratch_bytes > GENERAL_EXACT_ARRANGEMENT_SCRATCH_CAP_BYTES {
        return Err(GeneralPolygonError::new(format!(
            "exact boundary arrangement scratch requires {scratch_bytes} bytes, exceeding the {GENERAL_EXACT_ARRANGEMENT_SCRATCH_CAP_BYTES}-byte cap"
        )));
    }
    let other_edges = polygon_set_edges(other, other_edge_count);
    let midpoint_capacity = path
        .len()
        .checked_mul(
            other_edge_count
                .checked_mul(2)
                .and_then(|count| count.checked_add(1))
                .ok_or_else(|| GeneralPolygonError::new("exact arrangement scratch overflow"))?,
        )
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement scratch overflow"))?;
    let parameter_capacity = other_edge_count
        .checked_mul(2)
        .and_then(|count| count.checked_add(2))
        .ok_or_else(|| GeneralPolygonError::new("exact arrangement scratch overflow"))?;
    let mut midpoints = Vec::with_capacity(midpoint_capacity);
    let mut edge_checks = 0_usize;
    for (start, end) in path
        .iter()
        .zip(path.iter().cycle().skip(1))
        .take(path.len())
    {
        edge_checks = edge_checks
            .checked_add(other_edges.len())
            .ok_or_else(|| GeneralPolygonError::new("exact subset work overflow"))?;
        let mut parameters = Vec::with_capacity(parameter_capacity);
        parameters.push(ExactRationalScalar::new(0, 1)?);
        parameters.push(ExactRationalScalar::new(1, 1)?);
        for (other_start, other_end) in &other_edges {
            parameters.extend(exact_segment_intersection_parameters(
                *start,
                *end,
                *other_start,
                *other_end,
            )?);
        }
        for index in 1..parameters.len() {
            let mut cursor = index;
            while cursor > 0 && parameters[cursor - 1].cmp(parameters[cursor])? == Ordering::Greater
            {
                parameters.swap(cursor - 1, cursor);
                cursor -= 1;
            }
        }
        let mut unique: Vec<ExactRationalScalar> = Vec::with_capacity(parameter_capacity);
        for parameter in parameters {
            if unique
                .last()
                .is_none_or(|previous| previous.cmp(parameter) != Ok(Ordering::Equal))
            {
                unique.push(parameter);
            }
        }
        for pair in unique.windows(2) {
            if pair[0].cmp(pair[1])? == Ordering::Less {
                let midpoint = exact_midpoint_parameter(pair[0], pair[1])?;
                midpoints.push(exact_point_on_segment_parameter(*start, *end, midpoint)?);
            }
        }
    }
    Ok((midpoints, edge_checks))
}

fn polygon_set_rings(polygon: &PolygonSet) -> impl Iterator<Item = &PolygonRing> {
    polygon
        .regions
        .iter()
        .flat_map(|region| std::iter::once(&region.outer).chain(region.holes.iter()))
}

fn polygon_set_edge_count(polygon: &PolygonSet) -> Result<usize, GeneralPolygonError> {
    polygon_set_rings(polygon).try_fold(0_usize, |count, ring| {
        count
            .checked_add(ring.path.len())
            .ok_or_else(|| GeneralPolygonError::new("exact arrangement edge-count overflow"))
    })
}

fn polygon_set_edges(polygon: &PolygonSet, edge_count: usize) -> Vec<(Point64, Point64)> {
    let mut edges = Vec::with_capacity(edge_count);
    for ring in polygon_set_rings(polygon) {
        edges.extend(
            ring.path
                .iter()
                .copied()
                .zip(ring.path.iter().copied().cycle().skip(1))
                .take(ring.path.len()),
        );
    }
    edges
}

fn exact_boundary_is_in_material_closure(
    path: &Path64,
    container: &PolygonSet,
) -> Result<(bool, usize), GeneralPolygonError> {
    let (midpoints, edge_checks) = exact_boundary_intervals(path, container)?;
    let valid = midpoints.into_iter().try_fold(true, |valid, midpoint| {
        if !valid {
            return Ok(false);
        }
        Ok(matches!(
            container.exact_rational_location(midpoint),
            ExactRationalPointLocation::Inside | ExactRationalPointLocation::On
        ))
    })?;
    Ok((valid, edge_checks))
}

fn exact_boundary_runs_through_material(
    path: &Path64,
    baseline: &PolygonSet,
) -> Result<(bool, usize), GeneralPolygonError> {
    let (midpoints, edge_checks) = exact_boundary_intervals(path, baseline)?;
    let runs_through = midpoints
        .into_iter()
        .try_fold(false, |runs_through, midpoint| {
            Ok(runs_through
                || baseline.exact_rational_location(midpoint) == ExactRationalPointLocation::Inside)
        })?;
    Ok((runs_through, edge_checks))
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

fn exact_rational_ring_location(
    point: ExactRationalPoint,
    path: &Path64,
) -> ExactRationalPointLocation {
    if path.len() < 3 || point.den <= 0 {
        return ExactRationalPointLocation::Outside;
    }

    let mut winding = 0_i32;
    for (first, second) in path
        .iter()
        .zip(path.iter().cycle().skip(1))
        .take(path.len())
    {
        let first_x = first.x as i128;
        let first_y = first.y as i128;
        let second_x = second.x as i128;
        let second_y = second.y as i128;
        let point_x = first_x
            .checked_mul(point.den)
            .and_then(|scaled| point.x_num.checked_sub(scaled));
        let point_y = first_y
            .checked_mul(point.den)
            .and_then(|scaled| point.y_num.checked_sub(scaled));
        let edge_x = second_x.checked_sub(first_x);
        let edge_y = second_y.checked_sub(first_y);
        let (Some(point_x), Some(point_y), Some(edge_x), Some(edge_y)) =
            (point_x, point_y, edge_x, edge_y)
        else {
            // the public grid contract bounds these products. treating an
            // impossible overflow as outside is still fail-closed for the
            // diagnostic witness classifier.
            return ExactRationalPointLocation::Outside;
        };
        let cross = edge_x.checked_mul(point_y).and_then(|left| {
            edge_y
                .checked_mul(point_x)
                .and_then(|right| left.checked_sub(right))
        });
        let Some(cross) = cross else {
            return ExactRationalPointLocation::Outside;
        };
        if cross == 0
            && first_x
                .min(second_x)
                .checked_mul(point.den)
                .is_some_and(|bound| point.x_num >= bound)
            && first_x
                .max(second_x)
                .checked_mul(point.den)
                .is_some_and(|bound| point.x_num <= bound)
            && first_y
                .min(second_y)
                .checked_mul(point.den)
                .is_some_and(|bound| point.y_num >= bound)
            && first_y
                .max(second_y)
                .checked_mul(point.den)
                .is_some_and(|bound| point.y_num <= bound)
        {
            return ExactRationalPointLocation::On;
        }

        let (Some(first_y_scaled), Some(second_y_scaled)) = (
            first_y.checked_mul(point.den),
            second_y.checked_mul(point.den),
        ) else {
            return ExactRationalPointLocation::Outside;
        };
        if first_y_scaled <= point.y_num {
            if second_y_scaled > point.y_num && cross > 0 {
                winding += 1;
            }
        } else if second_y_scaled <= point.y_num && cross < 0 {
            winding -= 1;
        }
    }
    if winding == 0 {
        ExactRationalPointLocation::Outside
    } else {
        ExactRationalPointLocation::Inside
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

    fn rectangle(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> PolygonSet {
        PolygonSet::from_outer(vec![
            point(min_x, min_y),
            point(max_x, min_y),
            point(max_x, max_y),
            point(min_x, max_y),
        ])
        .unwrap()
    }

    #[test]
    fn rectangular_free_space_reports_one_frontier_connected_empty_region() {
        let topology =
            PolygonSet::rectangular_free_space_topology(&[], 0.0, 0.0, 10.0, 10.0).unwrap();
        assert_eq!(topology.input_vertices, 4);
        assert_eq!(topology.output_vertices, 4);
        assert_eq!(topology.regions.len(), 1);
        assert_eq!(topology.regions[0].doubled_area_grid2, 200_000_000);
        assert_eq!(topology.regions[0].frontier_contact_grid, 10_000);
        assert!(!topology.regions[0].frontier_point_contact_only);
    }

    #[test]
    fn rectangular_free_space_distinguishes_a_frontier_region_across_a_barrier() {
        let barrier = rectangle(0.0, 4.0, 10.0, 6.0);
        let topology =
            PolygonSet::rectangular_free_space_topology(&[&barrier], 0.0, 0.0, 10.0, 10.0).unwrap();
        assert_eq!(topology.regions.len(), 2);
        assert_eq!(topology.regions[0].doubled_area_grid2, 80_000_000);
        assert_eq!(topology.regions[0].frontier_contact_grid, 10_000);
        assert_eq!(topology.regions[1].doubled_area_grid2, 80_000_000);
        assert_eq!(topology.regions[1].frontier_contact_grid, 0);
        for region in &topology.regions {
            assert_eq!(
                region.polygon.exact_doubled_area_grid2().unwrap(),
                region.doubled_area_grid2
            );
        }
    }

    #[test]
    fn exact_component_witness_is_strictly_inside_and_deterministic() {
        let region = rectangle(0.0, 0.0, 10.0, 10.0);
        let first = region.strict_interior_witness().unwrap();
        let second = region.strict_interior_witness().unwrap();
        assert_eq!(first, second);
        assert_eq!(
            region.exact_rational_location(first),
            ExactRationalPointLocation::Inside
        );
    }

    #[test]
    fn exact_component_witness_rejects_boundary_classification() {
        let region = rectangle(0.0, 0.0, 10.0, 10.0);
        assert_eq!(
            region.exact_rational_location(ExactRationalPoint {
                x_num: 10,
                y_num: 0,
                den: 1,
            }),
            ExactRationalPointLocation::On
        );
    }

    #[test]
    fn exact_component_witness_stays_outside_a_hole() {
        let region = PolygonSet::new(vec![PolygonRegion::new(
            vec![
                point(0.0, 0.0),
                point(20.0, 0.0),
                point(20.0, 20.0),
                point(0.0, 20.0),
            ],
            vec![vec![
                point(8.0, 8.0),
                point(12.0, 8.0),
                point(12.0, 12.0),
                point(8.0, 12.0),
            ]],
        )
        .unwrap()])
        .unwrap();
        let witness = region.strict_interior_witness().unwrap();
        assert_eq!(
            region.exact_rational_location(witness),
            ExactRationalPointLocation::Inside
        );
    }

    #[test]
    fn exact_material_subset_accepts_containment_and_boundary_touching() {
        let inner = rectangle(0.0, 0.0, 10.0, 10.0);
        let outer = rectangle(0.0, 0.0, 20.0, 20.0);
        assert!(inner.exact_material_subset_of(&outer).unwrap());
        assert!(outer.exact_material_subset_of(&outer).unwrap());
    }

    #[test]
    fn exact_material_subset_rejects_a_counterfactual_hole_inside_material() {
        let baseline = rectangle(0.0, 0.0, 20.0, 20.0);
        let counter = PolygonSet::new(vec![PolygonRegion::new(
            vec![
                point(0.0, 0.0),
                point(20.0, 0.0),
                point(20.0, 20.0),
                point(0.0, 20.0),
            ],
            vec![vec![
                point(8.0, 8.0),
                point(12.0, 8.0),
                point(12.0, 12.0),
                point(8.0, 12.0),
            ]],
        )
        .unwrap()])
        .unwrap();
        assert!(!baseline.exact_material_subset_of(&counter).unwrap());
    }

    #[test]
    fn exact_material_subset_rejects_a_disconnected_counter_region_inside_material() {
        let baseline = rectangle(0.0, 0.0, 20.0, 20.0);
        let counter = PolygonSet::new(vec![
            PolygonRegion::new(
                vec![
                    point(0.0, 0.0),
                    point(20.0, 0.0),
                    point(20.0, 2.0),
                    point(0.0, 2.0),
                ],
                Vec::new(),
            )
            .unwrap(),
            PolygonRegion::new(
                vec![
                    point(8.0, 8.0),
                    point(12.0, 8.0),
                    point(12.0, 12.0),
                    point(8.0, 12.0),
                ],
                Vec::new(),
            )
            .unwrap(),
        ])
        .unwrap();
        assert!(!baseline.exact_material_subset_of(&counter).unwrap());
    }

    #[test]
    fn material_graph_recovers_the_exact_grid_sliver_without_relaxing_subset() {
        let baseline_polygon = PolygonSet::from_outer(vec![
            point(652.648, 69.013),
            point(655.494, 69.013),
            point(654.483, 71.958),
        ])
        .unwrap();
        let counter_polygon = PolygonSet::new(vec![PolygonRegion::new(
            vec![
                point(489.055, 3.5),
                point(653.62, 3.5),
                point(675.256, 68.412),
                point(655.7, 68.412),
                point(654.483, 71.958),
                point(652.463, 68.717),
                point(574.449, 71.536),
                point(588.198, 93.924),
                point(587.279, 95.52),
                point(509.6, 95.52),
                point(508.109, 98.115),
                point(504.318, 91.615),
                point(464.983, 159.047),
                point(466.392, 161.5),
                point(275.96, 161.5),
                point(358.758, 148.35),
                point(353.159, 113.104),
                point(374.917, 113.104),
                point(373.558, 115.499),
                point(402.05, 159.928),
                point(450.973, 161.497),
                point(482.254, 118.985),
                point(480.273, 114.91),
                point(423.999, 113.104),
                point(489.055, 113.104),
            ],
            vec![vec![
                point(491.301, 13.383),
                point(489.138, 17.15),
                point(526.598, 81.367),
                point(530.941, 81.337),
                point(569.367, 13.383),
            ]],
        )
        .unwrap()])
        .unwrap();
        assert_eq!(
            baseline_polygon.exact_doubled_area_grid2().unwrap(),
            8_381_470
        );
        assert_eq!(
            counter_polygon.exact_doubled_area_grid2().unwrap(),
            27_577_869_626
        );

        let witness = ExactRationalPoint {
            x_num: 1_308_142,
            y_num: 140_872,
            den: 2,
        };
        assert_eq!(
            baseline_polygon.exact_rational_location(witness),
            ExactRationalPointLocation::Inside
        );
        assert_eq!(
            counter_polygon.exact_rational_location(witness),
            ExactRationalPointLocation::Inside
        );
        assert!(!baseline_polygon
            .exact_material_subset_of(&counter_polygon)
            .unwrap());

        let (connected, _) = baseline_polygon
            .material_overlap_or_shared_segment(&counter_polygon)
            .unwrap();
        assert!(connected);
    }

    #[test]
    fn material_graph_excludes_point_contact_but_accepts_shared_segment() {
        let point_first = rectangle(0.0, 0.0, 1.0, 1.0);
        let point_second = rectangle(1.0, 1.0, 2.0, 2.0);
        assert!(
            !point_first
                .material_overlap_or_shared_segment(&point_second)
                .unwrap()
                .0
        );

        let segment_first = rectangle(0.0, 0.0, 1.0, 1.0);
        let segment_second = rectangle(1.0, 0.0, 2.0, 1.0);
        let (connected, edge_checks) = segment_first
            .material_overlap_or_shared_segment(&segment_second)
            .unwrap();
        assert!(connected);
        assert!(edge_checks > 0);
    }

    #[test]
    fn material_graph_excludes_interior_to_endpoint_contact() {
        let first_start = Point64::new(0.0, 0.0, 0.0);
        let first_end = Point64::new(10_000.0, 0.0, 0.0);
        let second_start = Point64::new(5_000.0, 0.0, 0.0);
        let second_end = Point64::new(5_000.0, 5_000.0, 0.0);
        assert!(!exact_segments_have_proper_crossing(
            first_start,
            first_end,
            second_start,
            second_end,
        )
        .unwrap());

        let crossing_start = Point64::new(0.0, 0.0, 0.0);
        let crossing_end = Point64::new(10_000.0, 10_000.0, 0.0);
        let other_start = Point64::new(0.0, 10_000.0, 0.0);
        let other_end = Point64::new(10_000.0, 0.0, 0.0);
        assert!(exact_segments_have_proper_crossing(
            crossing_start,
            crossing_end,
            other_start,
            other_end,
        )
        .unwrap());
    }

    #[test]
    fn exact_material_subset_handles_a_baseline_hole() {
        let baseline = PolygonSet::new(vec![PolygonRegion::new(
            vec![
                point(0.0, 0.0),
                point(20.0, 0.0),
                point(20.0, 20.0),
                point(0.0, 20.0),
            ],
            vec![vec![
                point(8.0, 8.0),
                point(12.0, 8.0),
                point(12.0, 12.0),
                point(8.0, 12.0),
            ]],
        )
        .unwrap()])
        .unwrap();
        let counter = rectangle(-5.0, -5.0, 25.0, 25.0);
        assert!(baseline.exact_material_subset_of(&counter).unwrap());
    }

    #[test]
    fn rectangular_free_space_retains_an_enclosed_vacancy_component() {
        let left = rectangle(3.0, 3.0, 4.0, 7.0);
        let right = rectangle(6.0, 3.0, 7.0, 7.0);
        let bottom = rectangle(3.0, 3.0, 7.0, 4.0);
        let top = rectangle(3.0, 6.0, 7.0, 7.0);
        let topology = PolygonSet::rectangular_free_space_topology(
            &[&left, &right, &bottom, &top],
            0.0,
            0.0,
            10.0,
            10.0,
        )
        .unwrap();
        assert_eq!(topology.regions.len(), 2);
        assert_eq!(topology.regions[0].doubled_area_grid2, 168_000_000);
        assert_eq!(topology.regions[0].frontier_contact_grid, 10_000);
        assert_eq!(topology.regions[1].doubled_area_grid2, 8_000_000);
        assert_eq!(topology.regions[1].frontier_contact_grid, 0);
    }

    #[test]
    fn frontier_contact_distinguishes_a_point_from_a_segment() {
        let point_only = vec![
            Point64::new(0.0, 0.0, 0.0),
            Point64::new(5_000.0, 10_000.0, 0.0),
            Point64::new(10_000.0, 0.0, 0.0),
        ];
        let segment = vec![
            Point64::new(0.0, 0.0, 0.0),
            Point64::new(0.0, 10_000.0, 0.0),
            Point64::new(10_000.0, 10_000.0, 0.0),
            Point64::new(10_000.0, 0.0, 0.0),
        ];
        assert_eq!(frontier_contact(&point_only, 10_000).unwrap(), (0, true));
        assert_eq!(frontier_contact(&segment, 10_000).unwrap(), (10_000, true));
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
