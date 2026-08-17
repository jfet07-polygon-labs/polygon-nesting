//! An adapter skeleton for the pinned `jagua-rs` 0.7.2 collision engine.
//!
//! # Status: skeleton, wired into nothing
//!
//! [`JaguaKernel`] exists so that the seam in [`super`] can be shown to admit a
//! second implementation, and so that the parity work the roadmap requires can
//! start against a compiling target. It is **not** reachable from any default
//! path, any production route, or any mode the CLI can select. Nothing
//! constructs one except its own tests and the parity smoke test in
//! `tests/kernel_parity.rs`.
//!
//! That is deliberate. The next-generation plan lists the gates the dependency
//! must clear before it may be default — three identical stream fingerprints,
//! no silent conversion repair, no false negative outside a derived ambiguity
//! band, at least 3x the rollback kernel's complete-query throughput, a peak
//! RSS at or below the ~143 MiB baseline — and none of them are claimed here.
//!
//! # What this skeleton does and does not share with production
//!
//! * **Shared with the existing adapter.** The conversion contract is the same
//!   one [`crate::search::general_hazard`] uses: exactly one offset region, no
//!   holes, recentred around a deterministic anchor, `f32` conversion checked
//!   rather than saturating. A conversion that would need repair is rejected
//!   and counted, never silently fixed.
//! * **Deliberately different.** A production kernel keeps one lane-local
//!   dynamic hazard index and updates one hazard per accepted move. This
//!   skeleton builds a container and a layout *per pair query*, because a
//!   symmetric two-shape verdict is what a parity test needs and an incremental
//!   index is what PR4 is for. Do not read any throughput number off this file.
//!
//! # The exact tier is not jagua's
//!
//! [`JaguaKernel`]'s exact tier forwards to
//! [`LegacyKernel`](super::LegacyKernel) verbatim. This is the structural form
//! of the roadmap's refusal to put `f32`, a tolerance, or jagua into
//! publication authority: even if this kernel were wired in tomorrow, an exact
//! overlap or collision-polygon build asked through it would still be the `f64`
//! Clipper answer. Jagua's verdicts rank and prune; they never publish.
//!
//! # Ambiguity is a property of the proxy, not a bug in it
//!
//! Jagua converts to `f32` and treats contact by its own convention. Its
//! verdicts therefore cannot agree with the `f64` triangle surrogate on
//! near-contact poses, and the plan says as much: agreement is required
//! *outside* an explicitly derived ambiguity band. [`JaguaShape::ambiguity_mm`]
//! derives that band from the converted geometry, and the parity test uses it
//! to keep its comparison honest instead of quietly widening a tolerance until
//! it passes.

use std::sync::Arc;

use jagua_rs::collision_detection::hazards::collector::{BasicHazardCollector, HazardCollector};
use jagua_rs::collision_detection::hazards::{Hazard, HazardEntity};
use jagua_rs::collision_detection::{CDEConfig, CDEngine};
use jagua_rs::entities::{Container, Item, Layout};
use jagua_rs::geometry::fail_fast::SPSurrogateConfig;
use jagua_rs::geometry::geo_enums::RotationRange;
use jagua_rs::geometry::geo_traits::TransformableFrom;
use jagua_rs::geometry::primitives::{Point, Rect, SPolygon};
use jagua_rs::geometry::shape_modification::{ShapeModifyConfig, ShapeModifyMode};
use jagua_rs::geometry::{DTransformation, OriginalShape};

use crate::domain::{IrregularBounds, IrregularPoint};
use crate::geometry::general_polygon::{GeneralPolygonError, PolygonSet};

use super::{ExplorationKernel, KernelPose, KernelProbes, PosedShape, LEGACY};

/// Quadtree depth and collision-detection threshold, matching the existing
/// adapter so that a later comparison is between search architectures rather
/// than between two arbitrary quadtree configurations.
const QUADTREE_DEPTH: u8 = 4;
const COLLISION_DETECTION_THRESHOLD: u8 = 16;

/// Margin, as a multiple of the pair's combined diameter, between the shapes
/// and the container ring built around them.
///
/// The container's own ring is a hazard. Keeping it this far away means the
/// exterior can never contribute an edge intersection to a pair verdict, so the
/// verdict is about the two shapes and nothing else.
const CONTAINER_MARGIN_RATIO: f64 = 1.0;

/// One piece geometry converted for jagua, at one baked orientation.
///
/// Rotation and mirroring are applied in `f64` *before* conversion, exactly as
/// the legacy surrogate bakes them in before triangulating. A pose is therefore
/// a pure translation at query time, which removes `f32` rotation arithmetic
/// from the comparison and leaves conversion as the only representational
/// difference between the two kernels.
#[derive(Clone)]
pub struct JaguaShape {
    /// The jagua item, whose collision shape is centred on the anchor.
    item: Item,
    /// Scratch buffer for the transformed query shape, sized once.
    scratch: SPolygon,
    /// The `f64` centroid-of-bounds the conversion subtracted.
    anchor: IrregularPoint,
    /// `f64` extent of the oriented, expanded ring before translation.
    bounds: IrregularBounds,
    /// Largest `f64` -> `f32` coordinate error observed during conversion.
    conversion_error_mm: f64,
}

impl JaguaShape {
    /// Converts one oriented, expanded collision ring for jagua.
    ///
    /// `source` is the *unoriented* source ring; `pose`'s rotation and mirror
    /// are baked in and its translation is ignored (a pose translation is
    /// applied per query instead). `expansion_mm` is the contract's collision
    /// expansion.
    ///
    /// Fails - rather than repairing - when the offset produces anything other
    /// than a single hole-free region, or when a coordinate does not survive
    /// the `f32` conversion.
    pub fn prepare(
        id: usize,
        source: &PolygonSet,
        pose: KernelPose,
        expansion_mm: f64,
        allow_rotation: bool,
    ) -> Result<Self, GeneralPolygonError> {
        let expanded = LEGACY.collision_polygon(
            source,
            KernelPose::oriented(pose.rotation_deg, pose.mirrored),
            expansion_mm,
        )?;
        if expanded.regions().len() != 1 {
            return Err(GeneralPolygonError::from_message(format!(
                "jagua conversion requires one offset region, found {}",
                expanded.regions().len()
            )));
        }
        let region = &expanded.regions()[0];
        if !region.holes.is_empty() {
            return Err(GeneralPolygonError::from_message(
                "jagua conversion does not flatten offset holes",
            ));
        }
        let bounds = region.outer.bounds();
        let anchor = IrregularPoint::new(
            bounds.min_x + (bounds.max_x - bounds.min_x) * 0.5,
            bounds.min_y + (bounds.max_y - bounds.min_y) * 0.5,
        );
        let mut conversion_error_mm = 0.0_f64;
        let mut points = Vec::with_capacity(region.outer.points().len());
        for point in region.outer.points() {
            let centered_x = point.x - anchor.x;
            let centered_y = point.y - anchor.y;
            let converted = Point(
                checked_f32(centered_x, "centred x")?,
                checked_f32(centered_y, "centred y")?,
            );
            conversion_error_mm = conversion_error_mm
                .max((converted.0 as f64 - centered_x).abs())
                .max((converted.1 as f64 - centered_y).abs());
            points.push(converted);
        }
        let mut shape = SPolygon::new(points).map_err(|error| {
            GeneralPolygonError::from_message(format!(
                "jagua rejected the converted polygon: {error}"
            ))
        })?;
        shape
            .generate_surrogate(surrogate_config())
            .map_err(|error| {
                GeneralPolygonError::from_message(format!(
                    "jagua surrogate generation failed: {error}"
                ))
            })?;
        let scratch = shape.clone();
        let shape_cd = Arc::new(shape.clone());
        let shape_orig = Arc::new(OriginalShape {
            shape,
            pre_transform: DTransformation::empty(),
            modify_mode: ShapeModifyMode::Inflate,
            modify_config: ShapeModifyConfig::default(),
        });
        Ok(Self {
            item: Item {
                id,
                shape_orig,
                shape_cd,
                allowed_rotation: if allow_rotation {
                    RotationRange::Continuous
                } else {
                    RotationRange::None
                },
                min_quality: None,
                surrogate_config: surrogate_config(),
            },
            scratch,
            anchor,
            bounds,
            conversion_error_mm,
        })
    }

    /// The `f64` extent of this shape when translated to `(x, y)`.
    pub fn translated_bounds(&self, translate_x: f64, translate_y: f64) -> IrregularBounds {
        IrregularBounds::new(
            self.bounds.min_x + translate_x,
            self.bounds.min_y + translate_y,
            self.bounds.max_x + translate_x,
            self.bounds.max_y + translate_y,
        )
    }

    /// The largest separation at which this shape's verdict may legitimately
    /// disagree with an `f64` kernel's.
    ///
    /// A vertex moved by at most `conversion_error_mm` during conversion can
    /// move an edge by at most that much, so two converted shapes can disagree
    /// with the `f64` answer only while their true clearance or penetration is
    /// within the sum of their two errors. This is derived from the geometry
    /// that was actually converted; it is not a tuned tolerance, and it is
    /// never consulted by anything that decides feasibility.
    pub fn ambiguity_mm(&self, other: &JaguaShape) -> f64 {
        self.conversion_error_mm + other.conversion_error_mm
    }

    fn diameter_mm(&self) -> f64 {
        (self.bounds.max_x - self.bounds.min_x).hypot(self.bounds.max_y - self.bounds.min_y)
    }
}

/// The pinned-`jagua-rs` exploration kernel, as a compiling skeleton.
///
/// See the module documentation: this is not wired into any default path and
/// carries none of the dependency gates. Its exact tier is the legacy kernel's.
pub struct JaguaKernel {
    cde_config: CDEConfig,
    collector: BasicHazardCollector,
    /// The first conversion or query error since [`Self::take_error`], if any.
    ///
    /// [`ExplorationKernel::pair_collides`] returns a `bool`, so a failure here
    /// has to be reported out of band. The verdict returned on failure is
    /// `true` - fail-closed, the candidate is treated as colliding - so that a
    /// dropped error can only ever cost a candidate, never admit an illegal
    /// one.
    error: Option<String>,
}

impl Default for JaguaKernel {
    fn default() -> Self {
        Self::new()
    }
}

impl JaguaKernel {
    /// Builds a kernel with the same quadtree configuration as the existing
    /// hazard adapter.
    pub fn new() -> Self {
        Self {
            cde_config: CDEConfig {
                quadtree_depth: QUADTREE_DEPTH,
                cd_threshold: COLLISION_DETECTION_THRESHOLD,
                item_surrogate_config: surrogate_config(),
            },
            collector: BasicHazardCollector::new(),
            error: None,
        }
    }

    /// Takes the first error recorded since the last call, if any.
    pub fn take_error(&mut self) -> Option<String> {
        self.error.take()
    }

    fn record(&mut self, message: impl Into<String>) {
        if self.error.is_none() {
            self.error = Some(message.into());
        }
    }

    /// The pair verdict, with the failure mode reported instead of swallowed.
    fn try_pair_collides(
        &mut self,
        first: PosedShape<'_, JaguaShape>,
        second: PosedShape<'_, JaguaShape>,
        probes: &mut KernelProbes,
    ) -> Result<bool, String> {
        let first_bounds = first
            .shape
            .translated_bounds(first.translate_x, first.translate_y);
        let second_bounds = second
            .shape
            .translated_bounds(second.translate_x, second.translate_y);
        probes.cell_index_probes = probes.cell_index_probes.wrapping_add(1);
        if !bounds_overlap(first_bounds, second_bounds) {
            return Ok(false);
        }

        let margin =
            CONTAINER_MARGIN_RATIO * first.shape.diameter_mm().max(second.shape.diameter_mm());
        let container_bounds = IrregularBounds::new(
            first_bounds.min_x.min(second_bounds.min_x) - margin,
            first_bounds.min_y.min(second_bounds.min_y) - margin,
            first_bounds.max_x.max(second_bounds.max_x) + margin,
            first_bounds.max_y.max(second_bounds.max_y) + margin,
        );
        let mut layout = Layout::new(build_container(container_bounds, self.cde_config)?);

        let placed = layout.place_item(
            &first.shape.item,
            translation_for(first.shape, first.translate_x, first.translate_y)?,
        );
        // A placement that produced no hazard would make the query silently
        // answer "no collision" against nothing at all.
        layout
            .cde()
            .haz_key_from_pi_key(placed)
            .ok_or_else(|| "jagua placed the first shape without a hazard".to_owned())?;

        let mut moving = second.shape.scratch.clone();
        moving.transform_from(
            second.shape.item.shape_cd.as_ref(),
            &translation_for(second.shape, second.translate_x, second.translate_y)?.compose(),
        );
        self.collector.clear();
        layout
            .cde()
            .collect_poly_collisions(&moving, &mut self.collector);
        probes.sat_tests = probes.sat_tests.wrapping_add(1);

        let mut collides = false;
        for entity in self.collector.entities() {
            match entity {
                HazardEntity::PlacedItem { .. } => collides = true,
                HazardEntity::Exterior => {
                    return Err(
                        "the pair container is too small: the exterior entered a pair verdict"
                            .to_owned(),
                    )
                }
                HazardEntity::Hole { .. } | HazardEntity::InferiorQualityZone { .. } => {
                    return Err("the pair container declared an unexpected zone".to_owned())
                }
            }
        }
        Ok(collides)
    }
}

impl ExplorationKernel for JaguaKernel {
    type Shape = JaguaShape;

    fn pair_collides(
        &mut self,
        first: PosedShape<'_, Self::Shape>,
        second: PosedShape<'_, Self::Shape>,
        probes: &mut KernelProbes,
    ) -> bool {
        match self.try_pair_collides(first, second, probes) {
            Ok(collides) => collides,
            Err(message) => {
                self.record(message);
                // Fail closed: an unanswerable query rejects the candidate.
                true
            }
        }
    }

    fn pair_pressure(
        &self,
        first: PosedShape<'_, Self::Shape>,
        second: PosedShape<'_, Self::Shape>,
    ) -> f64 {
        // Pole-pair overlap, the same shape of proxy the dynamic-hazard
        // pressure model already uses. Rotation is baked into the shape, so a
        // pose only shifts the poles.
        let mut pressure = 0.0_f64;
        for first_pole in &first.shape.item.shape_cd.surrogate().poles {
            let first_x = first_pole.center.0 as f64 + first.shape.anchor.x + first.translate_x;
            let first_y = first_pole.center.1 as f64 + first.shape.anchor.y + first.translate_y;
            for second_pole in &second.shape.item.shape_cd.surrogate().poles {
                let second_x =
                    second_pole.center.0 as f64 + second.shape.anchor.x + second.translate_x;
                let second_y =
                    second_pole.center.1 as f64 + second.shape.anchor.y + second.translate_y;
                let reach = first_pole.radius as f64 + second_pole.radius as f64;
                let distance = (second_x - first_x).hypot(second_y - first_y);
                if distance < reach {
                    pressure += reach - distance;
                }
            }
        }
        pressure
    }

    fn collision_polygon(
        &self,
        source: &PolygonSet,
        pose: KernelPose,
        expansion_mm: f64,
    ) -> Result<PolygonSet, GeneralPolygonError> {
        // Not jagua's. See the module documentation: the exact tier is `f64`
        // Clipper for every kernel, because it is what publication measures.
        LEGACY.collision_polygon(source, pose, expansion_mm)
    }

    fn exact_pair_overlaps(
        &self,
        first: &PolygonSet,
        first_bounds: Option<IrregularBounds>,
        second: &PolygonSet,
        second_bounds: Option<IrregularBounds>,
    ) -> Result<bool, GeneralPolygonError> {
        LEGACY.exact_pair_overlaps(first, first_bounds, second, second_bounds)
    }
}

fn surrogate_config() -> SPSurrogateConfig {
    SPSurrogateConfig {
        n_pole_limits: [(64, 0.0), (16, 0.8), (8, 0.9)],
        n_ff_poles: 1,
        n_ff_piers: 0,
    }
}

fn bounds_overlap(first: IrregularBounds, second: IrregularBounds) -> bool {
    first.min_x < second.max_x
        && first.max_x > second.min_x
        && first.min_y < second.max_y
        && first.max_y > second.min_y
}

/// The pure-translation jagua transform for a posed shape.
///
/// The shape was recentred on its anchor at conversion time, so placing it back
/// at `(translate_x, translate_y)` in engine coordinates means translating by
/// the anchor plus the pose.
fn translation_for(
    shape: &JaguaShape,
    translate_x: f64,
    translate_y: f64,
) -> Result<DTransformation, String> {
    let x = checked_f32(shape.anchor.x + translate_x, "pose x")
        .map_err(|error| error.to_string())?;
    let y = checked_f32(shape.anchor.y + translate_y, "pose y")
        .map_err(|error| error.to_string())?;
    Ok(DTransformation::new(0.0, (x, y)))
}

/// A rectangular container whose ring is far enough from the pair to stay out
/// of the verdict.
fn build_container(bounds: IrregularBounds, cde_config: CDEConfig) -> Result<Container, String> {
    let corners = [
        (bounds.min_x, bounds.min_y),
        (bounds.max_x, bounds.min_y),
        (bounds.max_x, bounds.max_y),
        (bounds.min_x, bounds.max_y),
    ];
    let mut points = Vec::with_capacity(corners.len());
    for (x, y) in corners {
        points.push(Point(
            checked_f32(x, "container x").map_err(|error| error.to_string())?,
            checked_f32(y, "container y").map_err(|error| error.to_string())?,
        ));
    }
    let outer = SPolygon::new(points)
        .map_err(|error| format!("invalid pair container: {error}"))?;
    let original_outer = Arc::new(OriginalShape {
        shape: outer.clone(),
        pre_transform: DTransformation::empty(),
        modify_mode: ShapeModifyMode::Deflate,
        modify_config: ShapeModifyConfig::default(),
    });
    let outer = Arc::new(outer);
    let span = (bounds.max_x - bounds.min_x).max(bounds.max_y - bounds.min_y);
    let root = Rect::try_new(
        checked_f32(bounds.min_x - span, "envelope min x").map_err(|error| error.to_string())?,
        checked_f32(bounds.min_y - span, "envelope min y").map_err(|error| error.to_string())?,
        checked_f32(bounds.max_x + span, "envelope max x").map_err(|error| error.to_string())?,
        checked_f32(bounds.max_y + span, "envelope max y").map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("invalid pair envelope: {error}"))?;
    let base_cde = Arc::new(CDEngine::new(
        root,
        vec![Hazard::new(HazardEntity::Exterior, outer.clone(), false)],
        cde_config,
    ));
    Ok(Container {
        id: 0,
        outer_orig: original_outer,
        outer_cd: outer,
        quality_zones: Default::default(),
        base_cde,
    })
}

/// `f64` -> `f32` with the failure surfaced rather than saturated.
fn checked_f32(value: f64, label: &str) -> Result<f32, GeneralPolygonError> {
    let converted = value as f32;
    if !value.is_finite() || !converted.is_finite() {
        return Err(GeneralPolygonError::from_message(format!(
            "{label} cannot be represented by the jagua backend"
        )));
    }
    Ok(converted)
}
