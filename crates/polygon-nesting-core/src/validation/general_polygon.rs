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
    validate_publication_inner(placements, settings, false)
}

/// [`validate_publication`] with its all-pairs clearance loop spread over the
/// job pool.
///
/// This is where a mode-34 confirmation's milliseconds actually are, and the
/// measurement that says so is in
/// docs/experiments/parallel-compression-schedule/: of the 4.92 ms an accepted
/// confirmation costs on the mixed-61 band, the collision-grid overlap loop in
/// `validate_and_measure_placements` is **0.13 ms** and this function is
/// essentially all the rest. The reason is the `n * (n - 1) / 2` calls to
/// [`minimum_boundary_distance`], which walks every edge of one material set
/// against every edge of the other - the exact-clearance contract is a
/// boundary-distance question, not an overlap question, and no bounds reject
/// short-circuits it.
///
/// The verdict is the serial function's, including its message: each row scans
/// its own second indices and the reduce returns the lexicographically lowest
/// `(first, second)` that fails, which is the pair the serial nest returns.
/// The only difference is on the failure path, where the serial nest stops at
/// the first bad pair and this lets every row finish - more work, same answer.
#[cfg(feature = "parallel-compression-schedule")]
pub fn validate_publication_parallel(
    placements: &[GeneralPlacement<'_>],
    settings: PublicationValidationSettings,
) -> Result<(), PublicationValidationError> {
    validate_publication_inner(placements, settings, true)
}

fn validate_publication_inner(
    placements: &[GeneralPlacement<'_>],
    settings: PublicationValidationSettings,
    #[cfg_attr(
        not(feature = "parallel-compression-schedule"),
        allow(unused_variables)
    )]
    parallel: bool,
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
    // The broad phase, sealed once for the whole call: `O(total points)` to
    // build against the `O(pairs * edges^2)` it filters. Its verdict is a proof
    // of clearance, never an estimate of it - see `ClearanceBroadPhase::new`.
    #[cfg(feature = "fast-contract-validator")]
    let broad_phase = ClearanceBroadPhase::new(&transformed, pair_clearance);
    #[cfg(feature = "fast-contract-validator")]
    contract_validator_census(&broad_phase, transformed.len());
    // One row per first index. Named so the serial nest and the job-pool
    // dispatch below run the same body against the same operands in the same
    // per-row order: this is one loop with two traversals, not two loops.
    let scan_row = |first_index: usize| -> Option<PublicationValidationError> {
        for second_index in (first_index + 1)..transformed.len() {
            let first = &transformed[first_index];
            let second = &transformed[second_index];
            // A proved-clear pair can fail neither test below: a positive
            // separation is a disjointness proof as well as a clearance one.
            // The debug arm runs both of them anyway and requires the verdict
            // the skip claimed - this is the only place the feature can be
            // wrong, so it is the place that is checked.
            #[cfg(feature = "fast-contract-validator")]
            if broad_phase.provably_clear(first_index, second_index) {
                debug_assert!(
                    !material_sets_overlap(first, second),
                    "fast-contract-validator skipped an overlapping pair: {} and {}",
                    placements[first_index].piece_id,
                    placements[second_index].piece_id
                );
                debug_assert!(
                    {
                        let distance = minimum_boundary_distance(first, second);
                        distance.is_finite() && distance >= pair_clearance
                    },
                    "fast-contract-validator skipped a pair the exact loop refuses: \
                     {} and {} at {} against a clearance of {}",
                    placements[first_index].piece_id,
                    placements[second_index].piece_id,
                    minimum_boundary_distance(first, second),
                    pair_clearance
                );
                continue;
            }
            if material_sets_overlap(first, second) {
                return Some(PublicationValidationError::new(format!(
                    "pieces {} and {} overlap",
                    placements[first_index].piece_id, placements[second_index].piece_id
                )));
            }
            let distance = minimum_boundary_distance(first, second);
            if !distance.is_finite() || distance < pair_clearance {
                return Some(PublicationValidationError::new(format!(
                    "pieces {} and {} violate the required clearance",
                    placements[first_index].piece_id, placements[second_index].piece_id
                )));
            }
        }
        None
    };
    #[cfg(feature = "parallel-compression-schedule")]
    if parallel {
        let rows = (0..transformed.len()).collect::<Vec<_>>();
        let scanned = crate::parallel::map_slice_with_job_pool(&rows, |first| scan_row(*first));
        // Input order, so the first `Some` here is the lowest-indexed failing
        // row - the pair the serial nest would have returned on.
        for row in scanned {
            if let Some(error) = row {
                return Err(error);
            }
        }
        return Ok(());
    }
    for first_index in 0..transformed.len() {
        if let Some(error) = scan_row(first_index) {
            return Err(error);
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

/// Rejects a placement whose geometry or transform cannot be measured at all,
/// and returns the rotation's `(sin, cos)`.
///
/// Shared by every raw-source measurement in this module so they agree on what
/// "measurable" means before any of them reads a coordinate.
fn placement_rotation(
    placement: &GeneralPlacement<'_>,
) -> Result<(f64, f64), PublicationValidationError> {
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
    Ok(placement.rotation_deg.to_radians().sin_cos())
}

/// Places one flattened source ring under a placement's transform, in `f64`
/// throughout.
///
/// This reads [`PolygonRing::source_points`] - the untouched `f64` ring - and
/// never the canonical integer-grid path, which is the whole point of this
/// module: the search's own geometry is quantized to the grid, so a validator
/// built on it could not see a sub-grid violation.
fn transform_source_ring(
    ring: &PolygonRing,
    placement: &GeneralPlacement<'_>,
    sin: f64,
    cos: f64,
) -> Result<Vec<IrregularPoint>, PublicationValidationError> {
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
}

/// The layout's long-axis depth, measured on the untouched `f64` source rings.
///
/// This is the same `max_y + edge clearance` quantity the search reports as its
/// independent depth, and it applies exactly the transform
/// [`validate_publication`] validates under - but it never leaves `f64`. The
/// search's own measurement goes through `PolygonSet::bounds`, which reads the
/// canonical *integer-grid* path and therefore snaps to the 0.001 mm grid. At a
/// hard threshold - a 155.000 mm goal, a ladder rung's bound - a layout whose
/// true depth sits a hair above the line can snap a hair below it and appear to
/// qualify. This measurement cannot round in either direction, so it is the one
/// to quote whenever a depth is being compared against a threshold rather than
/// against another snapped depth.
///
/// The maximum is taken over the outer rings only: a hole is contained in its
/// own outer ring by construction, so it can never be the deepest point.
pub fn raw_source_long_axis_depth_mm(
    placements: &[GeneralPlacement<'_>],
    sheet_edge_clearance_mm: f64,
) -> Result<f64, PublicationValidationError> {
    if !sheet_edge_clearance_mm.is_finite() {
        return Err(PublicationValidationError::new(
            "sheet edge clearance must be finite",
        ));
    }
    let mut deepest = f64::NEG_INFINITY;
    for placement in placements {
        let (sin, cos) = placement_rotation(placement)?;
        for region in &placement.polygon.regions {
            for point in transform_source_ring(&region.outer, placement, sin, cos)? {
                deepest = deepest.max(point.y + sheet_edge_clearance_mm);
            }
        }
    }
    if deepest == f64::NEG_INFINITY {
        return Err(PublicationValidationError::new(
            "a raw-source depth needs at least one placed ring to measure",
        ));
    }
    Ok(deepest)
}

fn transform_placement(
    placement: &GeneralPlacement<'_>,
) -> Result<MaterialSet, PublicationValidationError> {
    let (sin, cos) = placement_rotation(placement)?;
    let transform_ring =
        |ring: &PolygonRing| -> Result<Vec<IrregularPoint>, PublicationValidationError> {
            transform_source_ring(ring, placement, sin, cos)
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

/// The directions a [`ClearanceSlabs`] projects onto: the two axes and the two
/// diagonals.
#[cfg(feature = "fast-contract-validator")]
const CLEARANCE_SLAB_DIRECTIONS: usize = 4;

/// The length of each projection direction's normal, in the order the
/// projections are stored: `(1,0)`, `(0,1)`, `(1,1)`, `(1,-1)`.
///
/// A gap measured along an *unnormalised* direction `d` is `|d|` times the gap
/// along its unit normal, so a distance threshold has to be scaled by `|d|`
/// before it is compared against the diagonal projections. Scaling the
/// threshold up rather than the gap down keeps the test on the strict side of
/// the rounding either way.
#[cfg(feature = "fast-contract-validator")]
const CLEARANCE_SLAB_NORMS: [f64; CLEARANCE_SLAB_DIRECTIONS] =
    [1.0, 1.0, std::f64::consts::SQRT_2, std::f64::consts::SQRT_2];

/// The floor of the proof margin, in millimetres.
///
/// See [`ClearanceBroadPhase::new`] for what this has to dominate. It is a
/// picometre: six orders of magnitude below the smallest pair clearance this
/// engine is ever asked for (`0.0005 mm`), and three to four orders *above* the
/// worst rounding error either side of the comparison can carry at the
/// coordinate magnitudes a sheet has.
#[cfg(feature = "fast-contract-validator")]
const CLEARANCE_SLAB_ABSOLUTE_MARGIN_MM: f64 = 1e-9;

/// The scale-following part of the proof margin, as a fraction of the largest
/// projection magnitude in the layout.
#[cfg(feature = "fast-contract-validator")]
const CLEARANCE_SLAB_RELATIVE_MARGIN: f64 = 1e-12;

/// One material set's extent along [`CLEARANCE_SLAB_DIRECTIONS`] fixed
/// directions, in the same untouched-`f64` millimetres the exact loop measures
/// in.
///
/// This is the discrete oriented polytope `GridSlabs` is, asking a different
/// question - "how far apart are they at least", not "do they overlap" - and
/// built on different numbers. `GridSlabs` projects the canonical *integer*
/// grid, and this module's entire premise (see the module docs and
/// [`transform_source_ring`]) is that the quantized geometry cannot answer a
/// publication question, so its certificate is not reusable here however sound
/// it is for its own.
#[cfg(feature = "fast-contract-validator")]
#[derive(Clone, Copy, Debug)]
struct ClearanceSlabs {
    min: [f64; CLEARANCE_SLAB_DIRECTIONS],
    max: [f64; CLEARANCE_SLAB_DIRECTIONS],
    /// The largest `|projection|` in this set, which feeds the proof margin.
    extent: f64,
}

#[cfg(feature = "fast-contract-validator")]
impl ClearanceSlabs {
    /// The slabs of `set`, or `None` when it carries no points at all.
    ///
    /// `None` is load-bearing and not a convenience: a skip has to prove the
    /// exact loop would have *accepted* the pair, and the exact loop rejects a
    /// pair whose minimum stays at `f64::INFINITY` for want of a single segment
    /// to measure. Refusing to build slabs for a pointless set means a skip can
    /// only ever fire when both sets have at least one point, hence at least one
    /// ring, hence at least one segment pair, hence a finite minimum.
    fn of(set: &MaterialSet) -> Option<Self> {
        let mut slabs: Option<Self> = None;
        for region in &set.regions {
            for ring in region_rings(region) {
                for point in ring {
                    let projections = [point.x, point.y, point.x + point.y, point.x - point.y];
                    match slabs.as_mut() {
                        None => {
                            slabs = Some(Self {
                                min: projections,
                                max: projections,
                                extent: 0.0,
                            })
                        }
                        Some(slabs) => {
                            for index in 0..CLEARANCE_SLAB_DIRECTIONS {
                                slabs.min[index] = slabs.min[index].min(projections[index]);
                                slabs.max[index] = slabs.max[index].max(projections[index]);
                            }
                        }
                    }
                }
            }
        }
        let mut slabs = slabs?;
        for index in 0..CLEARANCE_SLAB_DIRECTIONS {
            if !slabs.min[index].is_finite() || !slabs.max[index].is_finite() {
                return None;
            }
            slabs.extent = slabs
                .extent
                .max(slabs.min[index].abs())
                .max(slabs.max[index].abs());
        }
        Some(slabs)
    }

    /// The gap between the two sets along direction `index`, or a negative
    /// number when their slabs overlap there.
    #[inline]
    fn gap(&self, other: &Self, index: usize) -> f64 {
        (other.min[index] - self.max[index]).max(self.min[index] - other.max[index])
    }
}

/// The all-pairs loop's broad phase: every set's slabs, plus the per-direction
/// gap a skip has to clear.
#[cfg(feature = "fast-contract-validator")]
struct ClearanceBroadPhase {
    slabs: Vec<Option<ClearanceSlabs>>,
    /// `(pair clearance + margin) * |direction normal|`, one per direction, so
    /// the per-pair test is four subtractions and four comparisons with no
    /// multiplication and no division in it.
    thresholds: [f64; CLEARANCE_SLAB_DIRECTIONS],
}

#[cfg(feature = "fast-contract-validator")]
impl ClearanceBroadPhase {
    /// Seals the certificate for one `validate_publication` call.
    ///
    /// # Why the margin makes this a proof
    ///
    /// Skipping a pair claims the exact loop would have found
    /// `minimum_boundary_distance >= pair_clearance`. Three things stand between
    /// the stored projections and that claim, and the margin dominates all of
    /// them by at least three orders of magnitude:
    ///
    /// * **the projections themselves.** `x` and `y` are stored coordinates and
    ///   exact; `x + y` and `x - y` are one rounded operation each, so each
    ///   diagonal projection sits within `2^-53 * (|x| + |y|)` of its real value,
    ///   and a slab can therefore be reported at most that much *wider* apart
    ///   than it is.
    /// * **the gap.** One further subtraction, correctly rounded, so at most
    ///   another `2^-53` relative.
    /// * **the exact loop's own arithmetic.** `segment_distance` is not exact
    ///   either; the value the skip is claiming about is the *computed* one, and
    ///   its coordinate differences and `hypot` carry a handful of ulps of the
    ///   coordinate magnitude.
    ///
    /// Every one of those is bounded by a small multiple of
    /// `2^-53 * extent ~= 1.1e-16 * extent`. The margin is
    /// `1e-9 mm + 1e-12 * extent` - four orders above the worst of them at any
    /// sheet-sized `extent`, and still six orders below the tightest clearance
    /// the engine is asked for, so it costs the filter nothing it could have had.
    ///
    /// The consequence is the property a prefilter needs, and it is the same one
    /// `GridSlabs::separated` states for its own question: this can be wrong
    /// about "they might be close", and cannot be wrong about "they are far".
    fn new(sets: &[MaterialSet], pair_clearance: f64) -> Self {
        let slabs = sets.iter().map(ClearanceSlabs::of).collect::<Vec<_>>();
        let extent = slabs
            .iter()
            .flatten()
            .fold(0.0f64, |extent, slabs| extent.max(slabs.extent));
        let threshold = pair_clearance
            + CLEARANCE_SLAB_ABSOLUTE_MARGIN_MM
            + CLEARANCE_SLAB_RELATIVE_MARGIN * extent;
        let mut thresholds = [f64::INFINITY; CLEARANCE_SLAB_DIRECTIONS];
        if threshold.is_finite() {
            for index in 0..CLEARANCE_SLAB_DIRECTIONS {
                thresholds[index] = threshold * CLEARANCE_SLAB_NORMS[index];
            }
        }
        Self { slabs, thresholds }
    }

    /// Whether the pair is **provably** clear: at least `pair_clearance` apart,
    /// and therefore both non-overlapping and contract-legal.
    ///
    /// `true` is a proof and `false` carries no information, so a `false` here
    /// costs only the four comparisons and hands the pair to the exact loop.
    /// Every comparison is a `>=` against a positive threshold, so a `NaN`
    /// anywhere in the operands answers `false` and cannot produce a skip.
    ///
    /// A `true` clears **both** tests in the scan row, and the second one is the
    /// case worth stating because it is not the distance question. A positive
    /// gap along any direction puts the two sets in disjoint half-planes, so:
    ///
    /// * `rings_properly_cross` is false - the rings are in disjoint strips;
    /// * `has_material_sample_inside` is false in both directions, because an
    ///   interior sample of one set lies inside that set's own polygon, hence
    ///   inside its own slab interval, hence outside the other's.
    ///
    /// That second bullet is the **containment** case, and it is the one a
    /// reader should check: a region sitting strictly inside another's outer
    /// ring has a large positive boundary distance and is nevertheless an
    /// overlap the validator must reject. It can never be skipped here, because
    /// containment makes one slab interval a subset of the other in *every*
    /// direction, so every gap is negative and no proof is available.
    #[inline]
    fn provably_clear(&self, first: usize, second: usize) -> bool {
        let (Some(first), Some(second)) = (&self.slabs[first], &self.slabs[second]) else {
            return false;
        };
        for index in 0..CLEARANCE_SLAB_DIRECTIONS {
            if first.gap(second, index) >= self.thresholds[index] {
                return true;
            }
        }
        false
    }
}

/// Totals for [`contract_validator_census`]: calls, pairs offered, pairs proved
/// clear.
#[cfg(feature = "fast-contract-validator")]
static CENSUS: [std::sync::atomic::AtomicU64; 3] = [
    std::sync::atomic::AtomicU64::new(0),
    std::sync::atomic::AtomicU64::new(0),
    std::sync::atomic::AtomicU64::new(0),
];

#[cfg(feature = "fast-contract-validator")]
fn census_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("POLYGON_NESTING_CONTRACT_VALIDATOR_CENSUS").is_some())
}

/// Counts what the broad phase would reject, for the measurement rounds only.
///
/// Two things about the shape of this are deliberate. It is a **separate**
/// `O(n^2)` pass rather than two counters in the scan row, so a wall
/// measurement never pays for the instrument that describes it and the hot loop
/// is byte-identical whether or not anyone is counting. And it reports through
/// [`contract_validator_census_totals`] to stderr rather than into the result
/// document, because this feature's entire claim is that the document does not
/// change - a counter inside it would be the one field that always did.
#[cfg(feature = "fast-contract-validator")]
fn contract_validator_census(broad_phase: &ClearanceBroadPhase, count: usize) {
    use std::sync::atomic::Ordering;
    if !census_enabled() {
        return;
    }
    let (mut pairs, mut clear) = (0u64, 0u64);
    for first in 0..count {
        for second in (first + 1)..count {
            pairs += 1;
            if broad_phase.provably_clear(first, second) {
                clear += 1;
            }
        }
    }
    CENSUS[0].fetch_add(1, Ordering::Relaxed);
    CENSUS[1].fetch_add(pairs, Ordering::Relaxed);
    CENSUS[2].fetch_add(clear, Ordering::Relaxed);
}

/// `(calls, pairs offered, pairs proved clear)` since process start; all zero
/// unless `POLYGON_NESTING_CONTRACT_VALIDATOR_CENSUS` is set.
#[cfg(feature = "fast-contract-validator")]
pub fn contract_validator_census_totals() -> (u64, u64, u64) {
    use std::sync::atomic::Ordering;
    (
        CENSUS[0].load(Ordering::Relaxed),
        CENSUS[1].load(Ordering::Relaxed),
        CENSUS[2].load(Ordering::Relaxed),
    )
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
    fn raw_source_depth_does_not_snap_to_the_canonical_grid() {
        // A ring whose top edge sits a third of a grid step above 2.000 mm.
        // `PolygonSet::bounds` reads the integer-grid path and rounds it back
        // down to 2.000; the raw-source measurement must keep the excess.
        let piece = PolygonSet::from_outer(vec![
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(1.0, 2.0003),
            point(0.0, 2.0003),
        ])
        .unwrap();
        let placements = [GeneralPlacement {
            piece_id: "sub_grid",
            polygon: &piece,
            rotation_deg: 0.0,
            mirrored: false,
            translate_x: 0.0,
            translate_y: 0.0,
        }];

        let raw = raw_source_long_axis_depth_mm(&placements, 0.5).unwrap();
        assert_eq!(raw, 2.5003);
        assert!(
            raw > piece.bounds().unwrap().max_y + 0.5,
            "the snapped bound must not be able to hide the excess"
        );
    }

    #[test]
    fn raw_source_depth_maximizes_over_every_placement_under_its_own_transform() {
        let piece = square(2.0);
        let placements = [
            GeneralPlacement {
                piece_id: "shallow",
                polygon: &piece,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 0.0,
                translate_y: 0.0,
            },
            GeneralPlacement {
                piece_id: "deep",
                polygon: &piece,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 5.0,
                translate_y: 3.25,
            },
        ];
        assert_eq!(
            raw_source_long_axis_depth_mm(&placements, 1.0).unwrap(),
            2.0 + 3.25 + 1.0
        );

        // A 90-degree rotation of a 1x3 piece is 3 wide and 1 deep, so the
        // measurement has to apply the rotation rather than read a bound.
        let tall = PolygonSet::from_outer(vec![
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(1.0, 3.0),
            point(0.0, 3.0),
        ])
        .unwrap();
        let rotated = [GeneralPlacement {
            piece_id: "rotated",
            polygon: &tall,
            rotation_deg: 90.0,
            mirrored: false,
            translate_x: 0.0,
            translate_y: 0.0,
        }];
        let depth = raw_source_long_axis_depth_mm(&rotated, 0.0).unwrap();
        assert!((depth - 1.0).abs() < 1e-12, "depth was {depth}");
    }

    #[test]
    fn raw_source_depth_rejects_unmeasurable_input() {
        let piece = square(2.0);
        assert!(raw_source_long_axis_depth_mm(&[], 1.0).is_err());
        assert!(raw_source_long_axis_depth_mm(
            &[GeneralPlacement {
                piece_id: "non_finite",
                polygon: &piece,
                rotation_deg: f64::NAN,
                mirrored: false,
                translate_x: 0.0,
                translate_y: 0.0,
            }],
            1.0
        )
        .is_err());
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

    /// The property the broad phase has to have, checked against the exact loop
    /// itself rather than against an expectation: over a sweep that walks a pair
    /// from deeply overlapped to far apart, through the boundary-contact case
    /// and both diagonal separations, a skip must never be claimed for a pair
    /// the exact loop would refuse.
    ///
    /// This is the same discipline the `debug_assert` arm applies inside the
    /// engine, run here as a dense sweep so a `cargo test` build exercises it
    /// without needing a request.
    #[cfg(feature = "fast-contract-validator")]
    #[test]
    fn a_proved_clear_pair_is_one_the_exact_loop_accepts() {
        let piece = square(2.0);
        let mut proofs = 0usize;
        for clearance in [0.0, 0.0005, 0.002, 0.5, 1.0] {
            for step_x in -30..=60 {
                for step_y in -30..=60 {
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
                            rotation_deg: 17.0,
                            mirrored: false,
                            translate_x: f64::from(step_x) * 0.1,
                            translate_y: f64::from(step_y) * 0.1,
                        },
                    ];
                    let transformed = placements
                        .iter()
                        .map(transform_placement)
                        .collect::<Result<Vec<_>, _>>()
                        .expect("both placements are measurable");
                    let broad_phase = ClearanceBroadPhase::new(&transformed, clearance);
                    if !broad_phase.provably_clear(0, 1) {
                        continue;
                    }
                    proofs += 1;
                    // The two things the skip claims, asked of the exact code
                    // the skip replaced.
                    assert!(
                        !material_sets_overlap(&transformed[0], &transformed[1]),
                        "proved clear but overlapping at ({step_x}, {step_y})"
                    );
                    let distance = minimum_boundary_distance(&transformed[0], &transformed[1]);
                    assert!(
                        distance.is_finite() && distance >= clearance,
                        "proved clear at ({step_x}, {step_y}) but the exact minimum is \
                         {distance} against a clearance of {clearance}"
                    );
                }
            }
        }
        assert!(proofs > 0, "the sweep never exercised a proof");
    }

    /// A touching pair is legal and is NOT proved clear: the margin is on the
    /// strict side, so the case the exact loop has to decide reaches it.
    ///
    /// This is the direction that would make the filter unsound if it were ever
    /// reversed, and it is also the one that keeps the filter honest about what
    /// it is: a pair at exactly the clearance is handed over, not skipped.
    #[cfg(feature = "fast-contract-validator")]
    #[test]
    fn a_pair_exactly_at_the_clearance_is_not_skipped() {
        let piece = square(2.0);
        for clearance in [0.0, 0.0005, 0.002, 1.0] {
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
                    translate_x: 2.0 + clearance,
                    translate_y: 0.0,
                },
            ];
            let transformed = placements
                .iter()
                .map(transform_placement)
                .collect::<Result<Vec<_>, _>>()
                .expect("both placements are measurable");
            let broad_phase = ClearanceBroadPhase::new(&transformed, clearance);
            assert!(
                !broad_phase.provably_clear(0, 1),
                "a pair exactly at the clearance {clearance} must reach the exact loop"
            );
        }
    }

    /// Containment is an overlap with a large POSITIVE boundary distance, so it
    /// is the case where "far apart" and "legal" come apart. The broad phase
    /// must not skip it, and it cannot: one slab interval is a subset of the
    /// other in every direction, so no direction offers a gap.
    ///
    /// Without this the filter would be unsound in exactly one way that the
    /// distance sweep above could never catch, because the exact *distance* on
    /// this input is 4 mm and passes any clearance the engine asks for.
    #[cfg(feature = "fast-contract-validator")]
    #[test]
    fn a_contained_piece_is_never_proved_clear() {
        let big = square(10.0);
        let small = square(2.0);
        let placements = [
            GeneralPlacement {
                piece_id: "big",
                polygon: &big,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 0.0,
                translate_y: 0.0,
            },
            GeneralPlacement {
                piece_id: "small",
                polygon: &small,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 4.0,
                translate_y: 4.0,
            },
        ];
        let transformed = placements
            .iter()
            .map(transform_placement)
            .collect::<Result<Vec<_>, _>>()
            .expect("both placements are measurable");
        // The trap: the exact boundary distance is a comfortable 4 mm.
        let distance = minimum_boundary_distance(&transformed[0], &transformed[1]);
        assert!(
            (distance - 4.0).abs() < 1e-12,
            "expected a 4 mm boundary distance, got {distance}"
        );
        assert!(material_sets_overlap(&transformed[0], &transformed[1]));
        for clearance in [0.0, 0.002, 1.0, 3.0] {
            let broad_phase = ClearanceBroadPhase::new(&transformed, clearance);
            assert!(
                !broad_phase.provably_clear(0, 1),
                "a contained piece was proved clear at a clearance of {clearance}"
            );
        }
        // And the verdict itself is still the overlap rejection.
        let error = validate_publication(&placements, settings())
            .expect_err("a contained piece is an overlap");
        assert!(error.message().contains("overlap"), "{}", error.message());
    }

    /// The whole point, stated as a test: flag-on and flag-off decide every
    /// input identically, including the error message.
    ///
    /// The cases are this module's own suite, re-run through both paths - the
    /// broad phase cannot be switched off at runtime, so what this checks is
    /// that the verdict on each is the one the flag-off build's committed
    /// assertions above already pin.
    #[cfg(feature = "fast-contract-validator")]
    #[test]
    fn the_broad_phase_changes_no_verdict() {
        let piece = square(2.0);
        let mut clearance_settings = settings();
        clearance_settings.total_padding_mm = 0.5;
        // Both pieces are held off the sheet edge by `ORIGIN`: with a 0.5 mm
        // pair clearance the sheet-edge clearance is 0.25 mm, and a piece at the
        // origin fails `validate_sheet` before the pair loop is ever reached -
        // which would make every row below a test of the wrong thing.
        const ORIGIN: f64 = 1.0;
        for (rotation, tx, ty, expected_ok) in [
            (0.0, 2.0, 0.0, false),  // touching, but 0.5 mm is required
            (0.0, 1.5, 0.0, false),  // overlapping
            (0.0, 2.5, 0.0, true),   // exactly at the clearance
            (0.0, 9.0, 0.0, true),   // far apart on x
            (0.0, 0.0, 9.0, true),   // far apart on y
            (0.0, 6.0, 6.0, true),   // far apart on the diagonal
            (33.0, 6.0, 6.0, true),  // ditto, rotated
            (33.0, 2.4, 0.0, false), // rotated into the clearance band
        ] {
            let placements = [
                GeneralPlacement {
                    piece_id: "a",
                    polygon: &piece,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: ORIGIN,
                    translate_y: ORIGIN,
                },
                GeneralPlacement {
                    piece_id: "b",
                    polygon: &piece,
                    rotation_deg: rotation,
                    mirrored: false,
                    translate_x: ORIGIN + tx,
                    translate_y: ORIGIN + ty,
                },
            ];
            assert_eq!(
                validate_publication(&placements, clearance_settings).is_ok(),
                expected_ok,
                "verdict changed at rotation {rotation}, ({tx}, {ty})"
            );
        }
    }
}
