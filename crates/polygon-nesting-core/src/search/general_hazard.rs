//! Optional dynamic collision-index experiment for the general engine.
//!
//! This module is deliberately isolated behind `jagua-experimental`. Jagua is
//! an exploration aid; the existing f64 publication validators remain the
//! only legality authority.

use std::fmt::{Display, Formatter};
use std::sync::Arc;

use jagua_rs::collision_detection::hazards::collector::{BasicHazardCollector, HazardCollector};
use jagua_rs::collision_detection::hazards::{HazKey, Hazard, HazardEntity};
use jagua_rs::collision_detection::{CDEConfig, CDEngine};
use jagua_rs::entities::{Container, Item, Layout, PItemKey};
use jagua_rs::geometry::fail_fast::SPSurrogateConfig;
use jagua_rs::geometry::geo_enums::RotationRange;
use jagua_rs::geometry::geo_traits::TransformableFrom;
use jagua_rs::geometry::primitives::{Point, Rect, SPolygon};
use jagua_rs::geometry::shape_modification::{ShapeModifyConfig, ShapeModifyMode};
use jagua_rs::geometry::{DTransformation, OriginalShape, Transformation};
use sha2::{Digest, Sha256};

use crate::canonical_grid::{from_grid, to_grid_mm, CLIPPER2_OFFSET_SCALE};
use crate::domain::{IrregularBounds, IrregularPoint};
use crate::geometry::general_polygon::PolygonSet;
use crate::search::general_fast::{
    collision_expansion_mm, collision_sheet_inset_mm, GeneralFastPiece, GeneralFastSettings,
};

const QUADTREE_DEPTH: u8 = 4;
const COLLISION_DETECTION_THRESHOLD: u8 = 16;
const OVERLAP_PROXY_EPSILON_DIAMETER_RATIO: f32 = 0.01;
// two grid units conservatively cover snap-before-transform versus
// transform-before-offset drift at the independent publication boundary
const CANONICAL_TRANSFORM_OFFSET_GUARD_MM: f64 = 2.0 / CLIPPER2_OFFSET_SCALE;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeneralHazardPose {
    pub rotation_deg: f64,
    pub mirrored: bool,
    pub translate_short_axis: f64,
    pub translate_long_axis: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneralHazardQuery {
    /// The fail-fast stage proved at least `lower_bound` collisions, but the
    /// full colliding-piece set was intentionally not collected.
    Pruned { lower_bound: usize },
    /// The complete Jagua result within the index query envelope.
    ///
    /// This is still only a conservative search-kernel result. A candidate
    /// selected for publication must pass the independent f64 validator.
    Complete {
        boundary: bool,
        colliding_piece_ids: Vec<usize>,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneralHazardCounters {
    pub fail_fast_queries: usize,
    pub pruned_queries: usize,
    pub complete_queries: usize,
    pub collected_piece_ids: usize,
    pub hazard_updates: usize,
    pub index_rebuilds: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneralHazardError {
    message: String,
}

impl GeneralHazardError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for GeneralHazardError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GeneralHazardError {}

#[derive(Clone)]
struct SharedVariant {
    anchor: IrregularPoint,
    precise_points: Arc<[IrregularPoint]>,
    shape_orig: Arc<OriginalShape>,
    shape_cd: Arc<SPolygon>,
}

struct PieceVariant {
    anchor: IrregularPoint,
    precise_points: Arc<[IrregularPoint]>,
    item: Item,
    scratch: SPolygon,
}

impl PieceVariant {
    fn new(
        id: usize,
        shared: &SharedVariant,
        allow_rotation: bool,
        surrogate_config: SPSurrogateConfig,
    ) -> Self {
        let allowed_rotation = if allow_rotation {
            RotationRange::Continuous
        } else {
            RotationRange::None
        };
        Self {
            anchor: shared.anchor,
            precise_points: shared.precise_points.clone(),
            item: Item {
                id,
                shape_orig: shared.shape_orig.clone(),
                shape_cd: shared.shape_cd.clone(),
                allowed_rotation,
                min_quality: None,
                surrogate_config,
            },
            scratch: shared.shape_cd.as_ref().clone(),
        }
    }
}

struct PieceVariants {
    original: PieceVariant,
    mirrored: Option<PieceVariant>,
}

impl PieceVariants {
    fn select_mut(
        &mut self,
        mirrored: bool,
        stable_id: usize,
    ) -> Result<&mut PieceVariant, GeneralHazardError> {
        if mirrored {
            self.mirrored.as_mut().ok_or_else(|| {
                GeneralHazardError::new(format!(
                    "piece {stable_id} does not allow mirrored hazard poses"
                ))
            })
        } else {
            Ok(&mut self.original)
        }
    }

    fn select(
        &self,
        mirrored: bool,
        stable_id: usize,
    ) -> Result<&PieceVariant, GeneralHazardError> {
        if mirrored {
            self.mirrored.as_ref().ok_or_else(|| {
                GeneralHazardError::new(format!(
                    "piece {stable_id} does not allow mirrored hazard poses"
                ))
            })
        } else {
            Ok(&self.original)
        }
    }
}

#[derive(Clone, Copy)]
struct CurrentHandle {
    placed_item_key: PItemKey,
    hazard_key: HazKey,
}

pub struct JaguaHazardIndex {
    pieces: Vec<PieceVariants>,
    layout: Layout,
    handles: Vec<Option<CurrentHandle>>,
    sheet_short_axis_mm: f64,
    sheet_inset_mm: f64,
    cde_config: CDEConfig,
    counters: GeneralHazardCounters,
}

pub(crate) struct JaguaHazardCatalog {
    shared: Vec<(SharedVariant, Option<SharedVariant>)>,
    contracts: Vec<JaguaHazardPieceContract>,
    collision_expansion_bits: u64,
    immutable_variant_count: usize,
}

struct JaguaHazardPieceContract {
    geometry_class: usize,
    polygon_fingerprint: [u8; 32],
    allow_rotation: bool,
    allow_mirror: bool,
}

impl JaguaHazardCatalog {
    pub(crate) fn new(
        pieces: &[GeneralFastPiece<'_>],
        settings: GeneralFastSettings,
    ) -> Result<Self, GeneralHazardError> {
        let surrogate_config = default_surrogate_config();
        let mut representatives = Vec::<usize>::new();
        let mut contracts = Vec::with_capacity(pieces.len());
        for (input_index, piece) in pieces.iter().enumerate() {
            let geometry_class = representatives
                .iter()
                .position(|representative| pieces[*representative].polygon == piece.polygon)
                .unwrap_or_else(|| {
                    representatives.push(input_index);
                    representatives.len() - 1
                });
            contracts.push(JaguaHazardPieceContract {
                geometry_class,
                polygon_fingerprint: polygon_contract_fingerprint(piece.polygon),
                allow_rotation: piece.allow_rotation,
                allow_mirror: piece.allow_mirror,
            });
        }

        let expansion_mm = collision_expansion_mm(settings);
        let mut shared = Vec::with_capacity(representatives.len());
        let mut immutable_variant_count = 0usize;
        for (geometry_class, representative) in representatives.iter().copied().enumerate() {
            let original = build_shared_variant(
                pieces[representative].polygon,
                false,
                expansion_mm,
                surrogate_config,
            )?;
            immutable_variant_count = immutable_variant_count.saturating_add(1);
            let needs_mirror = pieces.iter().enumerate().any(|(input_index, piece)| {
                contracts[input_index].geometry_class == geometry_class && piece.allow_mirror
            });
            let mirrored = needs_mirror
                .then(|| {
                    build_shared_variant(
                        pieces[representative].polygon,
                        true,
                        expansion_mm,
                        surrogate_config,
                    )
                })
                .transpose()?;
            immutable_variant_count =
                immutable_variant_count.saturating_add(usize::from(mirrored.is_some()));
            shared.push((original, mirrored));
        }
        Ok(Self {
            shared,
            contracts,
            collision_expansion_bits: expansion_mm.to_bits(),
            immutable_variant_count,
        })
    }

    pub(crate) fn immutable_variant_count(&self) -> usize {
        self.immutable_variant_count
    }

    fn validate(
        &self,
        pieces: &[GeneralFastPiece<'_>],
        settings: GeneralFastSettings,
    ) -> Result<(), GeneralHazardError> {
        if pieces.len() != self.contracts.len() {
            return Err(GeneralHazardError::new(format!(
                "hazard catalog contains {} pieces but {} were requested",
                self.contracts.len(),
                pieces.len()
            )));
        }
        if collision_expansion_mm(settings).to_bits() != self.collision_expansion_bits {
            return Err(GeneralHazardError::new(
                "hazard catalog collision expansion does not match the requested settings",
            ));
        }
        for (stable_id, (piece, contract)) in pieces.iter().zip(&self.contracts).enumerate() {
            if polygon_contract_fingerprint(piece.polygon) != contract.polygon_fingerprint
                || piece.allow_rotation != contract.allow_rotation
                || piece.allow_mirror != contract.allow_mirror
            {
                return Err(GeneralHazardError::new(format!(
                    "hazard catalog contract does not match piece {stable_id}"
                )));
            }
        }
        Ok(())
    }

    fn instantiate(
        &self,
        pieces: &[GeneralFastPiece<'_>],
        surrogate_config: SPSurrogateConfig,
    ) -> Result<Vec<PieceVariants>, GeneralHazardError> {
        pieces
            .iter()
            .enumerate()
            .map(|(stable_id, piece)| {
                let geometry_class = self.contracts[stable_id].geometry_class;
                let (original, mirrored) = self.shared.get(geometry_class).ok_or_else(|| {
                    GeneralHazardError::new(format!(
                        "hazard catalog is missing geometry class {geometry_class}"
                    ))
                })?;
                Ok(PieceVariants {
                    original: PieceVariant::new(
                        stable_id,
                        original,
                        piece.allow_rotation,
                        surrogate_config,
                    ),
                    mirrored: piece
                        .allow_mirror
                        .then(|| {
                            mirrored.as_ref().ok_or_else(|| {
                                GeneralHazardError::new(format!(
                                    "hazard catalog is missing mirrored geometry for piece {stable_id}"
                                ))
                            })
                        })
                        .transpose()?
                        .map(|mirrored| {
                            PieceVariant::new(
                                stable_id,
                                mirrored,
                                piece.allow_rotation,
                                surrogate_config,
                            )
                        }),
                })
            })
            .collect()
    }
}

pub fn hazard_collision_expansion_mm(settings: GeneralFastSettings) -> f64 {
    collision_expansion_mm(settings)
}

pub fn hazard_sheet_inset_mm(settings: GeneralFastSettings) -> f64 {
    collision_sheet_inset_mm(settings)
}

impl JaguaHazardIndex {
    pub fn new(
        pieces: &[GeneralFastPiece<'_>],
        settings: GeneralFastSettings,
        strip_depth_mm: f64,
        poses: &[GeneralHazardPose],
    ) -> Result<Self, GeneralHazardError> {
        if pieces.len() != poses.len() {
            return Err(GeneralHazardError::new(format!(
                "hazard index received {} pieces but {} poses",
                pieces.len(),
                poses.len()
            )));
        }
        let catalog = JaguaHazardCatalog::new(pieces, settings)?;
        Self::from_catalog(pieces, settings, strip_depth_mm, poses, &catalog)
    }

    pub(crate) fn from_catalog(
        pieces: &[GeneralFastPiece<'_>],
        settings: GeneralFastSettings,
        strip_depth_mm: f64,
        poses: &[GeneralHazardPose],
        catalog: &JaguaHazardCatalog,
    ) -> Result<Self, GeneralHazardError> {
        Self::from_catalog_active(
            pieces,
            settings,
            strip_depth_mm,
            poses,
            &vec![true; pieces.len()],
            catalog,
        )
    }

    pub(crate) fn from_catalog_active(
        pieces: &[GeneralFastPiece<'_>],
        settings: GeneralFastSettings,
        strip_depth_mm: f64,
        poses: &[GeneralHazardPose],
        active: &[bool],
        catalog: &JaguaHazardCatalog,
    ) -> Result<Self, GeneralHazardError> {
        if pieces.len() != poses.len() {
            return Err(GeneralHazardError::new(format!(
                "hazard index received {} pieces but {} poses",
                pieces.len(),
                poses.len()
            )));
        }
        if pieces.len() != active.len() {
            return Err(GeneralHazardError::new(format!(
                "hazard index received {} pieces but {} active flags",
                pieces.len(),
                active.len()
            )));
        }
        catalog.validate(pieces, settings)?;
        let surrogate_config = default_surrogate_config();
        let cde_config = CDEConfig {
            quadtree_depth: QUADTREE_DEPTH,
            cd_threshold: COLLISION_DETECTION_THRESHOLD,
            item_surrogate_config: surrogate_config,
        };
        let variants = catalog.instantiate(pieces, surrogate_config)?;
        let sheet_inset_mm = collision_sheet_inset_mm(settings);
        let layout = build_layout(
            settings.sheet_short_axis_mm,
            strip_depth_mm,
            sheet_inset_mm,
            cde_config,
        )?;
        let mut index = Self {
            pieces: variants,
            layout,
            handles: vec![None; pieces.len()],
            sheet_short_axis_mm: settings.sheet_short_axis_mm,
            sheet_inset_mm,
            cde_config,
            counters: GeneralHazardCounters::default(),
        };
        index.place_active(poses, active)?;
        Ok(index)
    }

    pub fn counters(&self) -> GeneralHazardCounters {
        self.counters
    }

    pub fn query(
        &mut self,
        moving_piece_id: usize,
        pose: GeneralHazardPose,
        prune_at_or_above: Option<usize>,
    ) -> Result<GeneralHazardQuery, GeneralHazardError> {
        let handle = self
            .handles
            .get(moving_piece_id)
            .copied()
            .flatten()
            .ok_or_else(|| {
                GeneralHazardError::new(format!(
                    "moving piece {moving_piece_id} is not active in the hazard index"
                ))
            })?;
        self.query_internal(moving_piece_id, pose, prune_at_or_above, Some(handle))
    }

    pub(crate) fn query_unplaced(
        &mut self,
        moving_piece_id: usize,
        pose: GeneralHazardPose,
    ) -> Result<GeneralHazardQuery, GeneralHazardError> {
        let active = self.handles.get(moving_piece_id).ok_or_else(|| {
            GeneralHazardError::new(format!("unknown moving piece {moving_piece_id}"))
        })?;
        if active.is_some() {
            return Err(GeneralHazardError::new(format!(
                "moving piece {moving_piece_id} is already active in the hazard index"
            )));
        }
        self.query_internal(moving_piece_id, pose, None, None)
    }

    fn query_internal(
        &mut self,
        moving_piece_id: usize,
        pose: GeneralHazardPose,
        prune_at_or_above: Option<usize>,
        own_handle: Option<CurrentHandle>,
    ) -> Result<GeneralHazardQuery, GeneralHazardError> {
        let (layout, pieces) = (&self.layout, &mut self.pieces);
        let variant = pieces
            .get_mut(moving_piece_id)
            .ok_or_else(|| {
                GeneralHazardError::new(format!("unknown moving piece {moving_piece_id}"))
            })?
            .select_mut(pose.mirrored, moving_piece_id)?;
        let transform = transform_for_pose(variant.anchor, pose)?;

        if let Some(limit) = prune_at_or_above {
            if limit == 0 {
                self.counters.pruned_queries = self.counters.pruned_queries.saturating_add(1);
                return Ok(GeneralHazardQuery::Pruned { lower_bound: 0 });
            }
        }

        variant
            .scratch
            .transform_from(variant.item.shape_cd.as_ref(), &transform);
        let root = layout.cde().bbox();
        let candidate_bounds = variant.scratch.bbox;
        if candidate_bounds.x_min < root.x_min
            || candidate_bounds.y_min < root.y_min
            || candidate_bounds.x_max > root.x_max
            || candidate_bounds.y_max > root.y_max
        {
            return Err(GeneralHazardError::new(format!(
                "piece {moving_piece_id} lies outside the complete-query envelope"
            )));
        }
        if prune_at_or_above == Some(1) && own_handle.is_some() {
            self.counters.fail_fast_queries = self.counters.fail_fast_queries.saturating_add(1);
            if layout.cde().detect_surrogate_collision(
                variant.item.shape_cd.surrogate(),
                &transform,
                &own_handle.expect("checked above").hazard_key,
            ) {
                self.counters.pruned_queries = self.counters.pruned_queries.saturating_add(1);
                return Ok(GeneralHazardQuery::Pruned { lower_bound: 1 });
            }
        }
        let mut collector = BasicHazardCollector::new();
        if let Some(handle) = own_handle {
            let own_entity = layout
                .cde()
                .hazards_map
                .get(handle.hazard_key)
                .ok_or_else(|| {
                    GeneralHazardError::new(format!(
                        "piece {moving_piece_id} references a stale hazard handle"
                    ))
                })?
                .entity;
            collector.insert(handle.hazard_key, own_entity);
        }
        layout
            .cde()
            .collect_poly_collisions(&variant.scratch, &mut collector);
        if let Some(handle) = own_handle {
            collector.remove_by_key(handle.hazard_key);
        }

        let mut boundary = false;
        let mut colliding_piece_ids = Vec::with_capacity(collector.len());
        for entity in collector.entities() {
            match *entity {
                HazardEntity::PlacedItem { id, .. } => {
                    if id != moving_piece_id {
                        colliding_piece_ids.push(id);
                    }
                }
                HazardEntity::Exterior
                | HazardEntity::Hole { .. }
                | HazardEntity::InferiorQualityZone { .. } => boundary = true,
            }
        }
        colliding_piece_ids.sort_unstable();
        colliding_piece_ids.dedup();
        self.counters.complete_queries = self.counters.complete_queries.saturating_add(1);
        self.counters.collected_piece_ids = self
            .counters
            .collected_piece_ids
            .saturating_add(colliding_piece_ids.len());
        Ok(GeneralHazardQuery::Complete {
            boundary,
            colliding_piece_ids,
        })
    }

    /// Returns the transformed exploration bounds for a continuous pose.
    pub fn pose_bounds(
        &mut self,
        stable_piece_id: usize,
        pose: GeneralHazardPose,
    ) -> Result<IrregularBounds, GeneralHazardError> {
        let variant = self
            .pieces
            .get_mut(stable_piece_id)
            .ok_or_else(|| GeneralHazardError::new(format!("unknown piece {stable_piece_id}")))?
            .select_mut(pose.mirrored, stable_piece_id)?;
        precise_pose_bounds(&variant.precise_points, pose)
    }

    /// Quantifies a collision already reported by [`Self::query`].
    ///
    /// The value is a continuous search signal derived from the fixed pole
    /// sets generated during preprocessing. It is not a legality result.
    pub fn collision_pressure(
        &mut self,
        moving_piece_id: usize,
        pose: GeneralHazardPose,
        fixed_piece_id: usize,
    ) -> Result<f64, GeneralHazardError> {
        if moving_piece_id == fixed_piece_id {
            return Err(GeneralHazardError::new(
                "collision pressure requires two different pieces",
            ));
        }
        let fixed_handle = self
            .handles
            .get(fixed_piece_id)
            .copied()
            .flatten()
            .ok_or_else(|| {
                GeneralHazardError::new(format!(
                    "fixed piece {fixed_piece_id} is not active in the hazard index"
                ))
            })?;
        let (layout, pieces) = (&self.layout, &mut self.pieces);
        let moving = pieces
            .get_mut(moving_piece_id)
            .ok_or_else(|| GeneralHazardError::new(format!("unknown piece {moving_piece_id}")))?
            .select_mut(pose.mirrored, moving_piece_id)?;
        let transform = transform_for_pose(moving.anchor, pose)?;
        moving
            .scratch
            .transform_from(moving.item.shape_cd.as_ref(), &transform);
        let fixed = &layout
            .placed_items
            .get(fixed_handle.placed_item_key)
            .ok_or_else(|| {
                GeneralHazardError::new(format!(
                    "piece {fixed_piece_id} references a stale placed-item handle"
                ))
            })?
            .shape;
        Ok(f64::from(overlap_pressure(&moving.scratch, fixed)))
    }

    pub fn commit(
        &mut self,
        stable_piece_id: usize,
        pose: GeneralHazardPose,
    ) -> Result<(), GeneralHazardError> {
        let variant = self
            .pieces
            .get(stable_piece_id)
            .ok_or_else(|| GeneralHazardError::new(format!("unknown piece {stable_piece_id}")))?
            .select(pose.mirrored, stable_piece_id)?;
        let transform = decomposed_transform_for_pose(variant.anchor, pose)?;
        let old_handle =
            self.handles.get(stable_piece_id).copied().ok_or_else(|| {
                GeneralHazardError::new(format!("unknown piece {stable_piece_id}"))
            })?;
        if let Some(old_handle) = old_handle {
            self.layout.remove_item(old_handle.placed_item_key);
        }
        let placed_item_key = self.layout.place_item(&variant.item, transform);
        let hazard_key = self
            .layout
            .cde()
            .haz_key_from_pi_key(placed_item_key)
            .ok_or_else(|| {
                GeneralHazardError::new(format!(
                    "piece {stable_piece_id} was placed without a hazard"
                ))
            })?;
        self.handles[stable_piece_id] = Some(CurrentHandle {
            placed_item_key,
            hazard_key,
        });
        self.counters.hazard_updates = self.counters.hazard_updates.saturating_add(1);
        Ok(())
    }

    pub fn rebuild(
        &mut self,
        strip_depth_mm: f64,
        poses: &[GeneralHazardPose],
    ) -> Result<(), GeneralHazardError> {
        if poses.len() != self.pieces.len() {
            return Err(GeneralHazardError::new(format!(
                "hazard index rebuild received {} poses for {} pieces",
                poses.len(),
                self.pieces.len()
            )));
        }
        self.layout = build_layout(
            self.sheet_short_axis_mm,
            strip_depth_mm,
            self.sheet_inset_mm,
            self.cde_config,
        )?;
        self.handles.fill(None);
        self.place_all(poses)?;
        self.counters.index_rebuilds = self.counters.index_rebuilds.saturating_add(1);
        Ok(())
    }

    fn place_all(&mut self, poses: &[GeneralHazardPose]) -> Result<(), GeneralHazardError> {
        self.place_active(poses, &vec![true; poses.len()])
    }

    fn place_active(
        &mut self,
        poses: &[GeneralHazardPose],
        active: &[bool],
    ) -> Result<(), GeneralHazardError> {
        for (stable_piece_id, pose) in poses.iter().copied().enumerate() {
            if !active[stable_piece_id] {
                continue;
            }
            let variant = self.pieces[stable_piece_id].select(pose.mirrored, stable_piece_id)?;
            let transform = decomposed_transform_for_pose(variant.anchor, pose)?;
            let placed_item_key = self.layout.place_item(&variant.item, transform);
            let hazard_key = self
                .layout
                .cde()
                .haz_key_from_pi_key(placed_item_key)
                .ok_or_else(|| {
                    GeneralHazardError::new(format!(
                        "piece {stable_piece_id} was placed without a hazard"
                    ))
                })?;
            self.handles[stable_piece_id] = Some(CurrentHandle {
                placed_item_key,
                hazard_key,
            });
        }
        Ok(())
    }
}

fn polygon_contract_fingerprint(polygon: &PolygonSet) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((polygon.regions().len() as u64).to_le_bytes());
    for region in polygon.regions() {
        hash_contract_ring(&mut digest, 0, region.outer.source_points());
        digest.update((region.holes.len() as u64).to_le_bytes());
        for hole in &region.holes {
            hash_contract_ring(&mut digest, 1, hole.source_points());
        }
    }
    digest.finalize().into()
}

fn hash_contract_ring(digest: &mut Sha256, role: u8, points: &[IrregularPoint]) {
    digest.update([role]);
    digest.update((points.len() as u64).to_le_bytes());
    for point in points {
        digest.update(point.x.to_bits().to_le_bytes());
        digest.update(point.y.to_bits().to_le_bytes());
    }
}

fn build_shared_variant(
    polygon: &PolygonSet,
    mirrored: bool,
    expansion_mm: f64,
    surrogate_config: SPSurrogateConfig,
) -> Result<SharedVariant, GeneralHazardError> {
    let expanded = polygon
        .transformed(0.0, mirrored, 0.0, 0.0)
        .and_then(|value| value.offset(expansion_mm))
        .map_err(|error| GeneralHazardError::new(error.to_string()))?;
    if expanded.regions().len() != 1 {
        return Err(GeneralHazardError::new(format!(
            "jagua hazard conversion requires one offset region, found {}",
            expanded.regions().len()
        )));
    }
    let region = &expanded.regions()[0];
    if !region.holes.is_empty() {
        return Err(GeneralHazardError::new(
            "jagua hazard conversion does not flatten offset holes",
        ));
    }
    let bounds = region.outer.bounds();
    let anchor = IrregularPoint::new(
        bounds.min_x + (bounds.max_x - bounds.min_x) * 0.5,
        bounds.min_y + (bounds.max_y - bounds.min_y) * 0.5,
    );
    let points = region
        .outer
        .points()
        .iter()
        .map(|point| {
            Ok(Point(
                checked_f32(point.x - anchor.x, "centered x")?,
                checked_f32(point.y - anchor.y, "centered y")?,
            ))
        })
        .collect::<Result<Vec<_>, GeneralHazardError>>()?;
    let mut shape = SPolygon::new(points).map_err(|error| {
        GeneralHazardError::new(format!("jagua rejected the converted polygon: {error}"))
    })?;
    shape
        .generate_surrogate(surrogate_config)
        .map_err(|error| {
            GeneralHazardError::new(format!("jagua surrogate generation failed: {error}"))
        })?;
    let shape_cd = Arc::new(shape.clone());
    let shape_orig = Arc::new(OriginalShape {
        shape,
        pre_transform: DTransformation::empty(),
        modify_mode: ShapeModifyMode::Inflate,
        modify_config: ShapeModifyConfig::default(),
    });
    Ok(SharedVariant {
        anchor,
        precise_points: Arc::from(region.outer.points().to_vec()),
        shape_orig,
        shape_cd,
    })
}

fn precise_pose_bounds(
    points: &[IrregularPoint],
    pose: GeneralHazardPose,
) -> Result<IrregularBounds, GeneralHazardError> {
    if points.is_empty() {
        return Err(GeneralHazardError::new(
            "hazard bounds require a non-empty polygon",
        ));
    }
    if !pose.rotation_deg.is_finite()
        || !pose.translate_short_axis.is_finite()
        || !pose.translate_long_axis.is_finite()
    {
        return Err(GeneralHazardError::new("hazard pose values must be finite"));
    }
    let (sin, cos) = pose.rotation_deg.rem_euclid(360.0).to_radians().sin_cos();
    let mut bounds = IrregularBounds::new(
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for point in points {
        let transformed_x = point.x * cos - point.y * sin + pose.translate_short_axis;
        let transformed_y = point.x * sin + point.y * cos + pose.translate_long_axis;
        let x = to_grid_mm(transformed_x).map(from_grid).ok_or_else(|| {
            GeneralHazardError::new("hazard x coordinate is outside the contractual grid")
        })?;
        let y = to_grid_mm(transformed_y).map(from_grid).ok_or_else(|| {
            GeneralHazardError::new("hazard y coordinate is outside the contractual grid")
        })?;
        bounds.min_x = bounds.min_x.min(x);
        bounds.min_y = bounds.min_y.min(y);
        bounds.max_x = bounds.max_x.max(x);
        bounds.max_y = bounds.max_y.max(y);
    }
    Ok(IrregularBounds::new(
        bounds.min_x - CANONICAL_TRANSFORM_OFFSET_GUARD_MM,
        bounds.min_y - CANONICAL_TRANSFORM_OFFSET_GUARD_MM,
        bounds.max_x + CANONICAL_TRANSFORM_OFFSET_GUARD_MM,
        bounds.max_y + CANONICAL_TRANSFORM_OFFSET_GUARD_MM,
    ))
}

fn build_layout(
    sheet_short_axis_mm: f64,
    strip_depth_mm: f64,
    inset_mm: f64,
    cde_config: CDEConfig,
) -> Result<Layout, GeneralHazardError> {
    let min_x = inset_mm;
    let min_y = inset_mm;
    let max_x = sheet_short_axis_mm - inset_mm;
    let max_y = strip_depth_mm - inset_mm;
    if ![min_x, min_y, max_x, max_y]
        .iter()
        .all(|value| value.is_finite())
        || min_x >= max_x
        || min_y >= max_y
    {
        return Err(GeneralHazardError::new(
            "hazard container has invalid inset bounds",
        ));
    }
    let outer = SPolygon::new(vec![
        Point(
            checked_f32(min_x, "container min x")?,
            checked_f32(min_y, "container min y")?,
        ),
        Point(
            checked_f32(max_x, "container max x")?,
            checked_f32(min_y, "container min y")?,
        ),
        Point(
            checked_f32(max_x, "container max x")?,
            checked_f32(max_y, "container max y")?,
        ),
        Point(
            checked_f32(min_x, "container min x")?,
            checked_f32(max_y, "container max y")?,
        ),
    ])
    .map_err(|error| GeneralHazardError::new(format!("invalid hazard container: {error}")))?;
    let original_outer = Arc::new(OriginalShape {
        shape: outer.clone(),
        pre_transform: DTransformation::empty(),
        modify_mode: ShapeModifyMode::Deflate,
        modify_config: ShapeModifyConfig::default(),
    });
    let outer = Arc::new(outer);
    let root_margin_mm = sheet_short_axis_mm.max(strip_depth_mm);
    let root = Rect::try_new(
        checked_f32(min_x - root_margin_mm, "query envelope min x")?,
        checked_f32(min_y - root_margin_mm, "query envelope min y")?,
        checked_f32(max_x + root_margin_mm, "query envelope max x")?,
        checked_f32(max_y + root_margin_mm, "query envelope max y")?,
    )
    .map_err(|error| GeneralHazardError::new(format!("invalid query envelope: {error}")))?;
    let base_cde = Arc::new(CDEngine::new(
        root,
        vec![Hazard::new(HazardEntity::Exterior, outer.clone(), false)],
        cde_config,
    ));
    let container = Container {
        id: 0,
        outer_orig: original_outer,
        outer_cd: outer,
        quality_zones: Default::default(),
        base_cde,
    };
    Ok(Layout::new(container))
}

fn transform_for_pose(
    anchor: IrregularPoint,
    pose: GeneralHazardPose,
) -> Result<Transformation, GeneralHazardError> {
    Ok(decomposed_transform_for_pose(anchor, pose)?.compose())
}

fn decomposed_transform_for_pose(
    anchor: IrregularPoint,
    pose: GeneralHazardPose,
) -> Result<DTransformation, GeneralHazardError> {
    if !pose.rotation_deg.is_finite()
        || !pose.translate_short_axis.is_finite()
        || !pose.translate_long_axis.is_finite()
    {
        return Err(GeneralHazardError::new("hazard pose values must be finite"));
    }
    let normalized_deg = pose.rotation_deg.rem_euclid(360.0);
    let rotation = checked_f32(normalized_deg.to_radians(), "rotation")?;
    let anchor_x = checked_f32(anchor.x, "anchor x")?;
    let anchor_y = checked_f32(anchor.y, "anchor y")?;
    let translate_x = checked_f32(pose.translate_short_axis, "translation x")?;
    let translate_y = checked_f32(pose.translate_long_axis, "translation y")?;
    let (sin, cos) = rotation.sin_cos();
    let adjusted_x = translate_x + anchor_x * cos - anchor_y * sin;
    let adjusted_y = translate_y + anchor_x * sin + anchor_y * cos;
    if !adjusted_x.is_finite() || !adjusted_y.is_finite() {
        return Err(GeneralHazardError::new(
            "anchor-adjusted hazard translation is not finite",
        ));
    }
    Ok(DTransformation::new(rotation, (adjusted_x, adjusted_y)))
}

fn checked_f32(value: f64, label: &str) -> Result<f32, GeneralHazardError> {
    let converted = value as f32;
    if !value.is_finite() || !converted.is_finite() {
        return Err(GeneralHazardError::new(format!(
            "{label} cannot be represented by the hazard backend"
        )));
    }
    Ok(converted)
}

fn default_surrogate_config() -> SPSurrogateConfig {
    SPSurrogateConfig {
        n_pole_limits: [(64, 0.0), (16, 0.8), (8, 0.9)],
        n_ff_poles: 1,
        n_ff_piers: 0,
    }
}

fn overlap_pressure(first: &SPolygon, second: &SPolygon) -> f32 {
    let epsilon = first.diameter.max(second.diameter) * OVERLAP_PROXY_EPSILON_DIAMETER_RATIO;
    let mut proxy = epsilon * epsilon;
    for first_pole in &first.surrogate().poles {
        for second_pole in &second.surrogate().poles {
            let dx = first_pole.center.0 - second_pole.center.0;
            let dy = first_pole.center.1 - second_pole.center.1;
            let penetration = first_pole.radius + second_pole.radius - dx.hypot(dy);
            let decayed = if penetration >= epsilon {
                penetration
            } else {
                epsilon * epsilon / (-penetration + 2.0 * epsilon)
            };
            proxy += std::f32::consts::PI * decayed * first_pole.radius.min(second_pole.radius);
        }
    }
    let first_penalty = first.surrogate().convex_hull_area.sqrt();
    let second_penalty = second.surrogate().convex_hull_area.sqrt();
    proxy.sqrt() * (first_penalty * second_penalty).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variant_geometry_fingerprint(variant: &PieceVariant) -> [u8; 32] {
        fn update_f32(hasher: &mut Sha256, value: f32) {
            hasher.update(value.to_bits().to_le_bytes());
        }

        fn update_polygon(hasher: &mut Sha256, polygon: &SPolygon) {
            hasher.update(polygon.vertices.len().to_le_bytes());
            for point in &polygon.vertices {
                update_f32(hasher, point.0);
                update_f32(hasher, point.1);
            }
            for value in [
                polygon.bbox.x_min,
                polygon.bbox.y_min,
                polygon.bbox.x_max,
                polygon.bbox.y_max,
                polygon.area,
                polygon.diameter,
                polygon.poi.center.0,
                polygon.poi.center.1,
                polygon.poi.radius,
            ] {
                update_f32(hasher, value);
            }
            match polygon.surrogate.as_ref() {
                Some(surrogate) => {
                    hasher.update([1]);
                    hasher.update(surrogate.poles.len().to_le_bytes());
                    for pole in &surrogate.poles {
                        update_f32(hasher, pole.center.0);
                        update_f32(hasher, pole.center.1);
                        update_f32(hasher, pole.radius);
                    }
                    hasher.update(surrogate.piers.len().to_le_bytes());
                    for pier in &surrogate.piers {
                        for point in [pier.start, pier.end] {
                            update_f32(hasher, point.0);
                            update_f32(hasher, point.1);
                        }
                    }
                    hasher.update(surrogate.convex_hull_indices.len().to_le_bytes());
                    for index in &surrogate.convex_hull_indices {
                        hasher.update(index.to_le_bytes());
                    }
                    update_f32(hasher, surrogate.convex_hull_area);
                    for (limit, coverage) in surrogate.config.n_pole_limits {
                        hasher.update(limit.to_le_bytes());
                        update_f32(hasher, coverage);
                    }
                    hasher.update(surrogate.config.n_ff_poles.to_le_bytes());
                    hasher.update(surrogate.config.n_ff_piers.to_le_bytes());
                }
                None => hasher.update([0]),
            }
        }

        let mut hasher = Sha256::new();
        update_polygon(&mut hasher, &variant.item.shape_orig.shape);
        update_polygon(&mut hasher, &variant.item.shape_cd);
        hasher.finalize().into()
    }

    fn concave_piece() -> PolygonSet {
        PolygonSet::from_outer(vec![
            IrregularPoint::new(0.0, 0.0),
            IrregularPoint::new(30.0, 0.0),
            IrregularPoint::new(30.0, 10.0),
            IrregularPoint::new(10.0, 10.0),
            IrregularPoint::new(10.0, 30.0),
            IrregularPoint::new(0.0, 30.0),
        ])
        .expect("concave test polygon")
    }

    fn square_piece() -> PolygonSet {
        PolygonSet::from_outer(vec![
            IrregularPoint::new(0.0, 0.0),
            IrregularPoint::new(12.0, 0.0),
            IrregularPoint::new(12.0, 12.0),
            IrregularPoint::new(0.0, 12.0),
        ])
        .expect("square test polygon")
    }

    fn pose(x: f64, y: f64) -> GeneralHazardPose {
        GeneralHazardPose {
            rotation_deg: 0.0,
            mirrored: false,
            translate_short_axis: x,
            translate_long_axis: y,
        }
    }

    #[test]
    fn continuous_angle_query_reports_stable_collision_ids() {
        let concave = concave_piece();
        let square = square_piece();
        let pieces = [
            GeneralFastPiece {
                id: "concave",
                polygon: &concave,
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "square",
                polygon: &square,
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let poses = [pose(20.0, 20.0), pose(70.0, 70.0)];
        let mut index =
            JaguaHazardIndex::new(&pieces, settings, 100.0, &poses).expect("build hazard index");

        let query = index
            .query(
                1,
                GeneralHazardPose {
                    rotation_deg: 21.375,
                    mirrored: true,
                    translate_short_axis: 22.0,
                    translate_long_axis: 22.0,
                },
                None,
            )
            .expect("query continuous pose");
        assert_eq!(
            query,
            GeneralHazardQuery::Complete {
                boundary: false,
                colliding_piece_ids: vec![0],
            }
        );
    }

    #[test]
    fn partial_index_excludes_inactive_hazards_and_restores_them_by_stable_id() {
        let polygons = [square_piece(), square_piece(), square_piece()];
        let pieces = [
            GeneralFastPiece {
                id: "fixed",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "moving",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "inactive",
                polygon: &polygons[2],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let poses = [pose(20.0, 20.0), pose(22.0, 22.0), pose(20.0, 20.0)];
        let catalog = JaguaHazardCatalog::new(&pieces, settings).expect("build catalog");
        let mut index = JaguaHazardIndex::from_catalog_active(
            &pieces,
            settings,
            100.0,
            &poses,
            &[true, false, false],
            &catalog,
        )
        .expect("build partial index");

        assert_eq!(
            index.query_unplaced(1, pose(22.0, 22.0)).unwrap(),
            GeneralHazardQuery::Complete {
                boundary: false,
                colliding_piece_ids: vec![0],
            }
        );
        assert!(index.collision_pressure(1, pose(22.0, 22.0), 0).unwrap() > 0.0);
        index.commit(1, pose(22.0, 22.0)).unwrap();
        assert_eq!(
            index.query_unplaced(2, pose(20.0, 20.0)).unwrap(),
            GeneralHazardQuery::Complete {
                boundary: false,
                colliding_piece_ids: vec![0, 1],
            }
        );
    }

    #[test]
    fn commit_replaces_handles_without_changing_stable_identity() {
        let concave = concave_piece();
        let square = square_piece();
        let pieces = [
            GeneralFastPiece {
                id: "concave",
                polygon: &concave,
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "square",
                polygon: &square,
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let poses = [pose(20.0, 20.0), pose(70.0, 70.0)];
        let mut index =
            JaguaHazardIndex::new(&pieces, settings, 100.0, &poses).expect("build hazard index");
        let committed = GeneralHazardPose {
            rotation_deg: 17.125,
            mirrored: true,
            translate_short_axis: 24.0,
            translate_long_axis: 24.0,
        };
        index.commit(1, committed).expect("commit hazard pose");

        let query = index
            .query(0, pose(20.0, 20.0), None)
            .expect("query after handle replacement");
        assert_eq!(
            query,
            GeneralHazardQuery::Complete {
                boundary: false,
                colliding_piece_ids: vec![1],
            }
        );
        assert_eq!(index.counters().hazard_updates, 1);
    }

    #[test]
    fn rebuild_recreates_boundary_and_piece_handles() {
        let concave = concave_piece();
        let square = square_piece();
        let pieces = [
            GeneralFastPiece {
                id: "concave",
                polygon: &concave,
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "square",
                polygon: &square,
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let poses = [pose(20.0, 20.0), pose(70.0, 70.0)];
        let mut index =
            JaguaHazardIndex::new(&pieces, settings, 100.0, &poses).expect("build hazard index");
        index.rebuild(80.0, &poses).expect("rebuild index");

        let outside = index
            .query(1, pose(70.0, 75.0), None)
            .expect("query rebuilt boundary");
        assert!(matches!(
            outside,
            GeneralHazardQuery::Complete { boundary: true, .. }
        ));
        assert_eq!(index.counters().index_rebuilds, 1);
    }

    #[test]
    fn catalog_shares_immutable_shapes_between_isolated_indexes() {
        let concave = concave_piece();
        let square = square_piece();
        let pieces = [
            GeneralFastPiece {
                id: "concave",
                polygon: &concave,
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "square",
                polygon: &square,
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let poses = [pose(20.0, 20.0), pose(70.0, 70.0)];
        let catalog = JaguaHazardCatalog::new(&pieces, settings).expect("build catalog");
        let mut first = JaguaHazardIndex::from_catalog(&pieces, settings, 100.0, &poses, &catalog)
            .expect("build first index");
        let mut second = JaguaHazardIndex::from_catalog(&pieces, settings, 100.0, &poses, &catalog)
            .expect("build second index");

        assert!(Arc::ptr_eq(
            &first.pieces[0].original.item.shape_cd,
            &second.pieces[0].original.item.shape_cd,
        ));
        assert!(Arc::ptr_eq(
            &first.pieces[0].original.item.shape_orig,
            &second.pieces[0].original.item.shape_orig,
        ));
        assert_ne!(
            first.pieces[0].original.scratch.vertices.as_ptr(),
            second.pieces[0].original.scratch.vertices.as_ptr(),
        );
        let query_pose = GeneralHazardPose {
            rotation_deg: 21.375,
            mirrored: false,
            translate_short_axis: 22.0,
            translate_long_axis: 22.0,
        };
        assert_eq!(
            first.query(0, query_pose, None).unwrap(),
            second.query(0, query_pose, None).unwrap(),
        );

        let second_counters = second.counters();
        let second_geometry = variant_geometry_fingerprint(&second.pieces[0].original);
        first.commit(0, pose(40.0, 40.0)).unwrap();
        first.rebuild(85.0, &poses).unwrap();
        let _ = first.query(1, pose(20.0, 20.0), None).unwrap();
        assert_eq!(second.counters(), second_counters);
        assert_eq!(
            variant_geometry_fingerprint(&second.pieces[0].original),
            second_geometry
        );

        let mut baseline = JaguaHazardIndex::new(&pieces, settings, 100.0, &poses)
            .expect("build independent baseline index");
        assert_eq!(
            variant_geometry_fingerprint(&second.pieces[0].original),
            variant_geometry_fingerprint(&baseline.pieces[0].original)
        );
        assert_eq!(
            second.query(0, query_pose, None).unwrap(),
            baseline.query(0, query_pose, None).unwrap(),
        );
        assert_ne!(first.counters(), second.counters());
    }

    #[test]
    fn catalog_rejects_same_length_contract_mismatches() {
        let concave = concave_piece();
        let square = square_piece();
        let pieces = [
            GeneralFastPiece {
                id: "concave",
                polygon: &concave,
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "square",
                polygon: &square,
                allow_rotation: true,
                allow_mirror: false,
            },
        ];
        let settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let poses = [pose(20.0, 20.0), pose(70.0, 70.0)];
        let catalog = JaguaHazardCatalog::new(&pieces, settings).expect("build catalog");

        let reordered = [pieces[1], pieces[0]];
        assert!(
            JaguaHazardIndex::from_catalog(&reordered, settings, 100.0, &poses, &catalog)
                .err()
                .expect("reject reordered pieces")
                .message()
                .contains("piece 0")
        );

        let mut changed_permissions = pieces;
        changed_permissions[1].allow_rotation = false;
        assert!(JaguaHazardIndex::from_catalog(
            &changed_permissions,
            settings,
            100.0,
            &poses,
            &catalog,
        )
        .err()
        .expect("reject changed permissions")
        .message()
        .contains("piece 1"));

        let mut changed_settings = settings;
        changed_settings.total_padding_mm += 2.0;
        assert!(
            JaguaHazardIndex::from_catalog(&pieces, changed_settings, 100.0, &poses, &catalog,)
                .err()
                .expect("reject changed settings")
                .message()
                .contains("collision expansion")
        );

        assert!(
            JaguaHazardIndex::from_catalog(&pieces, settings, 100.0, &poses[..1], &catalog,)
                .err()
                .expect("reject changed pose count")
                .message()
                .contains("1 poses")
        );
    }

    #[test]
    fn complete_query_keeps_piece_collisions_outside_the_sheet() {
        let first = square_piece();
        let second = square_piece();
        let pieces = [
            GeneralFastPiece {
                id: "first",
                polygon: &first,
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "second",
                polygon: &second,
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let poses = [pose(-6.0, 20.0), pose(70.0, 70.0)];
        let mut index =
            JaguaHazardIndex::new(&pieces, settings, 100.0, &poses).expect("build hazard index");

        let outside = index
            .query(1, pose(-2.0, 20.0), None)
            .expect("query outside sheet");
        assert_eq!(
            outside,
            GeneralHazardQuery::Complete {
                boundary: true,
                colliding_piece_ids: vec![0],
            }
        );
    }

    #[test]
    fn continuous_pose_bounds_and_collision_pressure_are_finite() {
        let first = square_piece();
        let second = square_piece();
        let pieces = [
            GeneralFastPiece {
                id: "first",
                polygon: &first,
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "second",
                polygon: &second,
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let poses = [pose(20.0, 20.0), pose(70.0, 70.0)];
        let mut index =
            JaguaHazardIndex::new(&pieces, settings, 100.0, &poses).expect("build hazard index");
        let candidate = GeneralHazardPose {
            rotation_deg: 17.125,
            mirrored: true,
            translate_short_axis: 22.0,
            translate_long_axis: 22.0,
        };

        let bounds = index.pose_bounds(1, candidate).expect("pose bounds");
        assert!(bounds.max_x > bounds.min_x);
        assert!(bounds.max_y > bounds.min_y);
        let pressure = index
            .collision_pressure(1, candidate, 0)
            .expect("collision pressure");
        assert!(pressure.is_finite() && pressure > 0.0);
    }

    /// Collision pressure is *mathematically* symmetric but not *bitwise*
    /// symmetric, and the difference is load-bearing for the coupled
    /// separator.
    ///
    /// [`JaguaHazardIndex::collision_pressure`] evaluates a freshly transformed
    /// scratch polygon for the moving piece against the committed layout shape
    /// of the fixed one. Swapping the roles swaps which side comes from which
    /// pipeline and reverses the order of the `f32` pole-pair summation in
    /// `overlap_pressure`, so the two readings of one pair can differ in their
    /// low bits.
    ///
    /// That matters because the coupled separator's incremental tracker records
    /// whichever reading the *last moved* piece produced, while a full rescore
    /// always reads a pair from its lower-indexed piece. When a rollback
    /// compares the two with exact equality, a pair last updated from its
    /// higher-indexed side can disagree purely through this asymmetry, and the
    /// target aborts with "rollback tracker disagrees with a complete rescore:
    /// collision rows differ".
    ///
    /// This test pins the *property*, not a particular discrepancy: the two
    /// readings must agree to within a loose relative tolerance (they measure
    /// the same thing) while exact equality is not something the separator may
    /// assume.
    #[test]
    fn collision_pressure_is_direction_dependent_in_its_low_bits() {
        let first = concave_piece();
        let second = square_piece();
        let pieces = [
            GeneralFastPiece {
                id: "first",
                polygon: &first,
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "second",
                polygon: &second,
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        // Overlapping poses, so both directions report a positive pressure.
        let poses = [pose(20.0, 20.0), pose(26.0, 26.0)];
        let mut index =
            JaguaHazardIndex::new(&pieces, settings, 100.0, &poses).expect("build hazard index");

        let forward = index
            .collision_pressure(0, poses[0], 1)
            .expect("forward collision pressure");
        let reverse = index
            .collision_pressure(1, poses[1], 0)
            .expect("reverse collision pressure");
        assert!(forward > 0.0 && reverse > 0.0);
        // Same quantity, so they agree to well within the f32 noise floor.
        assert!(
            (forward - reverse).abs() <= 1e-3 * forward.max(reverse),
            "forward {forward} and reverse {reverse} should measure the same overlap"
        );
    }
}
