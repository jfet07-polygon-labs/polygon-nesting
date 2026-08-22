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
    validate_publication_inner(placements, settings, false, true)
}

/// The process-wide arming of the clearance certificate.
///
/// `true`, so a build that compiles `fast-contract-validator` runs the filter
/// unless something takes it off - which is what the feature has always done
/// and is why this constant is the default rather than a promotion.
///
/// It exists because promotion needed a **disarm**, not an arm.
/// docs/experiments/fast-contract-validator/ §13.2 makes that the first of the
/// four conditions on default-on: *"promotion to default-on means the exact
/// loop becomes unreachable in a shipping build, and a way to disarm it in the
/// field is worth more than its absence"*. Before this switch the only way to
/// reach the exact loop in a release binary was to call
/// [`validate_publication_exact_reference`] directly, which no production route
/// does and no spec key could reach.
///
/// A process-wide `AtomicBool` rather than a parameter on
/// [`validate_publication`] because the certificate's callers are the whole
/// acceptance path - `general_fast`, `general_relaxed`,
/// `general_persistent_vacancy`, `general_micro_legalization` - and threading a
/// tuning flag through a *contract* settings struct would put an engine
/// preference inside the type that means "what the request asked for". The
/// coordinator sets it once, before any search thread exists, and puts it back
/// on the way out; see `ContractCertificateArming` in `search::portfolio`.
#[cfg(feature = "fast-contract-validator")]
static CERTIFICATE_ARMED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Whether the clearance certificate is armed in this process.
#[cfg(feature = "fast-contract-validator")]
pub fn contract_certificate_armed() -> bool {
    CERTIFICATE_ARMED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Arms or disarms the clearance certificate process-wide, returning what it
/// was.
///
/// Disarmed, every pair goes to the exact loop and the result is the one a
/// build without the feature produces - that is the equivalence
/// `examples/contract_validator_shadow.rs` measures, now reachable from a spec
/// key instead of only from a second binary.
///
/// `Relaxed` on both sides is sufficient and deliberate: the value is read once
/// per `validate_publication` call to seal a broad phase, never to publish or
/// acquire anything else, and the coordinator writes it before it spawns any
/// search work and after it joins all of it.
#[cfg(feature = "fast-contract-validator")]
pub fn set_contract_certificate_armed(armed: bool) -> bool {
    CERTIFICATE_ARMED.swap(armed, std::sync::atomic::Ordering::Relaxed)
}

/// [`validate_publication`] with the broad phase **disarmed**: the exact loop
/// runs on every pair, as it does in a build without this feature.
///
/// This exists so that one release binary can hold both implementations at
/// once. The equivalence evidence the previous round could offer was a test
/// comparing the feature-on path against *enumerated expectations*, which is not
/// two implementations meeting; and the 5.9M-pair census ran in release, where
/// the `debug_assert` on the skip is compiled out. `examples/
/// contract_validator_shadow.rs` closes that by running a randomized corpus
/// through this function and [`validate_publication`] in the same release
/// process and requiring the two `Result`s to be equal **including the error
/// message**.
///
/// Disarming costs the armed path nothing: a disarmed phase is one whose slabs
/// are all `None` and whose thresholds are all infinite, so `provably_clear` is
/// constantly false without the scan row acquiring a branch it did not have.
#[cfg(feature = "fast-contract-validator")]
pub fn validate_publication_exact_reference(
    placements: &[GeneralPlacement<'_>],
    settings: PublicationValidationSettings,
) -> Result<(), PublicationValidationError> {
    validate_publication_inner(placements, settings, false, false)
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
    validate_publication_inner(placements, settings, true, true)
}

fn validate_publication_inner(
    placements: &[GeneralPlacement<'_>],
    settings: PublicationValidationSettings,
    #[cfg_attr(
        not(feature = "parallel-compression-schedule"),
        allow(unused_variables)
    )]
    parallel: bool,
    #[cfg_attr(not(feature = "fast-contract-validator"), allow(unused_variables))]
    use_broad_phase: bool,
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
    //
    // `use_broad_phase` is the *call site's* choice - it is `false` for
    // [`validate_publication_exact_reference`], which must stay exact whatever
    // any switch says - and [`contract_certificate_armed`] is the process's.
    // Read once here, into the seal, so the scan row below is byte-identical
    // either way: a disarmed phase answers `provably_clear` `false` constantly
    // without the loop acquiring a branch it did not have (§8.1).
    #[cfg(feature = "fast-contract-validator")]
    let armed = use_broad_phase && contract_certificate_armed();
    #[cfg(feature = "fast-contract-validator")]
    let broad_phase = if armed {
        ClearanceBroadPhase::new(&transformed, pair_clearance)
    } else {
        ClearanceBroadPhase::disarmed(transformed.len())
    };
    #[cfg(feature = "fast-contract-validator")]
    if armed {
        contract_validator_census(&broad_phase, transformed.len());
    }
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

/// An **upper** bound on the length of each projection direction's normal, in
/// the order the projections are stored: `(1,0)`, `(0,1)`, `(1,1)`, `(1,-1)`.
///
/// A gap measured along an *unnormalised* direction `d` is `|d|` times the gap
/// along its unit normal, so a distance threshold has to be scaled by `|d|`
/// before it is compared against the diagonal projections. The bound has to be
/// an upper one in *both* uses: the threshold is what gets scaled, and a
/// threshold scaled by anything `>= |d|` is a threshold at least as strict as
/// the true one.
///
/// `SQRT_2` is the correctly rounded `sqrt(2)` and already rounds *up*
/// (`1.4142135623730951` against `1.41421356237309504880...`), but the proof
/// must not rest on which way a library constant happened to land, so this
/// takes one further ulp outward and does not care.
#[cfg(feature = "fast-contract-validator")]
const CLEARANCE_SLAB_NORM_UPPER_BOUNDS: [f64; CLEARANCE_SLAB_DIRECTIONS] = [
    1.0,
    1.0,
    f64::from_bits(std::f64::consts::SQRT_2.to_bits() + 1),
    f64::from_bits(std::f64::consts::SQRT_2.to_bits() + 1),
];

/// The floor of the proof margin, in millimetres.
///
/// This is *not* what carries the proof - [`CLEARANCE_SLAB_RELATIVE_MARGIN`] is
/// - and it is kept only because it costs nothing and holds the shipped
/// threshold where the measured rounds found it. It is a picometre: six orders
/// of magnitude below the smallest pair clearance this engine is ever asked for
/// (`0.0005 mm`).
#[cfg(feature = "fast-contract-validator")]
const CLEARANCE_SLAB_ABSOLUTE_MARGIN_MM: f64 = 1e-9;

/// The scale-following part of the proof margin, as a fraction of the largest
/// projection magnitude in the layout.
///
/// This is the number the whole certificate rests on, and it is held to
/// [`CLEARANCE_SLAB_PROVEN_RELATIVE_ERROR`] by
/// [`the_shipped_margin_dominates_the_proven_error_bound`]: it may be *raised*
/// freely and may never be lowered below the derived bound. It sits about 280x
/// above that bound, which is why the value the measured rounds ran with is
/// still the value here - nothing in this change moves a threshold.
#[cfg(feature = "fast-contract-validator")]
const CLEARANCE_SLAB_RELATIVE_MARGIN: f64 = 1e-12;

/// The **derived** relative error bound the proof needs the margin to dominate:
/// `CLEARANCE_SLAB_PROVEN_ERROR_ULPS * 2^-53`.
///
/// See [`ClearanceBroadPhase::new`] for the derivation this discharges. Both
/// halves of a skip depend on it - the distance half needs `16.5 * u * extent`
/// and the overlap half's rounded edge midpoints need `1.5 * u * extent`.
#[cfg(feature = "fast-contract-validator")]
const CLEARANCE_SLAB_PROVEN_ERROR_ULPS: f64 = 32.0;

/// `CLEARANCE_SLAB_PROVEN_ERROR_ULPS` unit roundoffs, relative.
#[cfg(feature = "fast-contract-validator")]
const CLEARANCE_SLAB_PROVEN_RELATIVE_ERROR: f64 =
    CLEARANCE_SLAB_PROVEN_ERROR_ULPS * (f64::EPSILON / 2.0);

/// The contractual grid's own coordinate ceiling, in millimetres.
///
/// [`PolygonRing::new`] admits a source coordinate only when `to_grid_mm` can
/// represent it, and that requires `|x| * 1000` to be an IEEE-754 *safe
/// integer*. Every **source** coordinate this validator can ever see therefore
/// satisfies `|x| <= (2^53 - 1) / 1000`. It is quoted here because it is the
/// only bound the type system gives, and because
/// [`CLEARANCE_SLAB_MAX_COORDINATE_MM`] exists precisely to say that it is not
/// enough on its own.
///
/// A proof input rather than a runtime value: nothing in the filter reads it,
/// and the test that recomputes the structural ceiling from it is what keeps it
/// honest, so it is scoped to the tests that consume it.
#[cfg(all(test, feature = "fast-contract-validator"))]
const CLEARANCE_SLAB_GRID_CEILING_MM: f64 = 9_007_199_254_740_991.0 / 1000.0;

/// The numeric domain the certificate is proved on: no skip is ever issued for
/// a pair carrying a coordinate of larger magnitude than this. `2^112 mm`.
///
/// # The step this exists to close
///
/// The scan row treats a non-finite minimum as a **rejection**, and the filter's
/// original argument for never skipping past one was "`minimum` starts at
/// `INFINITY` and `f64::min` ignores `NaN`, so the only way out is that no
/// segment pair existed". That does not follow on its own:
/// [`point_segment_distance`] contains squares, products and a division that can
/// each manufacture an `inf` or a `NaN` out of finite inputs, and `f64::min`
/// returning the non-`NaN` operand is exactly what would leave `INFINITY`
/// standing on a pair whose slabs are far apart. The implication needs a bound
/// on the coordinates, and the bound needs to survive the transform.
///
/// # Where the bound actually comes from, which is not where one would look
///
/// The grid contract ([`CLEARANCE_SLAB_GRID_CEILING_MM`]) bounds the *source*
/// ring, but it does not survive the transform: `translate_x` / `translate_y`
/// are checked only for finiteness ([`placement_rotation`]), and
/// `transform_source_ring` likewise rejects only a non-finite result. Nor can
/// `validate_sheet` be leaned on - it runs against `sheet_width_mm`, itself only
/// required to be finite and positive, and it runs *after* the transform and
/// only over outer rings.
///
/// The bound that does hold comes from [`interior_sample`], via
/// [`transform_placement`], which rejects any region with no discoverable
/// material interior. Discovering one requires two **distinct** `f64` y-levels
/// among the transformed ring points, and two distinct x-intersections at some
/// scan level. Two distinct doubles of magnitude `M` differ by at least
/// `M * 2^-53`, and both differences are bounded by the region's diameter, which
/// a rigid transform inherits from the source ring: at most
/// `2 * sqrt(2) * (2^53 - 1) / 1000 ~= 2.55e13 mm`. So every coordinate of a set
/// that `transform_placement` admits satisfies
///
/// ```text
/// |coordinate| <= 2.55e13 * 2^53 ~= 2.29e29 mm
/// ```
///
/// and at that magnitude nothing in the exact loop overflows. **So the original
/// lemma is true** - but for a reason that lives three functions away, in the
/// one function whose failure mode is "this piece has no interior", and it had
/// not been written down anywhere. A validator's soundness should not rest on an
/// unstated consequence of a helper that exists for another purpose.
///
/// # What the value is chosen to make true
///
/// `2^112 ~= 5.19e33` is picked to sit above that structural bound by four
/// orders of magnitude - so it can never refuse a layout the contract actually
/// admits, and the shipped skip rate is untouched - while still proving
/// finiteness *on its own*, without borrowing the argument above. Writing `C`
/// for the largest coordinate magnitude in a pair, `C <= 2^112` gives:
///
/// * `dx = end.x - start.x` has `|dx| <= 2^113`, so `dx * dx <= 2^226` and
///   `length_squared <= 2^227` - 797 binades below the overflow horizon, so no
///   product, sum or square in [`point_segment_distance`] reaches infinity;
/// * the projection numerator is bounded by the same `2^227`, so the division is
///   never `inf / inf` and never yields `NaN`; `clamp` then puts the parameter in
///   `[0, 1]` even where a denormal `length_squared` sends the quotient to
///   infinity;
/// * `hypot`'s arguments are bounded by `2^113`, so its result is finite;
/// * `orient2d`'s adaptive expansions multiply coordinate differences by the
///   splitter `2^27 + 1`, giving intermediates bounded by `2^254` - so the
///   `robust` predicate keeps its **exact sign**, which is what makes
///   `segments_touch_or_cross`, `rings_properly_cross` and
///   `classify_point_in_ring` exact rather than approximate, and that exactness
///   is load-bearing for the overlap half of every skip;
/// * `(a + b) / 2.0` in [`has_material_sample_inside`] and the `x + y` / `x - y`
///   projections are bounded by `2^113`.
///
/// No overflow anywhere means no `inf`, hence no `inf - inf`, `0 * inf` or
/// `inf / inf`, hence **no `NaN`**. So on the guarded domain
/// `point_segment_distance` returns a finite non-negative number on every input,
/// `minimum_boundary_distance` is non-finite *only* when it saw no segment pair
/// at all, and that case is exactly what [`ClearanceSlabs::of`]'s empty-set
/// `None` forecloses. The horizon for this argument is `C <= 2^497`; `2^112`
/// leaves 385 binades of room.
///
/// Outside the domain none of that is available, and the guard is why the
/// question never has to be asked there: the pair takes the exact loop, which is
/// the fail-closed direction.
#[cfg(feature = "fast-contract-validator")]
const CLEARANCE_SLAB_MAX_COORDINATE_MM: f64 = 5_192_296_858_534_827_628_530_496_329_220_096.0;

/// The structural coordinate bound derived in
/// [`CLEARANCE_SLAB_MAX_COORDINATE_MM`]: what `interior_sample` and the grid
/// contract already force, in millimetres.
///
/// Kept as a constant so `the_domain_guard_admits_everything_the_contract_can_build`
/// can assert the guard sits above it *and* that the number still matches its
/// own derivation. If a future change to `interior_sample` or to the grid
/// contract raises this above the guard, that test fails rather than the
/// validator silently starting to refuse certificates.
#[cfg(all(test, feature = "fast-contract-validator"))]
const CLEARANCE_SLAB_STRUCTURAL_CEILING_MM: f64 = 2.2946926991272400e29;

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
    /// A rounded-**down** lower bound on the true minimum of each projection.
    min: [f64; CLEARANCE_SLAB_DIRECTIONS],
    /// A rounded-**up** upper bound on the true maximum of each projection.
    max: [f64; CLEARANCE_SLAB_DIRECTIONS],
    /// The largest `|projection|` in this set, which feeds the proof margin.
    extent: f64,
}

#[cfg(feature = "fast-contract-validator")]
impl ClearanceSlabs {
    /// The slabs of `set`, or `None` when it carries no points at all, when any
    /// coordinate leaves [`CLEARANCE_SLAB_MAX_COORDINATE_MM`], or when any
    /// projection is not finite.
    ///
    /// # Two different `None`s, both load-bearing
    ///
    /// **No points.** A skip has to prove the exact loop would have *accepted*
    /// the pair, and the exact loop rejects a pair whose minimum stays at
    /// `f64::INFINITY` for want of a single segment to measure. Refusing to
    /// build slabs for a pointless set means a skip can only ever fire when both
    /// sets have at least one point, hence at least one ring, hence at least one
    /// segment pair.
    ///
    /// **Outside the numeric domain.** That "hence a finite minimum" is a step
    /// about floating-point arithmetic, not about geometry, and it is only true
    /// where the arithmetic cannot overflow: squares, products and a division in
    /// [`point_segment_distance`] can each manufacture an `inf` or a `NaN` out of
    /// finite inputs, `f64::min` propagates the *non*-`NaN` operand and so
    /// leaves `INFINITY` standing, and the scan row treats a non-finite minimum
    /// as a **rejection**. A filter that skipped such a pair would invert a
    /// verdict. [`CLEARANCE_SLAB_MAX_COORDINATE_MM`] is the domain on which that
    /// cannot happen, and this is where the guard is applied: any set with a
    /// coordinate outside it gets no certificate at all, and every pair
    /// containing it takes the exact loop.
    ///
    /// The interior sample is checked with the ring points even though it is
    /// never projected, because [`has_material_sample_inside`] feeds it to
    /// `orient2d` and the domain is what keeps that predicate exact.
    ///
    /// # Outward rounding
    ///
    /// `x` and `y` are stored coordinates and exact. `x + y` and `x - y` are one
    /// correctly rounded operation each, so the true projection lies within half
    /// an ulp of the computed one and therefore inside
    /// `[next_down(p), next_up(p)]`. Widening each diagonal projection outward
    /// by that ulp before the running `min`/`max` makes `[min, max]` a
    /// guaranteed **superset** of the true projection interval, which is what
    /// lets [`Self::gap`] carry a genuine bound rather than an estimate
    /// with an epsilon bolted on.
    fn of(set: &MaterialSet) -> Option<Self> {
        let mut slabs: Option<Self> = None;
        let mut admit = |x: f64, y: f64| -> bool {
            if !(x.abs() <= CLEARANCE_SLAB_MAX_COORDINATE_MM)
                || !(y.abs() <= CLEARANCE_SLAB_MAX_COORDINATE_MM)
            {
                // Written as a negated `<=` so that a `NaN` coordinate, on which
                // every comparison is false, takes this branch too.
                return false;
            }
            let sum = x + y;
            let difference = x - y;
            let lower = [x, y, sum.next_down(), difference.next_down()];
            let upper = [x, y, sum.next_up(), difference.next_up()];
            match slabs.as_mut() {
                None => {
                    slabs = Some(Self {
                        min: lower,
                        max: upper,
                        extent: 0.0,
                    })
                }
                Some(slabs) => {
                    for index in 0..CLEARANCE_SLAB_DIRECTIONS {
                        slabs.min[index] = slabs.min[index].min(lower[index]);
                        slabs.max[index] = slabs.max[index].max(upper[index]);
                    }
                }
            }
            true
        };
        for region in &set.regions {
            for ring in region_rings(region) {
                for point in ring {
                    if !admit(point.x, point.y) {
                        return None;
                    }
                }
            }
            if let Some(sample) = region.material_sample {
                if !(sample.x.abs() <= CLEARANCE_SLAB_MAX_COORDINATE_MM)
                    || !(sample.y.abs() <= CLEARANCE_SLAB_MAX_COORDINATE_MM)
                {
                    return None;
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

    /// The computed gap between the two sets along direction `index`, negative
    /// when their slabs overlap there.
    ///
    /// This is **not** itself a lower bound on the true gap - it is one rounded
    /// subtraction away from one - and it is deliberately left that way. The
    /// missing ulp is absorbed once, at build time, by the second `next_up` on
    /// the threshold in [`ClearanceBroadPhase::new`], because `next_down` is
    /// monotonic and `next_down(g) >= t` is exactly `g >= next_up(t)`. Moving
    /// the rounding to the threshold makes it cost `O(directions)` per
    /// `validate_publication` call instead of `O(pairs * directions)`, and
    /// leaves this function the two subtractions and one `max` the measured
    /// rounds timed - the 5.57x per-confirmation result is a result about
    /// *this* instruction sequence, and a version of the proof that changed it
    /// would have needed its own wall battery to keep that number.
    ///
    /// `self.min`/`self.max` already bracket the true projection interval
    /// outward, so each difference below is no larger than the corresponding
    /// true difference. `max` of two floats is exact.
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
    /// # What a skip has to be, and the two halves it is built from
    ///
    /// Skipping a pair claims two things: that `material_sets_overlap` is false,
    /// and that `minimum_boundary_distance` is finite and `>= pair_clearance`.
    /// The second is a claim about the **computed** value, not the real
    /// distance, so the proof has to cross the exact loop's own arithmetic as
    /// well as its own. It is built from two pieces that are kept deliberately
    /// separate:
    ///
    /// 1. a *certified* lower bound on the true geometric gap, carrying no
    ///    epsilon at all - [`ClearanceSlabs::of`] rounds the stored intervals
    ///    outward and the threshold absorbs the subtraction's own rounding, so
    ///    `next_down(gap) <= true gap` is unconditional; and
    /// 2. a *derived* bound on how far below the truth the exact loop's own
    ///    floating-point answer can land, which is what the margin is for.
    ///
    /// The previous form of this comment claimed "a handful of ulps" for the
    /// second piece, which is an assertion and not a bound. Here is the bound.
    ///
    /// # The derivation the margin discharges
    ///
    /// Write `u = 2^-53` and let `C` be the largest coordinate magnitude in the
    /// pair; `extent >= C` always, because the stored projections include `x`
    /// and `y` themselves. Every step below is a correctly rounded IEEE-754
    /// operation, and [`CLEARANCE_SLAB_MAX_COORDINATE_MM`] has already ruled out
    /// overflow, so each carries a relative error of at most `u`.
    ///
    /// **Distance half.** In [`point_segment_distance`] the clamped parameter
    /// `p` lies in `[0, 1]`, so `Q = S + p * (E - S)` - computed exactly - is a
    /// real point *on* the segment and `true_distance <= |P - Q|`. Tracking the
    /// roundings: `fl(p * dx)` is within `2.1 * C * u` of `p * (E.x - S.x)`;
    /// `closest_x` adds one more rounding for `5.3 * C * u`; the difference
    /// `P.x - closest_x` adds another for `7.4 * C * u` per component, so the
    /// computed difference vector is within `10.5 * C * u` of `P - Q`. `hypot`
    /// contributes at most `2u` relative on a result bounded by `3C`, i.e.
    /// `6 * C * u`. Hence
    /// **`computed >= true_distance - 16.5 * C * u`**. The degenerate branch
    /// (`length_squared == 0`) measures to an endpoint and is looser only in the
    /// safe direction, at `9 * C * u`.
    ///
    /// **Overlap half.** A positive true gap puts the two sets in disjoint
    /// half-planes. `rings_properly_cross` and `classify_point_in_ring` are then
    /// *exact* - they consume only signs of `robust`'s adaptive `orient2d`,
    /// which is exact on the guarded domain - so the only inexact input to
    /// `material_sets_overlap` is the rounded edge midpoint in
    /// [`has_material_sample_inside`], which sits within `1.5 * C * u` of a true
    /// point of its own set. (The interior sample needs no slack: it is a stored
    /// point that the exact winding rule certified as inside its own polygon,
    /// hence inside its own hull.)
    ///
    /// So `32 * u` dominates both halves, with the distance half binding.
    /// [`CLEARANCE_SLAB_RELATIVE_MARGIN`] is `1e-12`, about `280x` that, and
    /// [`the_shipped_margin_dominates_the_proven_error_bound`] is the test that
    /// fails if anyone ever lowers it under the derivation.
    ///
    /// # Putting them together
    ///
    /// The threshold is rounded **up** at every step, so
    /// `gap >= threshold[i]` implies
    /// `next_down(gap) >= (pair_clearance + margin) * |d_i|` in exact
    /// arithmetic. Since `|b - a| * |d| >= (b - a) . d >= gap` for any `a`, `b`
    /// in the two sets, that gives
    /// `true_distance >= pair_clearance + margin`, and the distance half then
    /// gives `computed >= pair_clearance`. That is the claim, and it is now an
    /// implication rather than a comfortable ratio.
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
        // Every rounding here is outward, so `threshold` is an upper bound on
        // the real `(pair_clearance + margin) * |d_i|` and a skip therefore
        // clears the real quantity too.
        //
        // The `max` is the proof made structural rather than merely tested:
        // whatever `CLEARANCE_SLAB_RELATIVE_MARGIN` is edited to, the margin
        // cannot fall below the derived error bound, so soundness does not
        // depend on anyone reading the derivation before touching the constant.
        // At the shipped `1e-12` against a derived `3.55e-15` the `max` selects
        // the shipped value and this changes no threshold.
        let relative = CLEARANCE_SLAB_RELATIVE_MARGIN.max(CLEARANCE_SLAB_PROVEN_RELATIVE_ERROR);
        let margin = (CLEARANCE_SLAB_ABSOLUTE_MARGIN_MM + (relative * extent).next_up()).next_up();
        let threshold = (pair_clearance + margin).next_up();
        let mut thresholds = [f64::INFINITY; CLEARANCE_SLAB_DIRECTIONS];
        if threshold.is_finite() {
            for index in 0..CLEARANCE_SLAB_DIRECTIONS {
                // Two outward steps, absorbing two different roundings. The
                // first is this multiplication's own. The second is the
                // *subtraction* inside `ClearanceSlabs::gap`, paid here rather
                // than per pair: `next_down(g) >= t` iff `g >= next_up(t)`, so
                // bumping the threshold once is worth a `next_down` on every
                // gap of every pair, and leaves the hot loop untouched.
                thresholds[index] = (threshold * CLEARANCE_SLAB_NORM_UPPER_BOUNDS[index])
                    .next_up()
                    .next_up();
            }
        }
        Self { slabs, thresholds }
    }

    /// A phase that certifies nothing, for
    /// [`validate_publication_exact_reference`].
    ///
    /// Both fields are independently sufficient: every slab is `None`, so the
    /// `let ... else` in [`Self::provably_clear`] returns on the first line, and
    /// every threshold is infinite, so even a hypothetical `Some` could not
    /// clear it - `gap` is always finite on a set that produced a
    /// certificate, because [`CLEARANCE_SLAB_MAX_COORDINATE_MM`] bounds the
    /// projections and `next_down` of a finite difference is finite.
    fn disarmed(count: usize) -> Self {
        Self {
            slabs: vec![None; count],
            thresholds: [f64::INFINITY; CLEARANCE_SLAB_DIRECTIONS],
        }
    }

    /// How many sets were refused a certificate outright - no points, a
    /// non-finite projection, or a coordinate outside
    /// [`CLEARANCE_SLAB_MAX_COORDINATE_MM`]. The fail-closed counter.
    fn domain_refusals(&self) -> u64 {
        self.slabs.iter().filter(|slabs| slabs.is_none()).count() as u64
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
    ///
    /// The `None` arm is the fail-closed one, and it carries the numeric-domain
    /// guard as well as the empty-set case: a pair either side of
    /// [`CLEARANCE_SLAB_MAX_COORDINATE_MM`] gets no certificate and goes to the
    /// exact loop unchanged.
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

/// What [`contract_validator_shadow_audit`] found on one layout.
#[cfg(feature = "fast-contract-validator")]
#[derive(Clone, Debug, PartialEq)]
pub struct ContractValidatorShadowAudit {
    /// Pairs offered to the broad phase.
    pub pairs: u64,
    /// Pairs it issued a certificate for. A corpus with this at zero has tested
    /// nothing, which is why the harness reports it.
    pub proved_clear: u64,
    /// Material sets refused a certificate outright - no points, a non-finite
    /// projection, or a coordinate outside
    /// [`CLEARANCE_SLAB_MAX_COORDINATE_MM`]. This is the fail-closed counter.
    pub domain_refusals: u64,
    /// Whether the layout never reached a pair, because the settings or a
    /// transform rejected it first.
    pub preamble_rejected: bool,
    /// The smallest `exact distance - pair_clearance` over the pairs this
    /// layout certified, or `f64::INFINITY` when it certified none.
    ///
    /// This is how far the corpus actually got from the boundary. A shadow run
    /// whose tightest certificate sits millimetres clear of the clearance has
    /// not probed the margin at all, however many pairs it counted, so this is
    /// reported beside the zero rather than left for the reader to assume.
    pub tightest_certified_excess: f64,
    /// One entry per skip that the exact tests then contradicted. **This is the
    /// finding**: it must be empty.
    pub mismatches: Vec<String>,
}

#[cfg(feature = "fast-contract-validator")]
impl Default for ContractValidatorShadowAudit {
    fn default() -> Self {
        Self {
            pairs: 0,
            proved_clear: 0,
            domain_refusals: 0,
            preamble_rejected: false,
            tightest_certified_excess: f64::INFINITY,
            mismatches: Vec::new(),
        }
    }
}

/// Re-runs, for real and in whatever build profile the caller is in, both of the
/// tests every certificate claims - and reports every disagreement.
///
/// The scan row's `debug_assert` does this too, but it is compiled out of a
/// release build, so the 5.9M-pair census the previous round quotes was taken
/// with no checking of any kind behind it. This is the release-visible form:
/// production never calls it, it costs the hot path nothing, and
/// `examples/contract_validator_shadow.rs` drives it over a randomized corpus.
///
/// It deliberately audits pairs the production loop would not reach - it does
/// not run `validate_sheet` first, and it does not stop at the first failing
/// pair - because the filter's claim is about geometry, not about how far the
/// scan got.
#[cfg(feature = "fast-contract-validator")]
pub fn contract_validator_shadow_audit(
    placements: &[GeneralPlacement<'_>],
    settings: PublicationValidationSettings,
) -> ContractValidatorShadowAudit {
    let mut audit = ContractValidatorShadowAudit::default();
    if validate_settings(settings).is_err() {
        audit.preamble_rejected = true;
        return audit;
    }
    let Ok(transformed) = placements
        .iter()
        .map(transform_placement)
        .collect::<Result<Vec<_>, _>>()
    else {
        audit.preamble_rejected = true;
        return audit;
    };
    let pair_clearance = settings.total_padding_mm + 2.0 * settings.flattening_sag_tolerance_mm;
    let broad_phase = ClearanceBroadPhase::new(&transformed, pair_clearance);
    audit.domain_refusals = broad_phase.domain_refusals();
    for first_index in 0..transformed.len() {
        for second_index in (first_index + 1)..transformed.len() {
            audit.pairs += 1;
            if !broad_phase.provably_clear(first_index, second_index) {
                continue;
            }
            audit.proved_clear += 1;
            let first = &transformed[first_index];
            let second = &transformed[second_index];
            if material_sets_overlap(first, second) {
                audit.mismatches.push(format!(
                    "skipped an overlapping pair: {} and {}",
                    placements[first_index].piece_id, placements[second_index].piece_id
                ));
            }
            let distance = minimum_boundary_distance(first, second);
            if distance.is_finite() {
                audit.tightest_certified_excess = audit
                    .tightest_certified_excess
                    .min(distance - pair_clearance);
            }
            if !(distance.is_finite() && distance >= pair_clearance) {
                audit.mismatches.push(format!(
                    "skipped a pair the exact loop refuses: {} and {} at {} against a clearance of {}",
                    placements[first_index].piece_id,
                    placements[second_index].piece_id,
                    distance,
                    pair_clearance
                ));
            }
        }
    }
    audit
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

    /// The margin is only a proof if it dominates the derived error bound, so
    /// the derivation is a test rather than a paragraph.
    ///
    /// [`ClearanceBroadPhase::new`] derives `16.5 * u * extent` for the distance
    /// half and `1.5 * u * extent` for the overlap half's rounded midpoints.
    /// `CLEARANCE_SLAB_PROVEN_ERROR_ULPS` is the `32` that dominates both, and
    /// the shipped `CLEARANCE_SLAB_RELATIVE_MARGIN` must not fall under it. It
    /// may be raised freely; this fails if it is ever lowered past the proof.
    #[cfg(feature = "fast-contract-validator")]
    #[test]
    fn the_shipped_margin_dominates_the_proven_error_bound() {
        assert!(
            CLEARANCE_SLAB_PROVEN_ERROR_ULPS >= 16.5,
            "the derivation needs at least 16.5 ulps for the distance half"
        );
        assert!(
            CLEARANCE_SLAB_RELATIVE_MARGIN >= CLEARANCE_SLAB_PROVEN_RELATIVE_ERROR,
            "the shipped margin {CLEARANCE_SLAB_RELATIVE_MARGIN:e} is below the proven \
             error bound {CLEARANCE_SLAB_PROVEN_RELATIVE_ERROR:e}"
        );
        // And the margin is still far below the tightest clearance the engine is
        // ever asked for, at the coordinate scale a sheet actually has, so
        // dominating the error costs the filter nothing.
        assert!(CLEARANCE_SLAB_RELATIVE_MARGIN * 3000.0 < 0.0005 / 1000.0);
    }

    /// The guard must sit above everything the contract can actually build, or
    /// it would be refusing certificates rather than bounding them.
    #[cfg(feature = "fast-contract-validator")]
    #[test]
    fn the_domain_guard_admits_everything_the_contract_can_build() {
        // The structural ceiling, recomputed here from its two inputs rather
        // than copied, so the constant cannot drift away from its derivation.
        let diameter = 2.0 * std::f64::consts::SQRT_2 * CLEARANCE_SLAB_GRID_CEILING_MM;
        let structural = diameter * (2.0f64).powi(53);
        assert!(
            (structural / CLEARANCE_SLAB_STRUCTURAL_CEILING_MM - 1.0).abs() < 1e-9,
            "the structural ceiling constant {CLEARANCE_SLAB_STRUCTURAL_CEILING_MM:e} no \
             longer matches its derivation {structural:e}"
        );
        assert!(
            CLEARANCE_SLAB_MAX_COORDINATE_MM > structural,
            "the domain guard {CLEARANCE_SLAB_MAX_COORDINATE_MM:e} is below the structural \
             ceiling {structural:e}, so it can refuse contractual layouts"
        );
        // And it is far below the horizon where `orient2d`'s splitter overflows,
        // which is what the guard is for in the other direction.
        assert!(CLEARANCE_SLAB_MAX_COORDINATE_MM < (2.0f64).powi(497));
    }

    /// The lemma the guard replaces is **false** as stated, and this is the
    /// witness: a pair of material sets whose exact minimum is not finite - so
    /// the scan row rejects them - and whose slab gap, computed the way the
    /// unguarded certificate computed it, is `+inf` and clears every threshold.
    ///
    /// The old certificate would have skipped a rejection. It never could in
    /// production, because `transform_placement` cannot build these sets (see
    /// `CLEARANCE_SLAB_MAX_COORDINATE_MM`: `interior_sample` bounds coordinates
    /// at `2.29e29` long before this), which is why this constructs them
    /// directly. The point is that the filter's soundness was resting on that
    /// unstated bound, and now it rests on a check.
    #[cfg(feature = "fast-contract-validator")]
    #[test]
    fn the_numeric_domain_guard_fails_closed_where_the_lemma_does_not_hold() {
        let far = 1.3e308;
        let strip = |x: f64| MaterialRegion {
            outer: vec![point(x, 0.0), point(x, 2.0), point(x, 4.0)],
            holes: Vec::new(),
            material_sample: Some(point(x, 2.0)),
        };
        let left = MaterialSet {
            regions: vec![strip(-far)],
        };
        let right = MaterialSet {
            regions: vec![strip(far)],
        };

        // 1. The exact loop's verdict on this pair is a REJECTION: its minimum
        //    is not finite, which `scan_row` turns into a clearance violation.
        let distance = minimum_boundary_distance(&left, &right);
        assert!(
            !distance.is_finite(),
            "expected a non-finite exact minimum, got {distance}"
        );

        // 2. The unguarded certificate would nevertheless have proved it clear:
        //    the x-projections are finite on both sides, and their difference
        //    overflows to +inf, which clears any finite threshold.
        let raw_gap = far - -far;
        assert!(
            raw_gap.is_infinite() && raw_gap > 0.0,
            "expected the unguarded gap to overflow, got {raw_gap}"
        );
        assert!(
            raw_gap >= 5.0,
            "an infinite gap clears every real clearance"
        );

        // 3. The guard refuses both sets outright, so no certificate exists and
        //    the pair takes the exact loop - which rejects it.
        assert!(ClearanceSlabs::of(&left).is_none());
        assert!(ClearanceSlabs::of(&right).is_none());
        let phase = ClearanceBroadPhase::new(&[left, right], 5.0);
        assert_eq!(phase.domain_refusals(), 2);
        assert!(!phase.provably_clear(0, 1));
    }

    /// Outward rounding is only worth having if it actually brackets: the stored
    /// interval must contain the true projection of every point, and the gap
    /// must never be reported larger than it is.
    #[cfg(feature = "fast-contract-validator")]
    #[test]
    fn the_slab_interval_brackets_the_true_projection() {
        // Coordinates chosen so `x + y` and `x - y` are both inexact.
        let awkward = MaterialSet {
            regions: vec![MaterialRegion {
                outer: vec![
                    point(0.1, 0.2),
                    point(1.0 / 3.0, 7.0 / 9.0),
                    point(1e15 + 0.1, 1e-15),
                ],
                holes: Vec::new(),
                material_sample: Some(point(0.2, 0.3)),
            }],
        };
        let slabs = ClearanceSlabs::of(&awkward).expect("in domain");
        for p in &awkward.regions[0].outer {
            // The true projections, as exact rationals would give them, lie
            // inside the stored interval for every direction.
            for (index, exact) in [
                p.x,
                p.y,
                // `next_down`/`next_up` of the rounded sum bracket the true
                // value; comparing against the rounded value is therefore a
                // weaker but sufficient check that the bracket did not shrink.
                p.x + p.y,
                p.x - p.y,
            ]
            .into_iter()
            .enumerate()
            {
                assert!(
                    slabs.min[index] <= exact && exact <= slabs.max[index],
                    "direction {index}: {exact} escaped [{}, {}]",
                    slabs.min[index],
                    slabs.max[index]
                );
            }
        }
        // A set displaced by a known amount: the certified lower bound must not
        // exceed the true gap.
        let shifted = MaterialSet {
            regions: vec![MaterialRegion {
                outer: awkward.regions[0]
                    .outer
                    .iter()
                    .map(|p| point(p.x + 1000.0, p.y))
                    .collect(),
                holes: Vec::new(),
                material_sample: Some(point(1000.2, 0.3)),
            }],
        };
        let other = ClearanceSlabs::of(&shifted).expect("in domain");
        let bound = slabs.gap(&other, 0).next_down();
        let truth = 1000.0 + 0.1 - (1e15 + 0.1);
        assert!(
            bound <= truth,
            "the certified lower bound {bound} exceeds the true gap {truth}"
        );
    }

    /// The reference path must certify nothing, or the shadow harness would be
    /// comparing the filter against itself.
    #[cfg(feature = "fast-contract-validator")]
    #[test]
    fn the_disarmed_phase_certifies_nothing() {
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
                translate_x: 12.0,
                translate_y: 1.0,
            },
        ];
        let transformed = placements
            .iter()
            .map(transform_placement)
            .collect::<Result<Vec<_>, _>>()
            .expect("both placements are measurable");
        // Armed, this pair is far apart and is proved clear.
        assert!(ClearanceBroadPhase::new(&transformed, 0.5).provably_clear(0, 1));
        // Disarmed, nothing is.
        assert!(!ClearanceBroadPhase::disarmed(transformed.len()).provably_clear(0, 1));
        // And the two public entry points agree on the verdict.
        assert_eq!(
            validate_publication(&placements, settings()).is_ok(),
            validate_publication_exact_reference(&placements, settings()).is_ok()
        );
    }

    /// The whole point, stated as a test: the filtered path and the exact path
    /// decide every input identically, including the error message.
    ///
    /// This used to compare the feature-on path against *enumerated
    /// expectations*, which is not two implementations meeting - a wrong
    /// expectation and a wrong filter would have agreed. Now
    /// [`validate_publication_exact_reference`] runs the exact loop on every
    /// pair in the same process, and the two `Result`s are compared whole. The
    /// enumerated expectations are kept as a third opinion, so the test still
    /// fails if both paths move together.
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
            let filtered = validate_publication(&placements, clearance_settings)
                .map_err(|error| error.message().to_string());
            let exact = validate_publication_exact_reference(&placements, clearance_settings)
                .map_err(|error| error.message().to_string());
            assert_eq!(
                filtered, exact,
                "the filtered and exact paths disagreed at rotation {rotation}, ({tx}, {ty})"
            );
            assert_eq!(
                filtered.is_ok(),
                expected_ok,
                "verdict changed at rotation {rotation}, ({tx}, {ty})"
            );
        }
    }
}
