//! Bounded overlap-relaxation search for the opt-in general engine.
//!
//! The constructor remains the protected anytime incumbent. This module
//! searches complete, temporarily infeasible layouts with a cheap convex-cell
//! surrogate and can replace that incumbent only after both publication gates
//! in `general_fast` accept a strict improvement.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::canonical_grid::{from_grid, to_grid_mm};
use crate::clipper::core::PointInPolygonResult;
use crate::domain::{IrregularBounds, IrregularPoint};
use crate::geometry::convex::bounds_for_points;
use crate::geometry::general_polygon::{GeneralPolygonError, PolygonSet};
use crate::geometry::predicates::orientation;
use crate::nfp_ifp::compute_relative_nfp_boundary_reference;
use crate::parallel::map_slice_with_job_pool;
use crate::search::general_fast::{
    collision_expansion_mm, collision_sheet_inset_mm, collision_sheet_short_axis_mm,
    polygons_overlap_exact, validate_and_measure_placements, GeneralFastError, GeneralFastPiece,
    GeneralFastPlacement, GeneralFastResult, GeneralFastSettings, GeneralPlacementMetrics,
};

#[cfg(feature = "jagua-experimental")]
#[path = "general_persistent_vacancy.rs"]
mod persistent_vacancy;

#[cfg(feature = "jagua-experimental")]
use crate::search::general_hazard::{
    GeneralHazardPose, GeneralHazardQuery, JaguaHazardCatalog, JaguaHazardIndex,
};
const ANGLE_KEY_SCALE: f64 = 1_000_000.0;
const SURROGATE_ANGLE_STEP_DEG: f64 = 2.5;
const MAX_CELLS_PER_PIECE: usize = 512;
const MAX_CELLS_PER_JOB: usize = 524_288;
const CELL_INDEX_SIDE: usize = 8;
const PIECE_INDEX_SIDE: usize = 16;
const LOCAL_DESCENT_STARTS: usize = 3;
const UNIQUE_SAMPLE_POSITION_RATIO: f64 = 0.05;
const UNIQUE_SAMPLE_ANGLE_DEG: f64 = 1.0;
const OVERLAP_PROXY_EPSILON_DIAMETER_RATIO: f64 = 0.01;
const EJECTION_CHAIN_MAX_DONORS: usize = 4;
const EJECTION_CHAIN_DIVERSITY: usize = 3;
const ENABLE_EJECTION_CHAIN: bool = false;
const PRE_REFINEMENT_INITIAL_RATIO: f64 = 0.25;
const PRE_REFINEMENT_LIMIT_RATIO: f64 = 0.02;
const FINAL_REFINEMENT_INITIAL_RATIO: f64 = 0.01;
const FINAL_REFINEMENT_LIMIT_RATIO: f64 = 0.001;
const REFINEMENT_SUCCESS_MULTIPLIER: f64 = 1.1;
const REFINEMENT_FAILURE_MULTIPLIER: f64 = 0.5;
const MAX_NFP_COMPONENTS_PER_MOVE: usize = 4_096;
const MAX_AXIS_EVENTS_PER_MOVE: usize = 16_384;
const MAX_LANE_NFP_COMPONENTS: usize = 65_536;
const MAX_SHARED_NFP_COMPONENTS: usize = MAX_LANE_NFP_COMPONENTS;
const MAX_SHARED_NFP_ESTIMATED_BYTES: usize = 32 * 1024 * 1024;
const MAX_TRIANGLE_NFP_POINTS: usize = 6;
const AXIS_MINIMIZATION_PASSES: usize = 4;
const AXIS_RETAINED_CANDIDATES: usize = 4;
const ENABLE_NFP_AXIS_MINIMIZER: bool = false;
const DIRECTIONAL_LANE_UNSCORABLE: &str = "directional penetration lane is unscorable";
const COUPLED_SEPARATOR_SEED_DOMAIN: u64 = 0x4350_4C44_5350_5231;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_SEED_DOMAIN: u64 = 0x4352_5549_4E5F_5231;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_ANGLE_SEED_DOMAIN: u64 = 0x4352_5549_4E5F_4131;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_POSITION_SEED_DOMAIN: u64 = 0x4352_5549_4E5F_5031;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_DIVERSITY_SEED_DOMAIN: u64 = 0x4352_5549_4E5F_4431;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_RETRY_SEED_DOMAIN: u64 = 0x4352_5549_4E5F_5331;
#[cfg(feature = "jagua-experimental")]
const PRECOMPRESSION_FRONTIER_SEED_DOMAIN: u64 = 0x5052_4543_4F4D_5031;
#[cfg(feature = "jagua-experimental")]
const PRECOMPRESSION_HANDOFF_SEED_DOMAIN: u64 = 0x5052_4548_414E_4431;
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_TARGETS: usize = 32;
const COUPLED_SEPARATOR_WORKERS: usize = 8;
const COUPLED_SEPARATOR_ROUNDS: usize = 40;
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_NO_IMPROVEMENT_LIMIT: usize = 10;
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_STRIKE_LIMIT: usize = 3;
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_CONTRACTION_RATIO: f64 = 0.001;
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_SUBSTANTIAL_RATIO: f64 = 0.98;
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_WORKER_QUERY_CAP: usize = 420_000;
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_WORKER_PRESSURE_CAP: usize = 4_000_000;
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_WORKER_CONFIRMATION_CAP: usize = 2_440;
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_WORKER_UPDATE_CAP: usize = 2_440;
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_WORKER_LAYOUT_LOAD_CAP: usize = 40;
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_WORKER_FULL_SCORE_PAIR_VISIT_CAP: usize = 73_200;
#[cfg(feature = "jagua-experimental")]
const COUPLED_SEPARATOR_AUDITOR_FULL_SCORES: usize = 5;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_REMOVED_PIECES: usize = 3;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_BEAM_WIDTH: usize = 4;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_ORIENTATIONS_PER_PARENT: usize = 12;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_POSES_PER_STREAM: usize = 64;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_FINALISTS_PER_STREAM: usize = 4;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_STREAM_CAP: usize = 108;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_QUERY_CAP: usize = 6_912;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_FINALIST_CAP: usize = 432;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_TRANSFORMED_VERTEX_CAP: usize = 262_144;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_FEATURE_VISIT_CAP: usize = 131_072;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_CONTACT_ATTEMPT_CAP: usize = 131_072;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_PROPOSAL_CAP: usize = 32_768;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_CLIPPER_INPUT_VERTEX_CAP: usize = 8_000_000;
#[cfg(feature = "jagua-experimental")]
const CONFLICT_RUIN_CLIPPER_OUTPUT_VERTEX_CAP: usize = 2_000_000;
#[cfg(feature = "jagua-experimental")]
const PRECOMPRESSION_FRONTIER_FULL_SCORE_CAP: usize = 9;
#[cfg(feature = "jagua-experimental")]
const PRECOMPRESSION_FRONTIER_PAIR_VISIT_CAP: usize = 16_470;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneralRelaxedCollisionBackend {
    RollbackTriangle,
    DynamicHazard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneralRelaxedAngleSeedPolicy {
    CurrentOnly,
    StructuredGrid,
    ContinuousUniform,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneralRelaxedPressureModel {
    StructuredTrianglePoles,
    DirectionalPenetration,
    ContinuousTrianglePoles,
    DynamicPoles,
}

#[derive(Clone, Copy, Debug)]
pub struct GeneralAngularRepairSettings {
    pub neighborhood_size: usize,
    pub successors: usize,
    pub complete_query_budget: usize,
    pub retained_confirmation_budget: usize,
    pub early_stop_queries: usize,
}

impl GeneralAngularRepairSettings {
    pub const fn disabled() -> Self {
        Self {
            neighborhood_size: 0,
            successors: 0,
            complete_query_budget: 0,
            retained_confirmation_budget: 0,
            early_stop_queries: 0,
        }
    }

    pub const fn bounded_probe() -> Self {
        Self {
            neighborhood_size: 10,
            successors: 1,
            complete_query_budget: 2_048,
            retained_confirmation_budget: 64,
            early_stop_queries: 512,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GeneralRelaxedSettings {
    pub seed: u64,
    pub epochs: usize,
    pub lanes: usize,
    pub sweeps_per_epoch: usize,
    pub global_samples_per_move: usize,
    pub focused_samples_per_move: usize,
    pub refinement_rounds: usize,
    pub initial_shrink_ratio: f64,
    pub minimum_shrink_ratio: f64,
    pub synchronize_lanes: bool,
    pub collision_backend: GeneralRelaxedCollisionBackend,
    pub angle_seed_policy: GeneralRelaxedAngleSeedPolicy,
    pub pressure_model: GeneralRelaxedPressureModel,
    pub angular_repair: GeneralAngularRepairSettings,
    pub coupled_dynamic_separator: bool,
    pub precompression_frontier_vacancy_mode: usize,
    pub persistent_vacancy_mode: usize,
    pub persistent_vacancy_target_depth_mm: Option<f64>,
}

impl GeneralRelaxedSettings {
    pub fn mixed_61_probe(seed: u64, lanes: usize) -> Self {
        Self {
            seed,
            epochs: 12,
            lanes: lanes.max(1),
            sweeps_per_epoch: 12,
            global_samples_per_move: 36,
            focused_samples_per_move: 36,
            refinement_rounds: 3,
            initial_shrink_ratio: 0.02,
            minimum_shrink_ratio: 0.001,
            synchronize_lanes: false,
            collision_backend: GeneralRelaxedCollisionBackend::RollbackTriangle,
            angle_seed_policy: GeneralRelaxedAngleSeedPolicy::StructuredGrid,
            pressure_model: GeneralRelaxedPressureModel::StructuredTrianglePoles,
            angular_repair: GeneralAngularRepairSettings::disabled(),
            coupled_dynamic_separator: false,
            precompression_frontier_vacancy_mode: 0,
            persistent_vacancy_mode: 0,
            persistent_vacancy_target_depth_mm: None,
        }
    }

    pub fn mixed_61_dynamic_hazard_probe(seed: u64, lanes: usize) -> Self {
        Self {
            collision_backend: GeneralRelaxedCollisionBackend::DynamicHazard,
            angular_repair: GeneralAngularRepairSettings::disabled(),
            ..Self::mixed_61_probe(seed, lanes)
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralRelaxedDiagnostics {
    pub epochs_attempted: usize,
    pub epochs_improved: usize,
    pub oriented_surrogate_builds: usize,
    pub generated_cells: usize,
    pub ejection_chain_evaluations: usize,
    pub ejection_chain_accepts: usize,
    pub surrogate_evaluations: usize,
    pub piece_broad_phase_probes: usize,
    pub cell_index_probes: usize,
    pub sat_tests: usize,
    pub pair_nfp_builds: usize,
    pub pair_nfp_components: usize,
    pub shared_pair_nfp_entries: usize,
    pub shared_pair_nfp_components: usize,
    pub shared_pair_nfp_estimated_bytes: usize,
    pub shared_pair_nfp_adoptions: usize,
    pub directional_pair_evaluations: usize,
    pub directional_exact_confirmations: usize,
    pub directional_cache_hits: usize,
    pub directional_cache_misses: usize,
    pub directional_component_visits: usize,
    pub directional_intervals_produced: usize,
    pub directional_intervals_merged: usize,
    pub directional_over_budget_candidates: usize,
    pub directional_zero_penetration_inconsistencies: usize,
    pub directional_lane_rejections: usize,
    pub directional_relocations: usize,
    pub directional_rejected_contractions: usize,
    pub directional_containment_rejections: usize,
    pub directional_initial_pair_loss: GeneralRelaxedLossDistribution,
    pub directional_initial_boundary_loss: GeneralRelaxedLossDistribution,
    pub directional_accepted_pair_loss: GeneralRelaxedLossDistribution,
    pub directional_accepted_boundary_loss: GeneralRelaxedLossDistribution,
    pub axis_events: usize,
    pub axis_candidate_evaluations: usize,
    pub dynamic_hazard_queries: usize,
    pub dynamic_hazard_updates: usize,
    pub dynamic_pressure_evaluations: usize,
    pub translation_evaluations: usize,
    pub rotation_evaluations: usize,
    pub retained_f64_confirmations: usize,
    pub confirmed_pair_additions: usize,
    pub confirmed_pair_removals: usize,
    pub accepted_moves: usize,
    pub angular_repair_successors: usize,
    pub angular_repair_improvements: usize,
    pub angular_repair_queries: usize,
    pub angular_repair_base_loss: Option<f64>,
    pub angular_repair_control_loss: Option<f64>,
    pub angular_repair_rotation_loss: Option<f64>,
    pub surrogate_feasible_states: usize,
    pub exact_rejected_states: usize,
    pub exact_valid_non_improvements: usize,
    pub exact_rejection_reasons: Vec<String>,
    pub skipped_reason: Option<String>,
    pub epochs: Vec<GeneralRelaxedEpochDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coupled_dynamic_separator: Option<GeneralCoupledSeparatorDiagnostics>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralCoupledSeparatorDiagnostics {
    pub seed_domain: u64,
    pub control: GeneralCoupledSeparatorArmDiagnostics,
    pub treatment: GeneralCoupledSeparatorArmDiagnostics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boundary_projection_treatment: Option<GeneralCoupledSeparatorArmDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict_ruin_recreate: Option<GeneralConflictRuinDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub precompression_frontier_vacancy: Option<GeneralPrecompressionFrontierVacancyDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistent_vacancy_population: Option<GeneralPersistentVacancyDiagnostics>,
}

/// A pinned persistent-vacancy parent layout loaded from a committed fixture.
///
/// The frozen `b9335a72...` parent is a fingerprint of the boundary-projection
/// trajectory on the canonical Apple M4 Max platform. Arbitrary-angle
/// trigonometry is not promised byte-identical across numeric platforms, so a
/// different machine cannot reproduce that parent in-run. The fixture supplies
/// the identical placements explicitly; every frozen fingerprint, depth, and
/// dual-validation check still runs against the compiled-in constants.
#[derive(Clone, Debug)]
pub struct GeneralPersistentVacancyPinnedParent {
    pub placements: Vec<GeneralFastPlacement>,
    pub source: String,
    pub source_sha256: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyDiagnostics {
    pub mode: usize,
    pub attempted: bool,
    pub seed_domain: u64,
    pub target_depth_mm: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_source: Option<String>,
    pub parent_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_independent_depth_mm: Option<f64>,
    pub initial_state_fingerprint: Option<String>,
    pub initial_active_piece_ids: Vec<String>,
    pub initial_inactive_piece_ids: Vec<String>,
    pub initial_inactive_order_hash: Option<String>,
    pub layers_completed: usize,
    pub direct_insertions: usize,
    pub ejection_insertions: usize,
    pub immediate_reversals_rejected: usize,
    pub deduplicated_states: usize,
    pub distinct_signatures_retained: usize,
    pub complete_states: usize,
    pub publication_rejections: usize,
    pub exact_valid: bool,
    pub independent_depth_mm: Option<f64>,
    pub final_placement_fingerprint: Option<String>,
    pub final_placements: Vec<GeneralCoupledSeparatorPlacementDiagnostics>,
    pub work: GeneralPersistentVacancyWorkDiagnostics,
    pub layers: Vec<GeneralPersistentVacancyLayerDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settle: Option<GeneralPersistentVacancySettleDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconstruction: Option<GeneralPersistentVacancyReconstructionDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub construction: Option<GeneralPersistentVacancyConstructionDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_drop: Option<GeneralPersistentVacancyGroupDropDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lns: Option<GeneralPersistentVacancyLnsDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontier_feasibility: Option<Vec<GeneralPersistentVacancyFeasibilityRow>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive: Option<GeneralPersistentVacancyArchiveDiagnostics>,
    pub cap_exhausted: Option<String>,
    pub failure_reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyReconstructionDiagnostics {
    pub insertions: usize,
    pub exact_rows: usize,
    pub rows_per_piece_cap: usize,
    pub deferred_first_pass: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_piece_id: Option<String>,
    pub failed_piece_count: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyConstructionDiagnostics {
    pub restarts: usize,
    pub beam_width: usize,
    pub hint_stations_per_slot: usize,
    pub rows_per_piece_cap: usize,
    pub finalists_per_slot: usize,
    pub slots: usize,
    pub exact_rows: usize,
    pub children_generated: usize,
    pub children_deduplicated: usize,
    pub shelf_finalists: usize,
    pub void_scans: usize,
    pub fixture_prior_finalists: usize,
    pub zero_prior_finalists: usize,
    pub complete_candidates: usize,
    pub audited_candidates: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_restart_ordinal: Option<usize>,
    pub restart_rows: Vec<GeneralPersistentVacancyConstructionRestartRow>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyConstructionRestartRow {
    pub order: String,
    pub complete: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontier_grid: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trapped_void_cells: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub independent_depth_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyFeasibilityRow {
    pub piece_id: String,
    pub piece_frontier_grid: i64,
    pub lattice_poses_screened: usize,
    pub exact_valid_sub_frontier_poses: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_sub_frontier_grid: Option<i64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyLnsDiagnostics {
    pub rounds: usize,
    pub rounds_accepted: usize,
    pub rounds_reverted: usize,
    pub reinsertions: usize,
    pub reinsert_failures: usize,
    pub separation_moves: usize,
    pub separation_probes: usize,
    pub separation_zero_overlap: usize,
    pub separation_recruits: usize,
    pub separation_pair_moves: usize,
    pub separation_weight_bumps: usize,
    pub separation_relocations: usize,
    pub rounds_wandered: usize,
    pub optimizer_improvements: usize,
    pub frontier_before_grid: i64,
    pub frontier_after_grid: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyGroupDropDiagnostics {
    pub rounds: usize,
    pub cuts_evaluated: usize,
    pub probes: usize,
    pub accepted_drops: usize,
    pub frontier_before_grid: i64,
    pub frontier_after_grid: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancySettleDiagnostics {
    pub sweeps: usize,
    pub attempts: usize,
    pub accepted_moves: usize,
    pub exact_rows: usize,
    pub frontier_before_grid: i64,
    pub frontier_after_grid: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyArchiveDiagnostics {
    pub stagnation_threshold_layers: usize,
    pub revival_cooldown_layers: usize,
    pub max_revival_expansions: usize,
    pub revival_policy: String,
    pub revivals_expanded: usize,
    pub revivals_skipped: usize,
    pub revival_children_generated: usize,
    pub revival_children_retained: usize,
    pub archive_peak_bytes: usize,
    pub final_archived_area_fingerprint: Option<String>,
    pub final_archived_count_fingerprint: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyArchiveLayerDiagnostics {
    pub layers_since_improvement: usize,
    pub revival_attempted: bool,
    pub revival_expanded: bool,
    pub revival_kind: Option<String>,
    pub revived_state_fingerprint: Option<String>,
    pub replaced_state_fingerprint: Option<String>,
    pub skipped_reason: Option<String>,
    pub revival_children_generated: usize,
    pub revival_children_retained: usize,
    pub archived_area_updated: bool,
    pub archived_count_updated: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyWorkDiagnostics {
    pub selected_piece_slots: usize,
    pub orientation_streams: usize,
    pub source_feature_visits: usize,
    pub position_source_attempts: usize,
    pub returned_positions: usize,
    pub hazard_queries: usize,
    pub proxy_pressure_visits: usize,
    pub exact_finalist_rows: usize,
    pub experimental_collision_builds: usize,
    pub validator_collision_builds: usize,
    pub experimental_pair_visits: usize,
    pub validator_pair_visits: usize,
    pub transformed_collision_vertices: usize,
    pub clipper_input_vertices: usize,
    pub clipper_output_vertices: usize,
    pub partial_audits: usize,
    pub complete_audits: usize,
    pub retained_peak_bytes: usize,
    pub selector_diagnostic_peak_bytes: usize,
    pub total_retained_peak_bytes: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyLayerDiagnostics {
    pub layer: usize,
    pub parents: usize,
    pub generated_children: usize,
    pub retained_states: usize,
    pub distinct_contact_signatures: usize,
    pub selected_piece_ids: Vec<String>,
    pub parent_selections: Vec<GeneralPersistentVacancyParentSelectionDiagnostics>,
    pub direct_insertions: usize,
    pub ejection_insertions: usize,
    pub best_inactive_piece_count: usize,
    pub best_inactive_piece_ids: Vec<String>,
    pub best_inactive_area_grid2: String,
    pub best_state_fingerprint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elite: Option<GeneralPersistentVacancyEliteLayerDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive: Option<GeneralPersistentVacancyArchiveLayerDiagnostics>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyEliteLayerDiagnostics {
    pub entering_population_hash: String,
    pub ordinary_child_order_hash: String,
    pub complete_candidate_order_hash: String,
    pub pre_carryover_work: GeneralPersistentVacancyWorkDiagnostics,
    pub area_elite_fingerprint: String,
    pub area_elite_inactive_piece_count: usize,
    pub area_elite_inactive_area_grid2: String,
    pub count_elite_fingerprint: String,
    pub count_elite_inactive_piece_count: usize,
    pub count_elite_inactive_area_grid2: String,
    pub best_ever_area_elite_fingerprint: String,
    pub best_ever_area_elite_inactive_piece_count: usize,
    pub best_ever_area_elite_inactive_area_grid2: String,
    pub best_ever_count_elite_fingerprint: String,
    pub best_ever_count_elite_inactive_piece_count: usize,
    pub best_ever_count_elite_inactive_area_grid2: String,
    pub offered_carryover_fingerprints: Vec<String>,
    pub offered_carryovers_distinct: bool,
    pub retained_carryover_fingerprints: Vec<String>,
    pub expanded_carryover_fingerprints: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancyParentSelectionDiagnostics {
    pub parent_state_fingerprint: String,
    pub inactive_order_hash: String,
    pub scheduler_family: String,
    pub hardest_piece_id: String,
    pub rotation_start_index: Option<usize>,
    pub coverage_piece_id: Option<String>,
    pub transition_seed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revived: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relocated_piece_id: Option<String>,
    pub slots: Vec<GeneralPersistentVacancySelectionSlotDiagnostics>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPersistentVacancySelectionSlotDiagnostics {
    pub selected_ordinal: usize,
    pub piece_id: String,
    pub angle_seed: u64,
    pub diversity_seed: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPrecompressionFrontierVacancyDiagnostics {
    pub mode: usize,
    pub attempted: bool,
    pub target_depth_mm: Option<f64>,
    pub incumbent_strip_depth_mm: Option<f64>,
    pub checkpoint_fingerprint: Option<String>,
    pub selected_piece_ids: Vec<String>,
    pub incumbent_parent_fingerprint: Option<String>,
    pub eligible_parent_fingerprints: Vec<String>,
    pub selected_parent_fingerprint: Option<String>,
    pub selected_parent_depth_mm: Option<f64>,
    pub selected_compressed_raw_loss: Option<f64>,
    pub full_scores: usize,
    pub full_score_pair_visits: usize,
    pub rebuilt_children: Vec<GeneralPrecompressionFrontierChildDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rebuilt_child_record_hash: Option<String>,
    pub rebuild: GeneralConflictRuinRebuildDiagnostics,
    pub control: Option<GeneralCoupledSeparatorTargetDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage_a_seed_domain: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage_a_target_seed: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage_a_compression_seed: Option<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stage_a_worker_seeds: Vec<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage_a: Option<GeneralConflictRuinArmDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage_a_independent_audit: Option<GeneralPrecompressionIndependentAuditDiagnostics>,
    pub treatment: GeneralConflictRuinArmDiagnostics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_counts: Option<GeneralPrecompressionValidationDiagnostics>,
    pub mechanism_passed: bool,
    pub skipped_reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPrecompressionIndependentAuditDiagnostics {
    pub attempted: bool,
    pub fresh_score_agreement: bool,
    pub final_positive_pairs: Option<usize>,
    pub final_boundary_violations: Option<usize>,
    pub final_boundary_loss: Option<f64>,
    pub positive_boundary_rows: Vec<GeneralPrecompressionBoundaryRowDiagnostics>,
    pub audited_placement_fingerprint: Option<String>,
    pub independent_audit_valid: bool,
    pub independent_audit_count: usize,
    pub used_short_axis_span_mm: Option<f64>,
    pub used_long_axis_depth_mm: Option<f64>,
    pub unused_short_axis_projection_mm: Option<f64>,
    pub occupied_envelope_area_mm2: Option<f64>,
    pub rejection_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPrecompressionBoundaryRowDiagnostics {
    pub piece_id: String,
    pub violations: usize,
    pub raw_loss: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPrecompressionValidationDiagnostics {
    pub incumbent: usize,
    pub rebuilt_children: usize,
    pub stage_a: usize,
    pub stage_b: usize,
    pub total: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralPrecompressionFrontierChildDiagnostics {
    pub beam_ordinal: usize,
    pub fingerprint: String,
    pub exact_overlap_area_mm2: f64,
    pub exact_positive_overlap_pairs: usize,
    pub frontier_depth_mm: f64,
    pub fresh_raw_loss: f64,
    pub fresh_positive_pairs: usize,
    pub fresh_feasible: bool,
    pub publication_valid: bool,
    pub publication_rejection_reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralConflictRuinDiagnostics {
    pub attempted: bool,
    pub seed_domain: u64,
    pub target_depth_mm: Option<f64>,
    pub checkpoint_fingerprint: Option<String>,
    pub selector_mode: Option<String>,
    pub root_piece_id: Option<String>,
    pub root_boundary_loss: Option<f64>,
    pub root_probe_pose: Option<GeneralCoupledSeparatorPlacementDiagnostics>,
    pub root_probe_blockers: Vec<GeneralConflictRuinBlockerDiagnostics>,
    pub root_probe_tracker_loss: Option<f64>,
    pub root_probe_tracker_boundary_loss: Option<f64>,
    pub root_probe_tracker_positive_pairs: Option<usize>,
    pub root_probe_tracker_feasible: Option<bool>,
    pub root_probe_exact_valid: Option<bool>,
    pub root_probe_exact_depth_mm: Option<f64>,
    pub root_probe_improves_incumbent: Option<bool>,
    pub root_probe_exact_rejection_reason: Option<String>,
    pub root_probe_state_fingerprint: Option<String>,
    pub removed_piece_ids: Vec<String>,
    pub removal_order_piece_ids: Vec<String>,
    pub rebuild: GeneralConflictRuinRebuildDiagnostics,
    pub retry_control: GeneralConflictRuinArmDiagnostics,
    pub treatment: GeneralConflictRuinArmDiagnostics,
    pub skipped_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralConflictRuinBlockerDiagnostics {
    pub piece_id: String,
    pub proxy_pressure: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralConflictRuinRebuildDiagnostics {
    pub elapsed_ms: f64,
    pub initial_exact_overlap_area_mm2: f64,
    pub selected_exact_overlap_area_mm2: Option<f64>,
    pub initial_positive_overlap_pairs: usize,
    pub selected_positive_overlap_pairs: Option<usize>,
    pub parent_orientation_streams: usize,
    pub cheap_queries: usize,
    pub exact_finalists: usize,
    pub exact_pair_intersection_limit: usize,
    pub exact_pair_intersections: usize,
    pub required_current_finalists: usize,
    pub orientation_build_limit: usize,
    pub orientation_builds: usize,
    pub transformed_output_vertices: usize,
    pub feature_visits: usize,
    pub pre_dedup_contact_attempts: usize,
    pub deduplicated_proposals: usize,
    pub clipper_input_vertices: usize,
    pub clipper_output_vertices: usize,
    pub partials_retained: usize,
    pub selected_state_fingerprint: Option<String>,
    pub cap_exhausted: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralConflictRuinArmDiagnostics {
    pub attempted: bool,
    pub applied_rebuild: bool,
    pub elapsed_ms: f64,
    pub initial_state_fingerprint: Option<String>,
    pub final_state_fingerprint: Option<String>,
    pub exact_valid: bool,
    pub accepted_depth_mm: Option<f64>,
    pub final_placement_fingerprint: Option<String>,
    pub final_placements: Vec<GeneralCoupledSeparatorPlacementDiagnostics>,
    pub work: GeneralConflictRuinRetryWorkDiagnostics,
    pub target: Option<GeneralCoupledSeparatorTargetDiagnostics>,
    pub failure_reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralConflictRuinRetryWorkDiagnostics {
    pub worker_sweeps: usize,
    pub dynamic_queries: usize,
    pub pressure_evaluations: usize,
    pub retained_confirmations: usize,
    pub hazard_updates: usize,
    pub layout_loads: usize,
    pub index_builds: usize,
    pub worker_full_score_pair_visits: usize,
    pub auditor_full_score_pair_visits: usize,
    pub auditor_dynamic_queries: usize,
    pub auditor_pressure_evaluations: usize,
    pub auditor_layout_loads: usize,
    pub auditor_index_builds: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralCoupledSeparatorArmDiagnostics {
    pub pressure_model: String,
    pub attempted: bool,
    pub targets_attempted: usize,
    pub targets_accepted: usize,
    pub initial_depth_mm: f64,
    pub final_depth_mm: f64,
    pub worker_sweeps: usize,
    pub dynamic_queries: usize,
    pub pressure_evaluations: usize,
    pub retained_confirmations: usize,
    pub hazard_updates: usize,
    pub layout_loads: usize,
    pub catalog_builds: usize,
    pub immutable_variant_builds: usize,
    pub index_builds: usize,
    pub worker_full_score_pair_visits: usize,
    pub auditor_full_score_pair_visits: usize,
    pub auditor_dynamic_queries: usize,
    pub auditor_pressure_evaluations: usize,
    pub auditor_layout_loads: usize,
    pub auditor_index_builds: usize,
    pub independently_measured_final_depth_mm: Option<f64>,
    pub final_placement_fingerprint: Option<String>,
    pub final_placements: Vec<GeneralCoupledSeparatorPlacementDiagnostics>,
    pub skipped_reason: Option<String>,
    pub targets: Vec<GeneralCoupledSeparatorTargetDiagnostics>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralCoupledSeparatorPlacementDiagnostics {
    pub piece_id: String,
    pub rotation_deg: f64,
    pub mirrored: bool,
    pub translate_short_axis: f64,
    pub translate_long_axis: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralCoupledSeparatorTargetDiagnostics {
    pub ordinal: usize,
    pub target_depth_mm: f64,
    pub compression_split_mm: f64,
    pub target_seed: u64,
    pub compression_seed: u64,
    pub worker_seeds: Vec<u64>,
    pub initial_state_fingerprint: String,
    pub final_state_fingerprint: String,
    pub rounds: usize,
    pub strikes: usize,
    pub rollbacks: usize,
    pub full_rescore_agreements: usize,
    pub initial_raw_loss: f64,
    pub minimum_raw_loss: f64,
    pub final_raw_loss: f64,
    pub final_weighted_loss: f64,
    pub feasible: bool,
    pub exact_valid: bool,
    pub exact_accepted: bool,
    pub exact_rejection_reason: Option<String>,
    pub accepted_depth_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boundary_projection: Option<GeneralBoundaryProjectionDiagnostics>,
    pub cap_exhausted: Option<String>,
    pub failure_reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralBoundaryProjectionDiagnostics {
    pub attempted: bool,
    pub root_piece_id: Option<String>,
    pub root_boundary_loss: Option<f64>,
    pub projected_pose: Option<GeneralCoupledSeparatorPlacementDiagnostics>,
    pub projected_pieces: Vec<GeneralCoupledSeparatorPlacementDiagnostics>,
    pub exact_valid: bool,
    pub exact_accepted: bool,
    pub exact_depth_mm: Option<f64>,
    pub state_fingerprint: Option<String>,
    pub rejection_reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralRelaxedLossDistribution {
    pub samples: usize,
    pub sum: f64,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
}

impl GeneralRelaxedLossDistribution {
    fn observe(&mut self, value: f64) {
        self.samples = self.samples.saturating_add(1);
        self.sum += value;
        self.minimum = Some(self.minimum.map_or(value, |minimum| minimum.min(value)));
        self.maximum = Some(self.maximum.map_or(value, |maximum| maximum.max(value)));
    }

    fn merge(&mut self, other: Self) {
        self.samples = self.samples.saturating_add(other.samples);
        self.sum += other.sum;
        if let Some(minimum) = other.minimum {
            self.minimum = Some(self.minimum.map_or(minimum, |current| current.min(minimum)));
        }
        if let Some(maximum) = other.maximum {
            self.maximum = Some(self.maximum.map_or(maximum, |current| current.max(maximum)));
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralRelaxedEpochDiagnostics {
    pub epoch: usize,
    pub selected_lane: usize,
    pub restart_disruptions: usize,
    pub target_depth_mm: f64,
    pub weighted_loss: f64,
    pub collision_pairs: usize,
    pub blocking_pairs: Vec<GeneralRelaxedPairDiagnostics>,
    pub boundary_violations: usize,
    pub boundary_piece_ids: Vec<String>,
    pub surrogate_feasible: bool,
    pub exact_valid: bool,
    pub exact_accepted: bool,
    pub translation_evaluations: usize,
    pub rotation_evaluations: usize,
    pub complete_queries: usize,
    pub retained_f64_confirmations: usize,
    pub accepted_moves: usize,
    pub incumbent_depth_before_mm: f64,
    pub incumbent_depth_after_mm: f64,
    pub incumbent_depth_delta_mm: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralRelaxedPairDiagnostics {
    pub first_piece_id: String,
    pub second_piece_id: String,
    pub raw_penalty: f64,
    pub guided_weight: f64,
    pub weighted_pressure: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeneralRelaxedOutcome {
    pub result: GeneralFastResult,
    pub diagnostics: GeneralRelaxedDiagnostics,
}

#[derive(Clone, Copy, Debug)]
struct Triangle {
    points: [IrregularPoint; 3],
    bounds: IrregularBounds,
}

#[derive(Clone, Copy)]
struct Pole {
    center: IrregularPoint,
    radius: f64,
}

impl Triangle {
    fn new(points: [IrregularPoint; 3]) -> Self {
        let bounds = IrregularBounds::new(
            points
                .iter()
                .map(|point| point.x)
                .fold(f64::INFINITY, f64::min),
            points
                .iter()
                .map(|point| point.y)
                .fold(f64::INFINITY, f64::min),
            points
                .iter()
                .map(|point| point.x)
                .fold(f64::NEG_INFINITY, f64::max),
            points
                .iter()
                .map(|point| point.y)
                .fold(f64::NEG_INFINITY, f64::max),
        );
        Self { points, bounds }
    }
}

#[derive(Clone)]
struct CellIndex {
    bounds: IrregularBounds,
    bins: Vec<Vec<usize>>,
}

struct PieceIndex {
    bounds: IrregularBounds,
    bins: Vec<Vec<usize>>,
}

impl PieceIndex {
    fn new(bounds: IrregularBounds) -> Self {
        Self {
            bounds,
            bins: vec![Vec::new(); PIECE_INDEX_SIDE * PIECE_INDEX_SIDE],
        }
    }

    fn insert(&mut self, piece_index: usize, bounds: IrregularBounds) {
        let (min_x, max_x, min_y, max_y) = bin_range(bounds, self.bounds, PIECE_INDEX_SIDE);
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                self.bins[y * PIECE_INDEX_SIDE + x].push(piece_index);
            }
        }
    }

    fn remove(&mut self, piece_index: usize, bounds: IrregularBounds) {
        let (min_x, max_x, min_y, max_y) = bin_range(bounds, self.bounds, PIECE_INDEX_SIDE);
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                if let Some(position) = self.bins[y * PIECE_INDEX_SIDE + x]
                    .iter()
                    .position(|candidate| *candidate == piece_index)
                {
                    self.bins[y * PIECE_INDEX_SIDE + x].swap_remove(position);
                }
            }
        }
    }

    fn query_into(&self, bounds: IrregularBounds, scratch: &mut PieceQueryScratch) {
        let (min_x, max_x, min_y, max_y) = bin_range(bounds, self.bounds, PIECE_INDEX_SIDE);
        scratch.begin_query();
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                for piece_index in self.bins[y * PIECE_INDEX_SIDE + x].iter().copied() {
                    scratch.report(piece_index);
                }
            }
        }
        scratch.selected.sort_unstable();
    }
}

struct PieceQueryScratch {
    marks: Vec<u32>,
    generation: u32,
    selected: Vec<usize>,
}

impl PieceQueryScratch {
    fn new(piece_count: usize) -> Self {
        Self {
            marks: vec![0; piece_count],
            generation: 0,
            selected: Vec::with_capacity(piece_count),
        }
    }

    fn begin_query(&mut self) {
        self.selected.clear();
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.marks.fill(0);
            self.generation = 1;
        }
    }

    fn report(&mut self, piece_index: usize) {
        if self.marks[piece_index] != self.generation {
            self.marks[piece_index] = self.generation;
            self.selected.push(piece_index);
        }
    }
}

impl CellIndex {
    fn new(cells: &[Triangle], bounds: IrregularBounds) -> Self {
        let mut bins = vec![Vec::new(); CELL_INDEX_SIDE * CELL_INDEX_SIDE];
        for (cell_index, cell) in cells.iter().enumerate() {
            let (min_x, max_x, min_y, max_y) = cell_bin_range(cell.bounds, bounds);
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    bins[y * CELL_INDEX_SIDE + x].push(cell_index);
                }
            }
        }
        Self { bounds, bins }
    }

    fn query_mask(
        &self,
        bounds: IrregularBounds,
        translate_x: f64,
        translate_y: f64,
    ) -> [u64; MAX_CELLS_PER_PIECE / 64] {
        let local = IrregularBounds::new(
            bounds.min_x - translate_x,
            bounds.min_y - translate_y,
            bounds.max_x - translate_x,
            bounds.max_y - translate_y,
        );
        let (min_x, max_x, min_y, max_y) = cell_bin_range(local, self.bounds);
        let mut selected = [0_u64; MAX_CELLS_PER_PIECE / 64];
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                for cell_index in self.bins[y * CELL_INDEX_SIDE + x].iter().copied() {
                    selected[cell_index / 64] |= 1_u64 << (cell_index % 64);
                }
            }
        }
        selected
    }
}

#[derive(Clone)]
struct OrientedSurrogate {
    collision: PolygonSet,
    cells: Vec<Triangle>,
    poles: Vec<Pole>,
    bounds: IrregularBounds,
    cell_index: CellIndex,
    difficulty: f64,
    diameter: f64,
}

struct SurrogateCatalog {
    geometry_class_by_input: Vec<usize>,
    orientations: BTreeMap<SurrogateKey, OrientedSurrogate>,
    shared_pair_nfps: BTreeMap<PairNfpKey, Arc<PairNfp>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SurrogateCatalogMode {
    StructuredGrid,
    CurrentAssignment,
    ZeroDegreeOnly,
}

#[derive(Clone)]
struct RelaxedPlacement {
    input_index: usize,
    rotation_deg: f64,
    mirrored: bool,
    translate_x: f64,
    translate_y: f64,
}

#[derive(Clone)]
struct RelaxedState {
    placements: Vec<RelaxedPlacement>,
    strip_depth_mm: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GridInnerFit {
    min_x: i128,
    max_x: i128,
    min_y: i128,
    max_y: i128,
}

impl GridInnerFit {
    fn contains(self, x: i128, y: i128) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct WorkCounters {
    oriented_surrogate_builds: usize,
    generated_cells: usize,
    ejection_chain_evaluations: usize,
    ejection_chain_accepts: usize,
    surrogate_evaluations: usize,
    piece_broad_phase_probes: usize,
    cell_index_probes: usize,
    sat_tests: usize,
    pair_nfp_builds: usize,
    pair_nfp_components: usize,
    shared_pair_nfp_entries: usize,
    shared_pair_nfp_components: usize,
    shared_pair_nfp_estimated_bytes: usize,
    shared_pair_nfp_adoptions: usize,
    directional_pair_evaluations: usize,
    directional_exact_confirmations: usize,
    directional_cache_hits: usize,
    directional_cache_misses: usize,
    directional_component_visits: usize,
    directional_intervals_produced: usize,
    directional_intervals_merged: usize,
    directional_over_budget_candidates: usize,
    directional_zero_penetration_inconsistencies: usize,
    directional_lane_rejections: usize,
    directional_relocations: usize,
    directional_rejected_contractions: usize,
    directional_containment_rejections: usize,
    directional_initial_pair_loss: GeneralRelaxedLossDistribution,
    directional_initial_boundary_loss: GeneralRelaxedLossDistribution,
    directional_accepted_pair_loss: GeneralRelaxedLossDistribution,
    directional_accepted_boundary_loss: GeneralRelaxedLossDistribution,
    axis_events: usize,
    axis_candidate_evaluations: usize,
    dynamic_hazard_queries: usize,
    dynamic_hazard_updates: usize,
    dynamic_pressure_evaluations: usize,
    dynamic_layout_loads: usize,
    dynamic_index_builds: usize,
    translation_evaluations: usize,
    rotation_evaluations: usize,
    retained_f64_confirmations: usize,
    confirmed_pair_additions: usize,
    confirmed_pair_removals: usize,
    accepted_moves: usize,
    angular_repair_successors: usize,
    angular_repair_improvements: usize,
    angular_repair_queries: usize,
}

impl WorkCounters {
    fn accumulate(&mut self, other: Self) {
        self.oriented_surrogate_builds = self
            .oriented_surrogate_builds
            .saturating_add(other.oriented_surrogate_builds);
        self.generated_cells = self.generated_cells.saturating_add(other.generated_cells);
        self.ejection_chain_evaluations = self
            .ejection_chain_evaluations
            .saturating_add(other.ejection_chain_evaluations);
        self.ejection_chain_accepts = self
            .ejection_chain_accepts
            .saturating_add(other.ejection_chain_accepts);
        self.surrogate_evaluations = self
            .surrogate_evaluations
            .saturating_add(other.surrogate_evaluations);
        self.piece_broad_phase_probes = self
            .piece_broad_phase_probes
            .saturating_add(other.piece_broad_phase_probes);
        self.cell_index_probes = self
            .cell_index_probes
            .saturating_add(other.cell_index_probes);
        self.sat_tests = self.sat_tests.saturating_add(other.sat_tests);
        self.pair_nfp_builds = self.pair_nfp_builds.saturating_add(other.pair_nfp_builds);
        self.pair_nfp_components = self
            .pair_nfp_components
            .saturating_add(other.pair_nfp_components);
        self.shared_pair_nfp_entries = self
            .shared_pair_nfp_entries
            .saturating_add(other.shared_pair_nfp_entries);
        self.shared_pair_nfp_components = self
            .shared_pair_nfp_components
            .saturating_add(other.shared_pair_nfp_components);
        self.shared_pair_nfp_estimated_bytes = self
            .shared_pair_nfp_estimated_bytes
            .saturating_add(other.shared_pair_nfp_estimated_bytes);
        self.shared_pair_nfp_adoptions = self
            .shared_pair_nfp_adoptions
            .saturating_add(other.shared_pair_nfp_adoptions);
        self.directional_pair_evaluations = self
            .directional_pair_evaluations
            .saturating_add(other.directional_pair_evaluations);
        self.directional_exact_confirmations = self
            .directional_exact_confirmations
            .saturating_add(other.directional_exact_confirmations);
        self.directional_cache_hits = self
            .directional_cache_hits
            .saturating_add(other.directional_cache_hits);
        self.directional_cache_misses = self
            .directional_cache_misses
            .saturating_add(other.directional_cache_misses);
        self.directional_component_visits = self
            .directional_component_visits
            .saturating_add(other.directional_component_visits);
        self.directional_intervals_produced = self
            .directional_intervals_produced
            .saturating_add(other.directional_intervals_produced);
        self.directional_intervals_merged = self
            .directional_intervals_merged
            .saturating_add(other.directional_intervals_merged);
        self.directional_over_budget_candidates = self
            .directional_over_budget_candidates
            .saturating_add(other.directional_over_budget_candidates);
        self.directional_zero_penetration_inconsistencies = self
            .directional_zero_penetration_inconsistencies
            .saturating_add(other.directional_zero_penetration_inconsistencies);
        self.directional_lane_rejections = self
            .directional_lane_rejections
            .saturating_add(other.directional_lane_rejections);
        self.directional_relocations = self
            .directional_relocations
            .saturating_add(other.directional_relocations);
        self.directional_rejected_contractions = self
            .directional_rejected_contractions
            .saturating_add(other.directional_rejected_contractions);
        self.directional_containment_rejections = self
            .directional_containment_rejections
            .saturating_add(other.directional_containment_rejections);
        self.directional_initial_pair_loss
            .merge(other.directional_initial_pair_loss);
        self.directional_initial_boundary_loss
            .merge(other.directional_initial_boundary_loss);
        self.directional_accepted_pair_loss
            .merge(other.directional_accepted_pair_loss);
        self.directional_accepted_boundary_loss
            .merge(other.directional_accepted_boundary_loss);
        self.axis_events = self.axis_events.saturating_add(other.axis_events);
        self.axis_candidate_evaluations = self
            .axis_candidate_evaluations
            .saturating_add(other.axis_candidate_evaluations);
        self.dynamic_hazard_queries = self
            .dynamic_hazard_queries
            .saturating_add(other.dynamic_hazard_queries);
        self.dynamic_hazard_updates = self
            .dynamic_hazard_updates
            .saturating_add(other.dynamic_hazard_updates);
        self.dynamic_pressure_evaluations = self
            .dynamic_pressure_evaluations
            .saturating_add(other.dynamic_pressure_evaluations);
        self.dynamic_layout_loads = self
            .dynamic_layout_loads
            .saturating_add(other.dynamic_layout_loads);
        self.dynamic_index_builds = self
            .dynamic_index_builds
            .saturating_add(other.dynamic_index_builds);
        self.translation_evaluations = self
            .translation_evaluations
            .saturating_add(other.translation_evaluations);
        self.rotation_evaluations = self
            .rotation_evaluations
            .saturating_add(other.rotation_evaluations);
        self.retained_f64_confirmations = self
            .retained_f64_confirmations
            .saturating_add(other.retained_f64_confirmations);
        self.confirmed_pair_additions = self
            .confirmed_pair_additions
            .saturating_add(other.confirmed_pair_additions);
        self.confirmed_pair_removals = self
            .confirmed_pair_removals
            .saturating_add(other.confirmed_pair_removals);
        self.accepted_moves = self.accepted_moves.saturating_add(other.accepted_moves);
        self.angular_repair_successors = self
            .angular_repair_successors
            .saturating_add(other.angular_repair_successors);
        self.angular_repair_improvements = self
            .angular_repair_improvements
            .saturating_add(other.angular_repair_improvements);
        self.angular_repair_queries = self
            .angular_repair_queries
            .saturating_add(other.angular_repair_queries);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PairEntry {
    raw_loss: f64,
    guided_weight: f64,
    normalization_scale: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BoundaryEntry {
    violations: usize,
    raw_loss: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct PairTracker {
    piece_count: usize,
    boundaries: Vec<BoundaryEntry>,
    pairs: Vec<PairEntry>,
    incident_raw_loss: Vec<f64>,
    boundary_violations: usize,
    boundary_loss: f64,
    collision_pairs: Vec<(usize, usize, f64)>,
    weighted_loss: f64,
}

impl PairTracker {
    fn feasible(&self) -> bool {
        self.boundary_violations == 0 && self.collision_pairs.is_empty()
    }

    fn common_loss(&self) -> f64 {
        self.boundary_loss
            + self
                .collision_pairs
                .iter()
                .map(|(_, _, penalty)| *penalty)
                .sum::<f64>()
    }

    fn pair(&self, first: usize, second: usize) -> PairEntry {
        self.pairs[pair_slot(self.piece_count, first, second)]
    }

    fn replace_pair(&mut self, first: usize, second: usize, raw_loss: f64, guided_weight: f64) {
        let slot = pair_slot(self.piece_count, first, second);
        let old = self.pair(first, second);
        let normalized_old = old.raw_loss / old.normalization_scale;
        let normalization_scale = 1.0;
        let normalized_new = raw_loss / normalization_scale;
        self.incident_raw_loss[first] =
            (self.incident_raw_loss[first] - normalized_old + normalized_new).max(0.0);
        self.incident_raw_loss[second] =
            (self.incident_raw_loss[second] - normalized_old + normalized_new).max(0.0);
        self.pairs[slot] = PairEntry {
            raw_loss,
            guided_weight,
            normalization_scale,
        };
    }

    fn replace_boundary(&mut self, index: usize, boundary: BoundaryEntry) {
        self.boundaries[index] = boundary;
    }
}

#[derive(Clone, Debug)]
struct PlacementScore {
    boundary_violations: usize,
    boundary_loss: f64,
    collision_pairs: Vec<(usize, usize, f64)>,
    weighted_loss: f64,
}

#[derive(Clone)]
struct LaneOutcome {
    state: RelaxedState,
    score: PairTracker,
    weights: BTreeMap<(usize, usize), f64>,
    counters: WorkCounters,
    selected_lane: usize,
    restart_disruptions: usize,
}

struct LaneBatch {
    outcomes: Vec<LaneOutcome>,
    counters: WorkCounters,
}

enum ExactLaneValidation {
    Infeasible,
    Accepted {
        placements: Vec<GeneralFastPlacement>,
        metrics: GeneralPlacementMetrics,
    },
    Rejected,
}

struct SelectedLane {
    outcome: LaneOutcome,
    validation: ExactLaneValidation,
}

#[derive(Clone)]
struct EjectionCandidate {
    replacements: Vec<(usize, RelaxedPlacement)>,
    score: PairTracker,
}

struct RepairExperimentOutcome {
    selected: Option<LaneOutcome>,
    counters: WorkCounters,
    control_loss: f64,
    rotation_loss: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoupledSeparatorArm {
    Control,
    Treatment,
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoupledTerminalPolicy {
    None,
    ExactBoundaryProjection,
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoupledRollbackRescorePolicy {
    StrictDerivedAgreement,
    CanonicalAuthoritativeRows,
}

impl CoupledSeparatorArm {
    #[cfg(feature = "jagua-experimental")]
    fn pressure_model(self) -> GeneralRelaxedPressureModel {
        GeneralRelaxedPressureModel::DynamicPoles
    }

    #[cfg(feature = "jagua-experimental")]
    fn refines_rotation(self) -> bool {
        self == Self::Treatment
    }

    #[cfg(feature = "jagua-experimental")]
    fn label(self) -> &'static str {
        match self {
            Self::Control => "dynamicCoveragePolesTranslationOnly",
            Self::Treatment => "dynamicCoveragePolesRigidDescent",
        }
    }
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RawMinimumTransition {
    NoImprovement,
    MinorImprovement,
    SubstantialImprovement,
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoupledRoundDisposition {
    AcceptFeasible,
    ContinueInfeasible(RawMinimumTransition),
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone)]
struct CoupledMinimumCheckpoint {
    state: RelaxedState,
    score: PairTracker,
}

#[cfg(feature = "jagua-experimental")]
struct CoupledTargetOutcome {
    diagnostics: GeneralCoupledSeparatorTargetDiagnostics,
    accepted: Option<GeneralFastResult>,
    work: CoupledSeparatorWork,
    minimum: Option<CoupledMinimumCheckpoint>,
    final_state: RelaxedState,
    exact_metrics: Option<GeneralPlacementMetrics>,
    independent_audit: Option<CoupledIndependentAuditOutcome>,
}

#[cfg(feature = "jagua-experimental")]
struct CoupledIndependentAuditOutcome {
    diagnostics: GeneralPrecompressionIndependentAuditDiagnostics,
    metrics: Option<GeneralPlacementMetrics>,
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone)]
struct CoupledFailedCheckpoint {
    incumbent: GeneralFastResult,
    target_ordinal: usize,
    target_depth_mm: f64,
    compression_split_mm: f64,
    target_seed: u64,
    compression_seed: u64,
    catalog: Arc<SurrogateCatalog>,
    hazard_catalog: Arc<JaguaHazardCatalog>,
    minimum: CoupledMinimumCheckpoint,
    attempt_diagnostics: GeneralCoupledSeparatorTargetDiagnostics,
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone)]
struct CoupledArmOutcome {
    diagnostics: GeneralCoupledSeparatorArmDiagnostics,
    checkpoint: Option<CoupledFailedCheckpoint>,
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone, Copy, Debug)]
struct ConflictRuinExactScore {
    total_overlap_area_mm2: f64,
    positive_overlap_pairs: usize,
    maximum_pair_area_mm2: f64,
    frontier_depth_mm: f64,
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone)]
struct ConflictRuinBeamState {
    state: RelaxedState,
    active: Vec<bool>,
    collisions: Vec<Option<PolygonSet>>,
    score: ConflictRuinExactScore,
}

#[cfg(feature = "jagua-experimental")]
struct ConflictRuinBuildOutcome {
    beam: Vec<ConflictRuinBeamState>,
    initial_score: ConflictRuinExactScore,
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone)]
struct ConflictRuinCandidate {
    placement: RelaxedPlacement,
    proxy_loss: f64,
}

#[cfg(feature = "jagua-experimental")]
struct PrecompressionExactParentCandidate {
    compressed: RelaxedState,
    metrics: GeneralPlacementMetrics,
    compressed_raw_loss: f64,
    frontier_depth_mm: f64,
    fingerprint: String,
}

#[cfg(feature = "jagua-experimental")]
struct PrecompressionInfeasibleChild {
    state: RelaxedState,
    fresh_raw_loss: f64,
    fresh_positive_pairs: usize,
    beam_ordinal: usize,
    fingerprint: String,
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone, Copy, Debug, Default)]
struct ConflictRuinWork {
    orientation_build_limit: usize,
    pair_intersection_limit: usize,
    parent_orientation_streams: usize,
    cheap_queries: usize,
    exact_finalists: usize,
    exact_pair_intersections: usize,
    required_current_finalists: usize,
    orientation_builds: usize,
    transformed_output_vertices: usize,
    feature_visits: usize,
    pre_dedup_contact_attempts: usize,
    deduplicated_proposals: usize,
    clipper_input_vertices: usize,
    clipper_output_vertices: usize,
    partials_retained: usize,
}

#[cfg(feature = "jagua-experimental")]
impl ConflictRuinWork {
    fn for_piece_count(piece_count: usize) -> Self {
        let projected_roots = piece_count.min(CONFLICT_RUIN_REMOVED_PIECES);
        let selector_pairs = projected_roots
            .saturating_mul(piece_count.saturating_sub(1))
            .saturating_sub(projected_roots.saturating_mul(projected_roots.saturating_sub(1)) / 2);
        Self {
            orientation_build_limit: piece_count
                .saturating_mul(2)
                .saturating_add(CONFLICT_RUIN_STREAM_CAP)
                .saturating_add(CONFLICT_RUIN_FINALIST_CAP),
            pair_intersection_limit: piece_count
                .saturating_mul(piece_count.saturating_sub(1))
                .saturating_div(2)
                .saturating_add(selector_pairs)
                .saturating_add(
                    CONFLICT_RUIN_FINALIST_CAP.saturating_mul(piece_count.saturating_sub(1)),
                ),
            ..Self::default()
        }
    }
}

#[cfg(feature = "jagua-experimental")]
struct ConflictRuinBoundaryProbe {
    root: usize,
    placement: RelaxedPlacement,
    blockers: Vec<(usize, f64)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CoordinateAxis {
    Horizontal,
    Vertical,
    ForwardDiagonal,
    BackwardDiagonal,
    Rotation,
}

struct LaneSearch<'a> {
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    catalog: Arc<SurrogateCatalog>,
    rng: SplitMix64,
    weights: BTreeMap<(usize, usize), f64>,
    counters: WorkCounters,
    allow_worsening_chain: bool,
    piece_query_scratch: PieceQueryScratch,
    pair_nfp_cache: BTreeMap<PairNfpKey, Arc<PairNfp>>,
    pair_nfp_cache_components: usize,
    #[cfg(feature = "jagua-experimental")]
    hazard_index: Option<JaguaHazardIndex>,
    #[cfg(feature = "jagua-experimental")]
    hazard_catalog: Option<Arc<JaguaHazardCatalog>>,
    dynamic_query_limit: Option<usize>,
    refine_rotation: bool,
}

type SurrogateKey = (usize, i64, bool);
type PairNfpKey = (usize, i64, bool, usize, i64, bool);

#[derive(Clone, PartialEq)]
struct ConvexNfp {
    points: Vec<IrregularPoint>,
    bounds: IrregularBounds,
}

#[derive(Clone, PartialEq)]
struct PairNfp {
    components: Vec<ConvexNfp>,
}

struct PairAxisIntervals {
    nfp_key: PairNfpKey,
    fixed_translate_x: f64,
    fixed_translate_y: f64,
    guided_weight: f64,
    normalization_scale: f64,
    intervals: Vec<(f64, f64)>,
}

#[derive(Clone, Copy, Debug)]
struct GridDirectionalPenetration {
    horizontal_grid: i64,
    vertical_grid: i64,
    horizontal_intervals: usize,
    vertical_intervals: usize,
}

impl GridDirectionalPenetration {
    fn penetration_mm(self) -> Option<f64> {
        let penetration = self.horizontal_grid.min(self.vertical_grid);
        (penetration > 0).then(|| from_grid(penetration as f64))
    }
}

pub fn improve_complete_layout(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    incumbent: &GeneralFastResult,
) -> Result<GeneralRelaxedOutcome, GeneralFastError> {
    improve_complete_layout_with_pinned_vacancy_parent(
        pieces,
        fast_settings,
        relaxed_settings,
        incumbent,
        None,
    )
}

pub fn improve_complete_layout_with_pinned_vacancy_parent(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    incumbent: &GeneralFastResult,
    pinned_vacancy_parent: Option<&GeneralPersistentVacancyPinnedParent>,
) -> Result<GeneralRelaxedOutcome, GeneralFastError> {
    validate_relaxed_settings(relaxed_settings)?;
    let mut diagnostics = GeneralRelaxedDiagnostics::default();
    if pieces.is_empty() {
        diagnostics.skipped_reason = Some("relaxed search requires at least one piece".to_owned());
        return Ok(GeneralRelaxedOutcome {
            result: incumbent.clone(),
            diagnostics,
        });
    }
    if pieces.iter().any(|piece| {
        piece
            .polygon
            .regions()
            .iter()
            .any(|region| !region.holes.is_empty())
    }) {
        diagnostics.skipped_reason =
            Some("relaxed search does not yet flatten hole topology".to_owned());
        if relaxed_settings.coupled_dynamic_separator {
            diagnostics.coupled_dynamic_separator = Some(run_coupled_dynamic_separator_experiment(
                pieces,
                fast_settings,
                relaxed_settings,
                incumbent,
                pinned_vacancy_parent,
            ));
        }
        return Ok(GeneralRelaxedOutcome {
            result: incumbent.clone(),
            diagnostics,
        });
    }

    let catalog_mode = if relaxed_settings.pressure_model
        == GeneralRelaxedPressureModel::DirectionalPenetration
    {
        SurrogateCatalogMode::CurrentAssignment
    } else if relaxed_settings.collision_backend == GeneralRelaxedCollisionBackend::RollbackTriangle
        || matches!(
            relaxed_settings.pressure_model,
            GeneralRelaxedPressureModel::StructuredTrianglePoles
        )
    {
        SurrogateCatalogMode::StructuredGrid
    } else {
        SurrogateCatalogMode::ZeroDegreeOnly
    };
    let (catalog, catalog_work) =
        match build_surrogate_catalog(pieces, fast_settings, catalog_mode, Some(incumbent)) {
            Ok(catalog) => catalog,
            Err(GeneralFastError::Geometry(error))
                if error.message().contains("relaxed surrogate") =>
            {
                diagnostics.skipped_reason = Some(error.to_string());
                return Ok(GeneralRelaxedOutcome {
                    result: incumbent.clone(),
                    diagnostics,
                });
            }
            Err(error) => return Err(error),
        };
    diagnostics.oriented_surrogate_builds = catalog_work.oriented_surrogate_builds;
    diagnostics.generated_cells = catalog_work.generated_cells;
    diagnostics.shared_pair_nfp_entries = catalog_work.shared_pair_nfp_entries;
    diagnostics.shared_pair_nfp_components = catalog_work.shared_pair_nfp_components;
    diagnostics.shared_pair_nfp_estimated_bytes = catalog_work.shared_pair_nfp_estimated_bytes;
    let mut protected = incumbent.clone();
    let mut working = initialize_complete_state(
        pieces,
        fast_settings,
        relaxed_settings.collision_backend,
        relaxed_settings.angle_seed_policy,
        relaxed_settings.pressure_model,
        incumbent,
    )?;
    let mut shrink_ratio = relaxed_settings.initial_shrink_ratio;
    let mut repair_successors_attempted = 0usize;
    for epoch in 0..relaxed_settings.epochs {
        diagnostics.epochs_attempted += 1;
        let incumbent_depth_before_mm = protected.used_long_axis_depth_mm;
        let protected_depth = protected
            .used_long_axis_depth_mm
            .max(working.strip_depth_mm);
        let target_depth = (protected.used_long_axis_depth_mm * (1.0 - shrink_ratio))
            .max(area_depth_lower_bound(pieces, fast_settings)?);
        let attempt_state = working.clone();
        let lane_result = if relaxed_settings.synchronize_lanes {
            run_synchronized_lanes(
                pieces,
                fast_settings,
                relaxed_settings,
                &attempt_state,
                target_depth,
                epoch,
                catalog.clone(),
            )
            .map(|lane| LaneBatch {
                counters: lane.counters,
                outcomes: vec![lane],
            })
        } else {
            run_independent_lanes(
                pieces,
                fast_settings,
                relaxed_settings,
                &attempt_state,
                target_depth,
                epoch,
                catalog.clone(),
            )
        };
        let batch = match lane_result {
            Ok(batch) => batch,
            Err(error) if is_directional_lane_unscorable(&error) => {
                diagnostics.directional_lane_rejections = diagnostics
                    .directional_lane_rejections
                    .saturating_add(relaxed_settings.lanes);
                diagnostics.skipped_reason = Some(error.to_string());
                return Ok(GeneralRelaxedOutcome {
                    result: protected,
                    diagnostics,
                });
            }
            Err(GeneralFastError::Geometry(error))
                if error.message().contains("relaxed surrogate") =>
            {
                diagnostics.skipped_reason = Some(error.to_string());
                return Ok(GeneralRelaxedOutcome {
                    result: protected,
                    diagnostics,
                });
            }
            Err(error) => return Err(error),
        };
        let mut selected =
            select_lane_for_publication(pieces, fast_settings, batch.outcomes, &mut diagnostics);
        selected.outcome.counters = batch.counters;
        let mut lane = selected.outcome;
        let mut exact_validation = selected.validation;
        if !lane.score.feasible()
            && repair_successors_attempted < relaxed_settings.angular_repair.successors
            && relaxed_settings.angular_repair.complete_query_budget > 0
        {
            let experiment = run_bounded_repair_experiment(
                pieces,
                fast_settings,
                relaxed_settings,
                &lane,
                epoch,
                catalog.clone(),
            )?;
            repair_successors_attempted = repair_successors_attempted.saturating_add(1);
            diagnostics.angular_repair_base_loss = Some(lane.score.common_loss());
            diagnostics.angular_repair_control_loss = Some(experiment.control_loss);
            diagnostics.angular_repair_rotation_loss = Some(experiment.rotation_loss);
            lane.counters.accumulate(experiment.counters);
            if let Some(selected) = experiment.selected {
                lane.state = selected.state;
                lane.score = selected.score;
                lane.weights = selected.weights;
                exact_validation =
                    validate_selected_lane(pieces, fast_settings, &lane, &mut diagnostics);
            }
        }
        diagnostics.ejection_chain_evaluations = diagnostics
            .ejection_chain_evaluations
            .saturating_add(lane.counters.ejection_chain_evaluations);
        diagnostics.ejection_chain_accepts = diagnostics
            .ejection_chain_accepts
            .saturating_add(lane.counters.ejection_chain_accepts);
        diagnostics.surrogate_evaluations = diagnostics
            .surrogate_evaluations
            .saturating_add(lane.counters.surrogate_evaluations);
        diagnostics.piece_broad_phase_probes = diagnostics
            .piece_broad_phase_probes
            .saturating_add(lane.counters.piece_broad_phase_probes);
        diagnostics.cell_index_probes = diagnostics
            .cell_index_probes
            .saturating_add(lane.counters.cell_index_probes);
        diagnostics.sat_tests = diagnostics
            .sat_tests
            .saturating_add(lane.counters.sat_tests);
        diagnostics.pair_nfp_builds = diagnostics
            .pair_nfp_builds
            .saturating_add(lane.counters.pair_nfp_builds);
        diagnostics.pair_nfp_components = diagnostics
            .pair_nfp_components
            .saturating_add(lane.counters.pair_nfp_components);
        diagnostics.shared_pair_nfp_adoptions = diagnostics
            .shared_pair_nfp_adoptions
            .saturating_add(lane.counters.shared_pair_nfp_adoptions);
        diagnostics.directional_pair_evaluations = diagnostics
            .directional_pair_evaluations
            .saturating_add(lane.counters.directional_pair_evaluations);
        diagnostics.directional_exact_confirmations = diagnostics
            .directional_exact_confirmations
            .saturating_add(lane.counters.directional_exact_confirmations);
        diagnostics.directional_cache_hits = diagnostics
            .directional_cache_hits
            .saturating_add(lane.counters.directional_cache_hits);
        diagnostics.directional_cache_misses = diagnostics
            .directional_cache_misses
            .saturating_add(lane.counters.directional_cache_misses);
        diagnostics.directional_component_visits = diagnostics
            .directional_component_visits
            .saturating_add(lane.counters.directional_component_visits);
        diagnostics.directional_intervals_produced = diagnostics
            .directional_intervals_produced
            .saturating_add(lane.counters.directional_intervals_produced);
        diagnostics.directional_intervals_merged = diagnostics
            .directional_intervals_merged
            .saturating_add(lane.counters.directional_intervals_merged);
        diagnostics.directional_over_budget_candidates = diagnostics
            .directional_over_budget_candidates
            .saturating_add(lane.counters.directional_over_budget_candidates);
        diagnostics.directional_zero_penetration_inconsistencies = diagnostics
            .directional_zero_penetration_inconsistencies
            .saturating_add(lane.counters.directional_zero_penetration_inconsistencies);
        diagnostics.directional_lane_rejections = diagnostics
            .directional_lane_rejections
            .saturating_add(lane.counters.directional_lane_rejections);
        diagnostics.directional_relocations = diagnostics
            .directional_relocations
            .saturating_add(lane.counters.directional_relocations);
        diagnostics.directional_rejected_contractions = diagnostics
            .directional_rejected_contractions
            .saturating_add(lane.counters.directional_rejected_contractions);
        diagnostics.directional_containment_rejections = diagnostics
            .directional_containment_rejections
            .saturating_add(lane.counters.directional_containment_rejections);
        diagnostics
            .directional_initial_pair_loss
            .merge(lane.counters.directional_initial_pair_loss);
        diagnostics
            .directional_initial_boundary_loss
            .merge(lane.counters.directional_initial_boundary_loss);
        diagnostics
            .directional_accepted_pair_loss
            .merge(lane.counters.directional_accepted_pair_loss);
        diagnostics
            .directional_accepted_boundary_loss
            .merge(lane.counters.directional_accepted_boundary_loss);
        diagnostics.axis_events = diagnostics
            .axis_events
            .saturating_add(lane.counters.axis_events);
        diagnostics.axis_candidate_evaluations = diagnostics
            .axis_candidate_evaluations
            .saturating_add(lane.counters.axis_candidate_evaluations);
        diagnostics.dynamic_hazard_queries = diagnostics
            .dynamic_hazard_queries
            .saturating_add(lane.counters.dynamic_hazard_queries);
        diagnostics.dynamic_hazard_updates = diagnostics
            .dynamic_hazard_updates
            .saturating_add(lane.counters.dynamic_hazard_updates);
        diagnostics.dynamic_pressure_evaluations = diagnostics
            .dynamic_pressure_evaluations
            .saturating_add(lane.counters.dynamic_pressure_evaluations);
        diagnostics.translation_evaluations = diagnostics
            .translation_evaluations
            .saturating_add(lane.counters.translation_evaluations);
        diagnostics.rotation_evaluations = diagnostics
            .rotation_evaluations
            .saturating_add(lane.counters.rotation_evaluations);
        diagnostics.retained_f64_confirmations = diagnostics
            .retained_f64_confirmations
            .saturating_add(lane.counters.retained_f64_confirmations);
        diagnostics.confirmed_pair_additions = diagnostics
            .confirmed_pair_additions
            .saturating_add(lane.counters.confirmed_pair_additions);
        diagnostics.confirmed_pair_removals = diagnostics
            .confirmed_pair_removals
            .saturating_add(lane.counters.confirmed_pair_removals);
        diagnostics.accepted_moves = diagnostics
            .accepted_moves
            .saturating_add(lane.counters.accepted_moves);
        diagnostics.angular_repair_successors = diagnostics
            .angular_repair_successors
            .saturating_add(lane.counters.angular_repair_successors);
        diagnostics.angular_repair_improvements = diagnostics
            .angular_repair_improvements
            .saturating_add(lane.counters.angular_repair_improvements);
        diagnostics.angular_repair_queries = diagnostics
            .angular_repair_queries
            .saturating_add(lane.counters.angular_repair_queries);
        let mut exact_valid = false;
        let mut exact_accepted = false;
        let mut retain_lane_state = false;
        if lane.score.feasible() {
            match exact_validation {
                ExactLaneValidation::Accepted {
                    placements,
                    metrics,
                } if placements.len() > protected.placements.len()
                    || (placements.len() == protected.placements.len()
                        && metrics.used_long_axis_depth_mm < protected.used_long_axis_depth_mm) =>
                {
                    exact_valid = true;
                    retain_lane_state = true;
                    protected.placements = placements;
                    protected.unplaced_piece_ids.clear();
                    protected.used_short_axis_span_mm = metrics.used_short_axis_span_mm;
                    protected.used_long_axis_depth_mm = metrics.used_long_axis_depth_mm;
                    protected.unused_short_axis_projection_mm =
                        metrics.unused_short_axis_projection_mm;
                    protected.occupied_envelope_area_mm2 = metrics.occupied_envelope_area_mm2;
                    working.strip_depth_mm = metrics.used_long_axis_depth_mm;
                    diagnostics.epochs_improved += 1;
                    exact_accepted = true;
                    shrink_ratio = relaxed_settings.initial_shrink_ratio;
                }
                ExactLaneValidation::Accepted { .. } => {
                    exact_valid = true;
                    retain_lane_state = true;
                    diagnostics.exact_valid_non_improvements =
                        diagnostics.exact_valid_non_improvements.saturating_add(1);
                    shrink_ratio = (shrink_ratio * 0.5).max(relaxed_settings.minimum_shrink_ratio);
                }
                ExactLaneValidation::Rejected | ExactLaneValidation::Infeasible => {
                    shrink_ratio = (shrink_ratio * 0.5).max(relaxed_settings.minimum_shrink_ratio);
                }
            }
        } else {
            shrink_ratio = (shrink_ratio * 0.75).max(relaxed_settings.minimum_shrink_ratio);
        }
        working = if retain_lane_state {
            lane.state.clone()
        } else {
            initialize_complete_state(
                pieces,
                fast_settings,
                relaxed_settings.collision_backend,
                relaxed_settings.angle_seed_policy,
                relaxed_settings.pressure_model,
                &protected,
            )?
        };
        diagnostics.epochs.push(GeneralRelaxedEpochDiagnostics {
            epoch,
            selected_lane: lane.selected_lane,
            restart_disruptions: lane.restart_disruptions,
            target_depth_mm: target_depth,
            weighted_loss: lane.score.weighted_loss,
            collision_pairs: lane.score.collision_pairs.len(),
            blocking_pairs: blocking_pair_diagnostics(pieces, &lane.score, &lane.weights),
            boundary_violations: lane.score.boundary_violations,
            boundary_piece_ids: lane
                .score
                .boundaries
                .iter()
                .enumerate()
                .filter(|(_, boundary)| boundary.violations > 0)
                .map(|(index, _)| pieces[index].id.to_owned())
                .collect(),
            surrogate_feasible: lane.score.feasible(),
            exact_valid,
            exact_accepted,
            translation_evaluations: lane.counters.translation_evaluations,
            rotation_evaluations: lane.counters.rotation_evaluations,
            complete_queries: lane.counters.dynamic_hazard_queries,
            retained_f64_confirmations: lane.counters.retained_f64_confirmations,
            accepted_moves: lane.counters.accepted_moves,
            incumbent_depth_before_mm,
            incumbent_depth_after_mm: protected.used_long_axis_depth_mm,
            incumbent_depth_delta_mm: incumbent_depth_before_mm - protected.used_long_axis_depth_mm,
        });
        if protected_depth <= protected.used_long_axis_depth_mm
            && shrink_ratio <= relaxed_settings.minimum_shrink_ratio
        {
            break;
        }
    }
    if relaxed_settings.coupled_dynamic_separator {
        diagnostics.coupled_dynamic_separator = Some(run_coupled_dynamic_separator_experiment(
            pieces,
            fast_settings,
            relaxed_settings,
            &protected,
            pinned_vacancy_parent,
        ));
    }
    Ok(GeneralRelaxedOutcome {
        result: protected,
        diagnostics,
    })
}

fn run_bounded_repair_experiment<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    base: &LaneOutcome,
    epoch: usize,
    catalog: Arc<SurrogateCatalog>,
) -> Result<RepairExperimentOutcome, GeneralFastError> {
    let total_budget = relaxed_settings.angular_repair.complete_query_budget;
    let control_budget = total_budget / 2;
    let rotation_budget = total_budget.saturating_sub(control_budget);
    let mut arm_settings = relaxed_settings;
    arm_settings.collision_backend = GeneralRelaxedCollisionBackend::DynamicHazard;
    arm_settings.angle_seed_policy = GeneralRelaxedAngleSeedPolicy::ContinuousUniform;
    arm_settings.pressure_model = GeneralRelaxedPressureModel::ContinuousTrianglePoles;
    arm_settings.angular_repair = GeneralAngularRepairSettings::disabled();

    let mut control = LaneSearch::new(
        pieces,
        fast_settings,
        arm_settings,
        derive_seed(relaxed_settings.seed, epoch, usize::MAX - 1),
        catalog.clone(),
    );
    control.weights = base.weights.clone();
    let control_outcome = control.run_repair_arm(
        base.state.clone(),
        false,
        relaxed_settings.angular_repair.neighborhood_size,
        control_budget,
        relaxed_settings.angular_repair.retained_confirmation_budget / 2,
    )?;

    let mut rotation = LaneSearch::new(
        pieces,
        fast_settings,
        arm_settings,
        derive_seed(relaxed_settings.seed, epoch, usize::MAX - 2),
        catalog,
    );
    rotation.weights = base.weights.clone();
    let rotation_outcome = rotation.run_repair_arm(
        base.state.clone(),
        true,
        relaxed_settings.angular_repair.neighborhood_size,
        rotation_budget,
        relaxed_settings
            .angular_repair
            .retained_confirmation_budget
            .saturating_sub(relaxed_settings.angular_repair.retained_confirmation_budget / 2),
    )?;

    let control_loss = control_outcome.score.common_loss();
    let rotation_loss = rotation_outcome.score.common_loss();
    let mut counters = WorkCounters::default();
    counters.accumulate(control_outcome.counters);
    counters.accumulate(rotation_outcome.counters);
    counters.angular_repair_successors = 1;
    counters.angular_repair_queries = counters.dynamic_hazard_queries;
    let best = if compare_lane_outcomes(0, &rotation_outcome, 1, &control_outcome) == Ordering::Less
    {
        rotation_outcome
    } else {
        control_outcome
    };
    let selected = if compare_chain_score(&best.score, &base.score) == Ordering::Less {
        counters.angular_repair_improvements = 1;
        Some(best)
    } else {
        None
    };
    Ok(RepairExperimentOutcome {
        selected,
        counters,
        control_loss,
        rotation_loss,
    })
}

fn run_coupled_dynamic_separator_experiment<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    protected: &GeneralFastResult,
    pinned_vacancy_parent: Option<&GeneralPersistentVacancyPinnedParent>,
) -> GeneralCoupledSeparatorDiagnostics {
    let skipped = coupled_separator_configuration_error(relaxed_settings);
    if let Some(reason) = skipped {
        return GeneralCoupledSeparatorDiagnostics {
            seed_domain: COUPLED_SEPARATOR_SEED_DOMAIN,
            control: skipped_coupled_separator_arm(
                CoupledSeparatorArm::Control,
                protected,
                reason.clone(),
            ),
            treatment: skipped_coupled_separator_arm(
                CoupledSeparatorArm::Treatment,
                protected,
                reason,
            ),
            boundary_projection_treatment: None,
            conflict_ruin_recreate: None,
            precompression_frontier_vacancy: None,
            persistent_vacancy_population: None,
        };
    }

    #[cfg(feature = "jagua-experimental")]
    {
        let catalog = match build_surrogate_catalog(
            pieces,
            fast_settings,
            SurrogateCatalogMode::ZeroDegreeOnly,
            Some(protected),
        ) {
            Ok((catalog, _)) => catalog,
            Err(error) => {
                let reason = format!("confirmation catalog: {error}");
                return GeneralCoupledSeparatorDiagnostics {
                    seed_domain: COUPLED_SEPARATOR_SEED_DOMAIN,
                    control: skipped_coupled_separator_arm(
                        CoupledSeparatorArm::Control,
                        protected,
                        reason.clone(),
                    ),
                    treatment: skipped_coupled_separator_arm(
                        CoupledSeparatorArm::Treatment,
                        protected,
                        reason,
                    ),
                    boundary_projection_treatment: None,
                    conflict_ruin_recreate: None,
                    precompression_frontier_vacancy: None,
                    persistent_vacancy_population: None,
                };
            }
        };
        let control = run_coupled_separator_arm(
            pieces,
            fast_settings,
            relaxed_settings,
            protected,
            CoupledSeparatorArm::Control,
            CoupledTerminalPolicy::None,
            catalog.clone(),
        );
        let treatment = run_coupled_separator_arm(
            pieces,
            fast_settings,
            relaxed_settings,
            protected,
            CoupledSeparatorArm::Treatment,
            CoupledTerminalPolicy::None,
            catalog.clone(),
        );
        let boundary_projection_treatment = run_coupled_separator_arm(
            pieces,
            fast_settings,
            relaxed_settings,
            protected,
            CoupledSeparatorArm::Treatment,
            CoupledTerminalPolicy::ExactBoundaryProjection,
            catalog,
        );
        let conflict_ruin_recreate = Some(run_conflict_ruin_recreate_experiment(
            pieces,
            fast_settings,
            relaxed_settings,
            &control,
            &treatment,
        ));
        let precompression_frontier_vacancy =
            (relaxed_settings.precompression_frontier_vacancy_mode > 0).then(|| {
                run_precompression_frontier_vacancy_experiment(
                    pieces,
                    fast_settings,
                    relaxed_settings,
                    &boundary_projection_treatment,
                    relaxed_settings.precompression_frontier_vacancy_mode,
                )
            });
        let persistent_vacancy_population =
            (relaxed_settings.persistent_vacancy_mode > 0).then(|| {
                // A pinned fixture parent replaces only the parent-layout
                // source; the compiled-in frozen fingerprint, depth, and dual
                // validation checks still gate the arm.
                let pinned_arm =
                    pinned_vacancy_parent.map(|pinned| GeneralCoupledSeparatorArmDiagnostics {
                        final_placements: coupled_placement_diagnostics(&pinned.placements),
                        ..GeneralCoupledSeparatorArmDiagnostics::default()
                    });
                let parent_source = pinned_vacancy_parent.map(|pinned| {
                    format!("pinnedFixture:{}#{}", pinned.source, pinned.source_sha256)
                });
                persistent_vacancy::run_persistent_vacancy_population(
                    pieces,
                    fast_settings,
                    relaxed_settings,
                    pinned_arm
                        .as_ref()
                        .unwrap_or(&boundary_projection_treatment.diagnostics),
                    parent_source,
                    relaxed_settings.persistent_vacancy_mode,
                )
            });
        GeneralCoupledSeparatorDiagnostics {
            seed_domain: COUPLED_SEPARATOR_SEED_DOMAIN,
            control: control.diagnostics,
            treatment: treatment.diagnostics,
            boundary_projection_treatment: Some(boundary_projection_treatment.diagnostics),
            conflict_ruin_recreate,
            precompression_frontier_vacancy,
            persistent_vacancy_population,
        }
    }
    #[cfg(not(feature = "jagua-experimental"))]
    {
        let _ = (pieces, fast_settings, pinned_vacancy_parent);
        let reason = "coupled dynamic separator requires the jagua-experimental feature".to_owned();
        GeneralCoupledSeparatorDiagnostics {
            seed_domain: COUPLED_SEPARATOR_SEED_DOMAIN,
            control: skipped_coupled_separator_arm(
                CoupledSeparatorArm::Control,
                protected,
                reason.clone(),
            ),
            treatment: skipped_coupled_separator_arm(
                CoupledSeparatorArm::Treatment,
                protected,
                reason,
            ),
            boundary_projection_treatment: None,
            conflict_ruin_recreate: None,
            precompression_frontier_vacancy: None,
            persistent_vacancy_population: None,
        }
    }
}

fn coupled_separator_configuration_error(settings: GeneralRelaxedSettings) -> Option<String> {
    let angular_disabled = settings.angular_repair.neighborhood_size == 0
        && settings.angular_repair.successors == 0
        && settings.angular_repair.complete_query_budget == 0
        && settings.angular_repair.retained_confirmation_budget == 0
        && settings.angular_repair.early_stop_queries == 0;
    if settings.collision_backend != GeneralRelaxedCollisionBackend::RollbackTriangle
        || settings.angle_seed_policy != GeneralRelaxedAngleSeedPolicy::StructuredGrid
        || settings.pressure_model != GeneralRelaxedPressureModel::StructuredTrianglePoles
        || settings.lanes != COUPLED_SEPARATOR_WORKERS
        || settings.sweeps_per_epoch != COUPLED_SEPARATOR_ROUNDS
        || settings.global_samples_per_move != 10
        || settings.focused_samples_per_move != 10
        || settings.refinement_rounds != 5
        || settings.synchronize_lanes
        || !angular_disabled
    {
        return Some(
            "coupled dynamic separator requires the protected 8-lane, 40-sweep, 10/10-sample, 5-refinement structured route"
                .to_owned(),
        );
    }
    None
}

fn skipped_coupled_separator_arm(
    arm: CoupledSeparatorArm,
    protected: &GeneralFastResult,
    reason: String,
) -> GeneralCoupledSeparatorArmDiagnostics {
    GeneralCoupledSeparatorArmDiagnostics {
        pressure_model: arm.label().to_owned(),
        initial_depth_mm: protected.used_long_axis_depth_mm,
        final_depth_mm: protected.used_long_axis_depth_mm,
        final_placement_fingerprint: Some(coupled_fast_placement_fingerprint(
            &protected.placements,
        )),
        final_placements: coupled_placement_diagnostics(&protected.placements),
        skipped_reason: Some(reason),
        ..GeneralCoupledSeparatorArmDiagnostics::default()
    }
}

#[cfg(feature = "jagua-experimental")]
fn run_coupled_separator_arm<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    protected: &GeneralFastResult,
    arm: CoupledSeparatorArm,
    terminal_policy: CoupledTerminalPolicy,
    catalog: Arc<SurrogateCatalog>,
) -> CoupledArmOutcome {
    let mut diagnostics = GeneralCoupledSeparatorArmDiagnostics {
        pressure_model: arm.label().to_owned(),
        attempted: true,
        initial_depth_mm: protected.used_long_axis_depth_mm,
        final_depth_mm: protected.used_long_axis_depth_mm,
        independently_measured_final_depth_mm: coupled_independent_source_depth(
            pieces,
            &protected.placements,
            fast_settings,
        )
        .ok(),
        final_placement_fingerprint: Some(coupled_fast_placement_fingerprint(
            &protected.placements,
        )),
        final_placements: coupled_placement_diagnostics(&protected.placements),
        ..GeneralCoupledSeparatorArmDiagnostics::default()
    };
    let experiment_seed = relaxed_settings.seed ^ COUPLED_SEPARATOR_SEED_DOMAIN;
    let mut incumbent = protected.clone();
    let mut checkpoint = None;
    let hazard_catalog = match JaguaHazardCatalog::new(pieces, fast_settings) {
        Ok(catalog) => Arc::new(catalog),
        Err(error) => {
            diagnostics.skipped_reason = Some(format!("dynamic hazard catalog: {error}"));
            return CoupledArmOutcome {
                diagnostics,
                checkpoint,
            };
        }
    };
    diagnostics.catalog_builds = 1;
    diagnostics.immutable_variant_builds = hazard_catalog.immutable_variant_count();

    for target_ordinal in 0..COUPLED_SEPARATOR_TARGETS {
        let target_seed = derive_seed(experiment_seed, target_ordinal, usize::MAX - 64);
        let compression_seed = derive_seed(target_seed, 0, usize::MAX - 63);
        let worker_seeds = (0..COUPLED_SEPARATOR_WORKERS)
            .map(|worker| derive_seed(target_seed, 0, worker))
            .collect::<Vec<_>>();
        let target_depth_mm =
            (incumbent.used_long_axis_depth_mm * (1.0 - COUPLED_SEPARATOR_CONTRACTION_RATIO)).max(
                area_depth_lower_bound(pieces, fast_settings)
                    .unwrap_or(incumbent.used_long_axis_depth_mm),
            );
        if target_depth_mm >= incumbent.used_long_axis_depth_mm {
            diagnostics.skipped_reason =
                Some("area lower bound prevents another contraction".to_owned());
            break;
        }
        diagnostics.targets_attempted = diagnostics.targets_attempted.saturating_add(1);
        let compression_split_mm = incumbent.used_long_axis_depth_mm * 0.5;
        let base_state = match initialize_complete_state(
            pieces,
            fast_settings,
            GeneralRelaxedCollisionBackend::DynamicHazard,
            GeneralRelaxedAngleSeedPolicy::ContinuousUniform,
            arm.pressure_model(),
            &incumbent,
        )
        .and_then(|state| {
            compress_state_at_split(&state, target_depth_mm, compression_split_mm, pieces)
        }) {
            Ok(state) => state,
            Err(error) => {
                let failure_reason = format!("target initialization: {error}");
                let incumbent_fingerprint =
                    coupled_fast_placement_fingerprint(&incumbent.placements);
                diagnostics
                    .targets
                    .push(GeneralCoupledSeparatorTargetDiagnostics {
                        ordinal: target_ordinal,
                        target_depth_mm,
                        compression_split_mm,
                        target_seed,
                        compression_seed,
                        worker_seeds,
                        initial_state_fingerprint: incumbent_fingerprint.clone(),
                        final_state_fingerprint: incumbent_fingerprint,
                        rounds: 0,
                        strikes: 0,
                        rollbacks: 0,
                        full_rescore_agreements: 0,
                        initial_raw_loss: 0.0,
                        minimum_raw_loss: 0.0,
                        final_raw_loss: 0.0,
                        final_weighted_loss: 0.0,
                        feasible: false,
                        exact_valid: false,
                        exact_accepted: false,
                        exact_rejection_reason: None,
                        accepted_depth_mm: None,
                        boundary_projection: None,
                        cap_exhausted: None,
                        failure_reason: Some(failure_reason.clone()),
                    });
                diagnostics.skipped_reason = Some(failure_reason);
                break;
            }
        };

        let mut arm_settings = relaxed_settings;
        arm_settings.collision_backend = GeneralRelaxedCollisionBackend::DynamicHazard;
        arm_settings.angle_seed_policy = GeneralRelaxedAngleSeedPolicy::ContinuousUniform;
        arm_settings.pressure_model = arm.pressure_model();
        arm_settings.angular_repair = GeneralAngularRepairSettings::disabled();
        arm_settings.synchronize_lanes = true;
        arm_settings.sweeps_per_epoch = COUPLED_SEPARATOR_ROUNDS;

        let target = run_coupled_separator_target(
            pieces,
            fast_settings,
            arm_settings,
            &incumbent,
            base_state,
            target_ordinal,
            target_depth_mm,
            compression_split_mm,
            target_seed,
            compression_seed,
            worker_seeds,
            arm,
            CoupledRollbackRescorePolicy::StrictDerivedAgreement,
            false,
            catalog.clone(),
            hazard_catalog.clone(),
        );
        let CoupledTargetOutcome {
            diagnostics: mut target_diagnostics,
            mut accepted,
            work: counters,
            minimum,
            ..
        } = match target {
            Ok(outcome) => outcome,
            Err(error) => {
                diagnostics.skipped_reason = Some(format!("target {target_ordinal}: {error}"));
                break;
            }
        };
        diagnostics.worker_sweeps = diagnostics
            .worker_sweeps
            .saturating_add(counters.worker_sweeps);
        diagnostics.dynamic_queries = diagnostics
            .dynamic_queries
            .saturating_add(counters.dynamic_queries);
        diagnostics.pressure_evaluations = diagnostics
            .pressure_evaluations
            .saturating_add(counters.pressure_evaluations);
        diagnostics.retained_confirmations = diagnostics
            .retained_confirmations
            .saturating_add(counters.retained_confirmations);
        diagnostics.hazard_updates = diagnostics
            .hazard_updates
            .saturating_add(counters.hazard_updates);
        diagnostics.layout_loads = diagnostics
            .layout_loads
            .saturating_add(counters.layout_loads);
        diagnostics.index_builds = diagnostics
            .index_builds
            .saturating_add(counters.index_builds);
        diagnostics.worker_full_score_pair_visits = diagnostics
            .worker_full_score_pair_visits
            .saturating_add(counters.worker_full_score_pair_visits);
        diagnostics.auditor_full_score_pair_visits = diagnostics
            .auditor_full_score_pair_visits
            .saturating_add(counters.auditor_full_score_pair_visits);
        diagnostics.auditor_dynamic_queries = diagnostics
            .auditor_dynamic_queries
            .saturating_add(counters.auditor_dynamic_queries);
        diagnostics.auditor_pressure_evaluations = diagnostics
            .auditor_pressure_evaluations
            .saturating_add(counters.auditor_pressure_evaluations);
        diagnostics.auditor_layout_loads = diagnostics
            .auditor_layout_loads
            .saturating_add(counters.auditor_layout_loads);
        diagnostics.auditor_index_builds = diagnostics
            .auditor_index_builds
            .saturating_add(counters.auditor_index_builds);
        if accepted.is_none()
            && terminal_policy == CoupledTerminalPolicy::ExactBoundaryProjection
            && target_diagnostics.failure_reason.is_none()
            && target_diagnostics.cap_exhausted.is_none()
        {
            if let Some(minimum) = minimum.as_ref() {
                let projection_checkpoint = CoupledFailedCheckpoint {
                    incumbent: incumbent.clone(),
                    target_ordinal,
                    target_depth_mm,
                    compression_split_mm,
                    target_seed,
                    compression_seed,
                    catalog: catalog.clone(),
                    hazard_catalog: hazard_catalog.clone(),
                    minimum: minimum.clone(),
                    attempt_diagnostics: target_diagnostics.clone(),
                };
                let (projection_diagnostics, projected) =
                    try_exact_boundary_projection(&projection_checkpoint, pieces, fast_settings);
                target_diagnostics.boundary_projection = Some(projection_diagnostics);
                accepted = projected;
            }
        }
        let cap_exhausted = target_diagnostics.cap_exhausted.is_some();
        let target_failure = target_diagnostics.failure_reason.clone();
        diagnostics.targets.push(target_diagnostics.clone());
        if let Some(reason) = target_failure {
            diagnostics.skipped_reason = Some(format!("target {target_ordinal}: {reason}"));
            break;
        }
        if cap_exhausted {
            diagnostics.skipped_reason = Some(format!(
                "target {target_ordinal} crossed an atomic experiment cap"
            ));
            break;
        }
        let Some(accepted) = accepted else {
            if let Some(minimum) = minimum {
                checkpoint = Some(CoupledFailedCheckpoint {
                    incumbent: incumbent.clone(),
                    target_ordinal,
                    target_depth_mm,
                    compression_split_mm,
                    target_seed,
                    compression_seed,
                    catalog: catalog.clone(),
                    hazard_catalog: hazard_catalog.clone(),
                    minimum,
                    attempt_diagnostics: target_diagnostics,
                });
            }
            break;
        };
        diagnostics.targets_accepted = diagnostics.targets_accepted.saturating_add(1);
        diagnostics.final_depth_mm = accepted.used_long_axis_depth_mm;
        diagnostics.independently_measured_final_depth_mm =
            coupled_independent_source_depth(pieces, &accepted.placements, fast_settings).ok();
        diagnostics.final_placement_fingerprint =
            Some(coupled_fast_placement_fingerprint(&accepted.placements));
        diagnostics.final_placements = coupled_placement_diagnostics(&accepted.placements);
        incumbent = accepted;
    }
    CoupledArmOutcome {
        diagnostics,
        checkpoint,
    }
}

#[cfg(feature = "jagua-experimental")]
fn try_exact_boundary_projection(
    checkpoint: &CoupledFailedCheckpoint,
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
) -> (
    GeneralBoundaryProjectionDiagnostics,
    Option<GeneralFastResult>,
) {
    let mut diagnostics = GeneralBoundaryProjectionDiagnostics {
        attempted: true,
        ..GeneralBoundaryProjectionDiagnostics::default()
    };
    let minimum = &checkpoint.minimum;
    if minimum.score.boundary_loss <= 0.0 {
        diagnostics.rejection_reason =
            Some("terminal state has no positive boundary loss".to_owned());
        return (diagnostics, None);
    }
    if !minimum.score.collision_pairs.is_empty() {
        diagnostics.rejection_reason = Some(
            "terminal state is not boundary-only; exact projection is intentionally narrow"
                .to_owned(),
        );
        return (diagnostics, None);
    }
    let mut projected = minimum.state.clone();
    projected.strip_depth_mm = checkpoint.target_depth_mm;
    let projections = match select_all_exact_boundary_projections(
        &projected,
        &minimum.score,
        pieces,
        fast_settings,
    ) {
        Ok(projections) if !projections.is_empty() => projections,
        Ok(_) => {
            diagnostics.rejection_reason =
                Some("terminal state has no projectable boundary pieces".to_owned());
            return (diagnostics, None);
        }
        Err(reason) => {
            diagnostics.rejection_reason = Some(reason);
            return (diagnostics, None);
        }
    };
    let (root, root_placement) = &projections[0];
    diagnostics.root_piece_id = Some(pieces[*root].id.to_owned());
    diagnostics.root_boundary_loss = Some(minimum.score.boundaries[*root].raw_loss);
    diagnostics.projected_pose = Some(GeneralCoupledSeparatorPlacementDiagnostics {
        piece_id: pieces[*root].id.to_owned(),
        rotation_deg: root_placement.rotation_deg,
        mirrored: root_placement.mirrored,
        translate_short_axis: root_placement.translate_x,
        translate_long_axis: root_placement.translate_y,
    });
    diagnostics.projected_pieces = projections
        .iter()
        .map(
            |(piece_index, placement)| GeneralCoupledSeparatorPlacementDiagnostics {
                piece_id: pieces[*piece_index].id.to_owned(),
                rotation_deg: placement.rotation_deg,
                mirrored: placement.mirrored,
                translate_short_axis: placement.translate_x,
                translate_long_axis: placement.translate_y,
            },
        )
        .collect();
    for (piece_index, placement) in projections {
        projected.placements[piece_index] = placement;
    }
    diagnostics.state_fingerprint = Some(coupled_state_fingerprint(&projected));
    let placements = to_fast_placements(&projected, pieces);
    let metrics = match validate_and_measure_placements(pieces, &placements, fast_settings) {
        Ok(metrics) => metrics,
        Err(error) => {
            diagnostics.rejection_reason = Some(error.to_string());
            return (diagnostics, None);
        }
    };
    diagnostics.exact_valid = true;
    diagnostics.exact_depth_mm = Some(metrics.used_long_axis_depth_mm);
    if metrics.used_long_axis_depth_mm >= checkpoint.incumbent.used_long_axis_depth_mm {
        diagnostics.rejection_reason =
            Some("exact-valid projection did not improve the incumbent".to_owned());
        return (diagnostics, None);
    }

    diagnostics.exact_accepted = true;
    let mut result = checkpoint.incumbent.clone();
    result.placements = placements;
    result.unplaced_piece_ids.clear();
    result.used_short_axis_span_mm = metrics.used_short_axis_span_mm;
    result.used_long_axis_depth_mm = metrics.used_long_axis_depth_mm;
    result.unused_short_axis_projection_mm = metrics.unused_short_axis_projection_mm;
    result.occupied_envelope_area_mm2 = metrics.occupied_envelope_area_mm2;
    (diagnostics, Some(result))
}

#[cfg(feature = "jagua-experimental")]
fn run_precompression_frontier_vacancy_experiment<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    boundary_projection: &CoupledArmOutcome,
    mode: usize,
) -> GeneralPrecompressionFrontierVacancyDiagnostics {
    let mut diagnostics = GeneralPrecompressionFrontierVacancyDiagnostics {
        mode,
        attempted: true,
        ..GeneralPrecompressionFrontierVacancyDiagnostics::default()
    };
    if mode == 3 {
        diagnostics.validation_counts = Some(GeneralPrecompressionValidationDiagnostics::default());
    }
    let Some(checkpoint) = boundary_projection.checkpoint.as_ref() else {
        diagnostics.skipped_reason =
            Some("exact-boundary arm retained no failed checkpoint".to_owned());
        return diagnostics;
    };
    diagnostics.target_depth_mm = Some(checkpoint.target_depth_mm);
    diagnostics.incumbent_strip_depth_mm = Some(checkpoint.incumbent.used_long_axis_depth_mm);
    diagnostics.checkpoint_fingerprint = Some(conflict_ruin_checkpoint_fingerprint(checkpoint));
    diagnostics.control = Some(checkpoint.attempt_diagnostics.clone());
    if checkpoint.attempt_diagnostics.failure_reason.is_some()
        || checkpoint.attempt_diagnostics.cap_exhausted.is_some()
    {
        diagnostics.skipped_reason = Some("failed target was not an uncapped outcome".to_owned());
        return diagnostics;
    }
    if checkpoint.target_depth_mm >= checkpoint.incumbent.used_long_axis_depth_mm {
        diagnostics.skipped_reason =
            Some("failed target does not contract the incumbent collision strip".to_owned());
        return diagnostics;
    }
    if let Err(error) =
        validate_and_measure_placements(pieces, &checkpoint.incumbent.placements, fast_settings)
    {
        diagnostics.skipped_reason = Some(format!("incumbent publication validation: {error}"));
        return diagnostics;
    }
    if let Some(counts) = diagnostics.validation_counts.as_mut() {
        counts.incumbent = 1;
        counts.total = 1;
    }

    let failed_score = match precompression_full_score(
        pieces,
        fast_settings,
        relaxed_settings,
        checkpoint,
        &checkpoint.minimum.state,
        &mut diagnostics,
    ) {
        Ok(score) => score,
        Err(reason) => {
            diagnostics.skipped_reason = Some(reason);
            return diagnostics;
        }
    };
    if let Some(disagreement) = raw_tracker_disagreement(&failed_score, &checkpoint.minimum.score) {
        diagnostics.skipped_reason = Some(format!(
            "failed checkpoint disagrees with a complete rescore: {disagreement}"
        ));
        return diagnostics;
    }
    if failed_score.boundary_loss <= 0.0 || !failed_score.collision_pairs.is_empty() {
        diagnostics.skipped_reason =
            Some("failed checkpoint is not strictly boundary-only".to_owned());
        return diagnostics;
    }
    let projections = match select_all_exact_boundary_projections(
        &checkpoint.minimum.state,
        &failed_score,
        pieces,
        fast_settings,
    ) {
        Ok(projections) if projections.len() == CONFLICT_RUIN_REMOVED_PIECES => projections,
        Ok(projections) => {
            diagnostics.skipped_reason = Some(format!(
                "selector requires exactly {CONFLICT_RUIN_REMOVED_PIECES} boundary offenders, found {}",
                projections.len()
            ));
            return diagnostics;
        }
        Err(reason) => {
            diagnostics.skipped_reason = Some(format!("boundary selector: {reason}"));
            return diagnostics;
        }
    };
    let removal_order = projections
        .iter()
        .map(|(piece_index, _)| *piece_index)
        .collect::<Vec<_>>();
    diagnostics.selected_piece_ids = removal_order
        .iter()
        .map(|piece_index| pieces[*piece_index].id.to_owned())
        .collect();

    let parent_state = match initialize_complete_state(
        pieces,
        fast_settings,
        GeneralRelaxedCollisionBackend::DynamicHazard,
        GeneralRelaxedAngleSeedPolicy::ContinuousUniform,
        GeneralRelaxedPressureModel::DynamicPoles,
        &checkpoint.incumbent,
    ) {
        Ok(state) => state,
        Err(error) => {
            diagnostics.skipped_reason = Some(format!("incumbent state: {error}"));
            return diagnostics;
        }
    };
    let incumbent_fingerprint = coupled_state_fingerprint(&parent_state);
    diagnostics.incumbent_parent_fingerprint = Some(incumbent_fingerprint.clone());
    let rebuild_started = Instant::now();
    let rebuilt = build_conflict_ruin_states(
        &parent_state,
        parent_state.strip_depth_mm,
        &checkpoint.hazard_catalog,
        pieces,
        fast_settings,
        relaxed_settings.seed ^ PRECOMPRESSION_FRONTIER_SEED_DOMAIN,
        &removal_order,
        &mut diagnostics.rebuild,
    );
    diagnostics.rebuild.elapsed_ms = rebuild_started.elapsed().as_secs_f64() * 1_000.0;
    let rebuilt = match rebuilt {
        Ok(rebuilt) => rebuilt,
        Err(reason) => {
            diagnostics.rebuild.cap_exhausted = reason
                .strip_prefix("cap: ")
                .map(str::to_owned)
                .or_else(|| diagnostics.rebuild.cap_exhausted.clone());
            diagnostics.skipped_reason = Some(reason);
            return diagnostics;
        }
    };

    let mut exact_parent_candidates = Vec::new();
    let mut infeasible_candidates = Vec::new();
    for (beam_ordinal, child) in rebuilt
        .into_iter()
        .take(CONFLICT_RUIN_BEAM_WIDTH)
        .enumerate()
    {
        let state = child.state;
        let fingerprint = coupled_state_fingerprint(&state);
        let score = match precompression_full_score(
            pieces,
            fast_settings,
            relaxed_settings,
            checkpoint,
            &state,
            &mut diagnostics,
        ) {
            Ok(score) => score,
            Err(reason) => {
                diagnostics.skipped_reason = Some(reason);
                return diagnostics;
            }
        };
        let placements = to_fast_placements(&state, pieces);
        let publication = validate_and_measure_placements(pieces, &placements, fast_settings);
        if let Some(counts) = diagnostics.validation_counts.as_mut() {
            counts.rebuilt_children = counts.rebuilt_children.saturating_add(1);
            counts.total = counts.total.saturating_add(1);
        }
        diagnostics
            .rebuilt_children
            .push(GeneralPrecompressionFrontierChildDiagnostics {
                beam_ordinal,
                fingerprint: fingerprint.clone(),
                exact_overlap_area_mm2: child.score.total_overlap_area_mm2,
                exact_positive_overlap_pairs: child.score.positive_overlap_pairs,
                frontier_depth_mm: child.score.frontier_depth_mm,
                fresh_raw_loss: score.common_loss(),
                fresh_positive_pairs: score.collision_pairs.len(),
                fresh_feasible: score.feasible(),
                publication_valid: publication.is_ok(),
                publication_rejection_reason: publication.as_ref().err().map(ToString::to_string),
            });
        if fingerprint == incumbent_fingerprint {
            continue;
        }
        match publication {
            Ok(metrics) => {
                let compressed = match compress_state_at_split(
                    &state,
                    checkpoint.target_depth_mm,
                    checkpoint.compression_split_mm,
                    pieces,
                ) {
                    Ok(compressed) => compressed,
                    Err(error) => {
                        diagnostics.skipped_reason = Some(format!("parent compression: {error}"));
                        return diagnostics;
                    }
                };
                let compressed_score = match precompression_full_score(
                    pieces,
                    fast_settings,
                    relaxed_settings,
                    checkpoint,
                    &compressed,
                    &mut diagnostics,
                ) {
                    Ok(score) => score,
                    Err(reason) => {
                        diagnostics.skipped_reason = Some(reason);
                        return diagnostics;
                    }
                };
                exact_parent_candidates.push(PrecompressionExactParentCandidate {
                    compressed,
                    metrics,
                    compressed_raw_loss: compressed_score.common_loss(),
                    frontier_depth_mm: child.score.frontier_depth_mm,
                    fingerprint,
                });
            }
            Err(_) if mode == 3 && !score.feasible() => {
                infeasible_candidates.push(PrecompressionInfeasibleChild {
                    state,
                    fresh_raw_loss: score.common_loss(),
                    fresh_positive_pairs: score.collision_pairs.len(),
                    beam_ordinal,
                    fingerprint,
                });
            }
            Err(_) => {}
        }
    }
    if mode == 3 {
        diagnostics.rebuilt_child_record_hash = Some(precompression_child_record_hash(
            &diagnostics.rebuilt_children,
        ));
        if diagnostics.validation_counts.is_some_and(|counts| {
            counts.rebuilt_children > CONFLICT_RUIN_BEAM_WIDTH || counts.total > 7
        }) {
            diagnostics.skipped_reason =
                Some("cap: pre-compression validation budget exhausted".to_owned());
            return diagnostics;
        }
    }
    exact_parent_candidates.sort_by(|first, second| {
        first
            .compressed_raw_loss
            .total_cmp(&second.compressed_raw_loss)
            .then_with(|| {
                first
                    .metrics
                    .used_long_axis_depth_mm
                    .total_cmp(&second.metrics.used_long_axis_depth_mm)
            })
            .then_with(|| first.frontier_depth_mm.total_cmp(&second.frontier_depth_mm))
            .then_with(|| first.fingerprint.cmp(&second.fingerprint))
    });
    diagnostics.eligible_parent_fingerprints = exact_parent_candidates
        .iter()
        .map(|candidate| candidate.fingerprint.clone())
        .collect();
    if mode == 3 {
        infeasible_candidates.sort_by(|first, second| {
            first
                .fresh_raw_loss
                .total_cmp(&second.fresh_raw_loss)
                .then_with(|| first.fresh_positive_pairs.cmp(&second.fresh_positive_pairs))
                .then_with(|| first.beam_ordinal.cmp(&second.beam_ordinal))
                .then_with(|| first.fingerprint.cmp(&second.fingerprint))
        });
        let Some(selected) = infeasible_candidates.into_iter().next() else {
            diagnostics.skipped_reason = Some(
                "rebuild produced no distinct fresh-infeasible publication-invalid child"
                    .to_owned(),
            );
            return diagnostics;
        };
        diagnostics.selected_parent_fingerprint = Some(selected.fingerprint.clone());
        diagnostics.rebuild.selected_state_fingerprint = Some(selected.fingerprint.clone());
        run_precompression_infeasible_handoff(
            pieces,
            fast_settings,
            relaxed_settings,
            checkpoint,
            selected.state,
            &mut diagnostics,
        );
        return diagnostics;
    }
    let Some(selected) = exact_parent_candidates.into_iter().next() else {
        diagnostics.skipped_reason =
            Some("rebuild produced no distinct authoritative exact-valid parent".to_owned());
        return diagnostics;
    };
    diagnostics.selected_parent_fingerprint = Some(selected.fingerprint.clone());
    diagnostics.selected_parent_depth_mm = Some(selected.metrics.used_long_axis_depth_mm);
    diagnostics.selected_compressed_raw_loss = Some(selected.compressed_raw_loss);
    diagnostics.rebuild.selected_state_fingerprint = Some(selected.fingerprint.clone());
    if mode == 1 {
        diagnostics.skipped_reason =
            Some("mode one froze the direct exact-valid-parent candidate".to_owned());
        return diagnostics;
    }
    if mode != 2 {
        diagnostics.skipped_reason = Some("unsupported pre-compression mode".to_owned());
        return diagnostics;
    }
    run_precompression_stage_b(
        pieces,
        fast_settings,
        relaxed_settings,
        checkpoint,
        selected.compressed,
        &mut diagnostics,
    );
    diagnostics
}

#[cfg(feature = "jagua-experimental")]
fn precompression_child_record_hash(
    children: &[GeneralPrecompressionFrontierChildDiagnostics],
) -> String {
    let mut digest = Sha256::new();
    for child in children {
        digest.update(child.beam_ordinal.to_le_bytes());
        digest.update((child.fingerprint.len() as u64).to_le_bytes());
        digest.update(child.fingerprint.as_bytes());
        digest.update(child.exact_overlap_area_mm2.to_bits().to_le_bytes());
        digest.update(child.exact_positive_overlap_pairs.to_le_bytes());
        digest.update(child.frontier_depth_mm.to_bits().to_le_bytes());
        digest.update(child.fresh_raw_loss.to_bits().to_le_bytes());
        digest.update(child.fresh_positive_pairs.to_le_bytes());
        digest.update([u8::from(child.fresh_feasible)]);
        digest.update([u8::from(child.publication_valid)]);
        if let Some(reason) = &child.publication_rejection_reason {
            digest.update([1]);
            digest.update((reason.len() as u64).to_le_bytes());
            digest.update(reason.as_bytes());
        } else {
            digest.update([0]);
        }
    }
    format!("{:x}", digest.finalize())
}

#[cfg(feature = "jagua-experimental")]
fn run_precompression_infeasible_handoff<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    checkpoint: &CoupledFailedCheckpoint,
    selected_state: RelaxedState,
    diagnostics: &mut GeneralPrecompressionFrontierVacancyDiagnostics,
) {
    let stage_a_target_seed = derive_seed(
        relaxed_settings.seed ^ PRECOMPRESSION_HANDOFF_SEED_DOMAIN,
        checkpoint.target_ordinal,
        usize::MAX - 72,
    );
    let stage_a_compression_seed = derive_seed(stage_a_target_seed, 0, usize::MAX - 71);
    let stage_a_worker_seeds = (0..COUPLED_SEPARATOR_WORKERS)
        .map(|worker| derive_seed(stage_a_target_seed, 0, worker))
        .collect::<Vec<_>>();
    diagnostics.stage_a_seed_domain = Some(PRECOMPRESSION_HANDOFF_SEED_DOMAIN);
    diagnostics.stage_a_target_seed = Some(stage_a_target_seed);
    diagnostics.stage_a_compression_seed = Some(stage_a_compression_seed);
    diagnostics.stage_a_worker_seeds = stage_a_worker_seeds.clone();

    let stage_a_initial_fingerprint = coupled_state_fingerprint(&selected_state);
    let stage_a_started = Instant::now();
    let stage_a = run_coupled_separator_target(
        pieces,
        fast_settings,
        precompression_arm_settings(relaxed_settings),
        &checkpoint.incumbent,
        selected_state,
        checkpoint
            .target_ordinal
            .saturating_add(COUPLED_SEPARATOR_TARGETS),
        checkpoint.incumbent.used_long_axis_depth_mm,
        checkpoint.compression_split_mm,
        stage_a_target_seed,
        stage_a_compression_seed,
        stage_a_worker_seeds,
        CoupledSeparatorArm::Treatment,
        CoupledRollbackRescorePolicy::CanonicalAuthoritativeRows,
        true,
        checkpoint.catalog.clone(),
        checkpoint.hazard_catalog.clone(),
    );
    let stage_a_elapsed_ms = stage_a_started.elapsed().as_secs_f64() * 1_000.0;
    let stage_a = match stage_a {
        Ok(outcome) => outcome,
        Err(error) => {
            diagnostics.stage_a = Some(GeneralConflictRuinArmDiagnostics {
                attempted: true,
                applied_rebuild: true,
                elapsed_ms: stage_a_elapsed_ms,
                initial_state_fingerprint: Some(stage_a_initial_fingerprint),
                failure_reason: Some(error.to_string()),
                ..GeneralConflictRuinArmDiagnostics::default()
            });
            return;
        }
    };
    diagnostics.stage_a = Some(precompression_target_arm_diagnostics(
        &stage_a,
        stage_a_initial_fingerprint,
        stage_a_elapsed_ms,
        true,
    ));
    if let Some(audit) = &stage_a.independent_audit {
        diagnostics.stage_a_independent_audit = Some(audit.diagnostics.clone());
        if let Some(counts) = diagnostics.validation_counts.as_mut() {
            counts.stage_a = audit.diagnostics.independent_audit_count;
        }
    } else if stage_a.exact_metrics.is_some() {
        if let Some(counts) = diagnostics.validation_counts.as_mut() {
            counts.stage_a = 1;
        }
    }
    if let Some(counts) = diagnostics.validation_counts.as_mut() {
        counts.total = counts
            .incumbent
            .saturating_add(counts.rebuilt_children)
            .saturating_add(counts.stage_a);
    }
    if stage_a.diagnostics.failure_reason.is_some() || stage_a.diagnostics.cap_exhausted.is_some() {
        diagnostics.skipped_reason = Some("Stage A ended with a failure or cap".to_owned());
        return;
    }
    if let Some(reason) = precompression_handoff_work_cap_reason(stage_a.work) {
        diagnostics.skipped_reason = Some(reason);
        return;
    }
    if diagnostics
        .validation_counts
        .is_some_and(|counts| counts.stage_a > 1 || counts.total > 7)
    {
        diagnostics.skipped_reason =
            Some("cap: pre-compression validation budget exhausted".to_owned());
        return;
    }
    let stage_a_metrics = stage_a.exact_metrics.or_else(|| {
        stage_a
            .independent_audit
            .as_ref()
            .and_then(|audit| audit.metrics)
    });
    let Some(stage_a_metrics) = stage_a_metrics else {
        diagnostics.skipped_reason = Some("Stage A did not produce exact-valid metrics".to_owned());
        return;
    };
    let stage_a_placements = to_fast_placements(&stage_a.final_state, pieces);
    let stage_a_placement_fingerprint = coupled_fast_placement_fingerprint(&stage_a_placements);
    if stage_a_placement_fingerprint
        == coupled_fast_placement_fingerprint(&checkpoint.incumbent.placements)
    {
        diagnostics.skipped_reason =
            Some("Stage A restored the original incumbent placement".to_owned());
        return;
    }
    diagnostics.selected_parent_depth_mm = Some(stage_a_metrics.used_long_axis_depth_mm);
    let stage_a_parent =
        fast_result_from_exact_state(&checkpoint.incumbent, stage_a_placements, stage_a_metrics);
    let stage_b_initial = match compress_state_at_split(
        &stage_a.final_state,
        checkpoint.target_depth_mm,
        checkpoint.compression_split_mm,
        pieces,
    ) {
        Ok(state) => state,
        Err(error) => {
            diagnostics.skipped_reason = Some(format!("Stage B compression: {error}"));
            return;
        }
    };
    let stage_b_initial_fingerprint = coupled_state_fingerprint(&stage_b_initial);
    let stage_b_started = Instant::now();
    let stage_b = run_coupled_separator_target(
        pieces,
        fast_settings,
        precompression_arm_settings(relaxed_settings),
        &stage_a_parent,
        stage_b_initial,
        checkpoint.target_ordinal,
        checkpoint.target_depth_mm,
        checkpoint.compression_split_mm,
        checkpoint.target_seed,
        checkpoint.compression_seed,
        checkpoint.attempt_diagnostics.worker_seeds.clone(),
        CoupledSeparatorArm::Treatment,
        CoupledRollbackRescorePolicy::CanonicalAuthoritativeRows,
        false,
        checkpoint.catalog.clone(),
        checkpoint.hazard_catalog.clone(),
    );
    let stage_b_elapsed_ms = stage_b_started.elapsed().as_secs_f64() * 1_000.0;
    let stage_b = match stage_b {
        Ok(outcome) => outcome,
        Err(error) => {
            diagnostics.treatment = GeneralConflictRuinArmDiagnostics {
                attempted: true,
                applied_rebuild: true,
                elapsed_ms: stage_b_elapsed_ms,
                initial_state_fingerprint: Some(stage_b_initial_fingerprint),
                failure_reason: Some(error.to_string()),
                ..GeneralConflictRuinArmDiagnostics::default()
            };
            return;
        }
    };
    diagnostics.treatment = precompression_target_arm_diagnostics(
        &stage_b,
        stage_b_initial_fingerprint,
        stage_b_elapsed_ms,
        true,
    );
    if stage_b.diagnostics.failure_reason.is_none()
        && stage_b.diagnostics.cap_exhausted.is_none()
        && stage_b.diagnostics.feasible
    {
        if let Some(counts) = diagnostics.validation_counts.as_mut() {
            counts.stage_b = 1;
        }
    }
    if let Some(counts) = diagnostics.validation_counts.as_mut() {
        counts.total = counts
            .incumbent
            .saturating_add(counts.rebuilt_children)
            .saturating_add(counts.stage_a)
            .saturating_add(counts.stage_b);
    }
    let mut aggregate_work = stage_a.work;
    aggregate_work.accumulate(stage_b.work);
    if let Some(reason) = precompression_handoff_work_cap_reason(aggregate_work) {
        diagnostics.skipped_reason = Some(reason);
        return;
    }
    if diagnostics
        .validation_counts
        .is_some_and(|counts| counts.stage_b > 1 || counts.total > 7)
    {
        diagnostics.skipped_reason =
            Some("cap: pre-compression validation budget exhausted".to_owned());
        return;
    }
    diagnostics.mechanism_passed = !checkpoint.attempt_diagnostics.feasible
        && !checkpoint.attempt_diagnostics.exact_valid
        && stage_b.diagnostics.exact_valid
        && stage_b.accepted.is_some();
    if !diagnostics.mechanism_passed {
        diagnostics.skipped_reason =
            Some("Stage B did not accept the failed contraction".to_owned());
    }
}

#[cfg(feature = "jagua-experimental")]
fn precompression_arm_settings(mut settings: GeneralRelaxedSettings) -> GeneralRelaxedSettings {
    settings.collision_backend = GeneralRelaxedCollisionBackend::DynamicHazard;
    settings.angle_seed_policy = GeneralRelaxedAngleSeedPolicy::ContinuousUniform;
    settings.pressure_model = GeneralRelaxedPressureModel::DynamicPoles;
    settings.angular_repair = GeneralAngularRepairSettings::disabled();
    settings.synchronize_lanes = true;
    settings.sweeps_per_epoch = COUPLED_SEPARATOR_ROUNDS;
    settings
}

#[cfg(feature = "jagua-experimental")]
fn fast_result_from_exact_state(
    base: &GeneralFastResult,
    placements: Vec<GeneralFastPlacement>,
    metrics: GeneralPlacementMetrics,
) -> GeneralFastResult {
    let mut result = base.clone();
    result.placements = placements;
    result.unplaced_piece_ids.clear();
    result.used_short_axis_span_mm = metrics.used_short_axis_span_mm;
    result.used_long_axis_depth_mm = metrics.used_long_axis_depth_mm;
    result.unused_short_axis_projection_mm = metrics.unused_short_axis_projection_mm;
    result.occupied_envelope_area_mm2 = metrics.occupied_envelope_area_mm2;
    result
}

#[cfg(feature = "jagua-experimental")]
fn precompression_handoff_work_cap_reason(work: CoupledSeparatorWork) -> Option<String> {
    let checks = [
        (work.dynamic_queries, 6_720_000, "dynamic-query"),
        (work.pressure_evaluations, 64_000_000, "pressure-evaluation"),
        (work.retained_confirmations, 39_040, "retained-confirmation"),
        (work.hazard_updates, 39_040, "hazard-update"),
        (work.layout_loads, 640, "layout-load"),
        (
            work.worker_full_score_pair_visits,
            1_171_200,
            "worker full-score pair-visit",
        ),
        (
            work.auditor_full_score_pair_visits,
            18_300,
            "auditor full-score pair-visit",
        ),
    ];
    checks.into_iter().find_map(|(actual, cap, label)| {
        (actual > cap).then(|| format!("cap: pre-compression handoff {label} budget exhausted"))
    })
}

#[cfg(feature = "jagua-experimental")]
fn precompression_target_arm_diagnostics(
    outcome: &CoupledTargetOutcome,
    initial_state_fingerprint: String,
    elapsed_ms: f64,
    applied_rebuild: bool,
) -> GeneralConflictRuinArmDiagnostics {
    let final_placements = outcome
        .accepted
        .as_ref()
        .map(|result| result.placements.clone())
        .unwrap_or_default();
    GeneralConflictRuinArmDiagnostics {
        attempted: true,
        applied_rebuild,
        elapsed_ms,
        initial_state_fingerprint: Some(initial_state_fingerprint),
        final_state_fingerprint: Some(outcome.diagnostics.final_state_fingerprint.clone()),
        exact_valid: outcome.diagnostics.exact_valid,
        accepted_depth_mm: outcome.diagnostics.accepted_depth_mm,
        final_placement_fingerprint: (!final_placements.is_empty())
            .then(|| coupled_fast_placement_fingerprint(&final_placements)),
        final_placements: coupled_placement_diagnostics(&final_placements),
        work: conflict_ruin_retry_work(outcome.work),
        target: Some(outcome.diagnostics.clone()),
        failure_reason: outcome
            .diagnostics
            .failure_reason
            .clone()
            .or_else(|| outcome.diagnostics.cap_exhausted.clone())
            .or_else(|| outcome.diagnostics.exact_rejection_reason.clone()),
    }
}

#[cfg(feature = "jagua-experimental")]
fn run_precompression_stage_b<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    checkpoint: &CoupledFailedCheckpoint,
    initial_state: RelaxedState,
    diagnostics: &mut GeneralPrecompressionFrontierVacancyDiagnostics,
) {
    let initial_state_fingerprint = coupled_state_fingerprint(&initial_state);
    let started = Instant::now();
    let outcome = run_coupled_separator_target(
        pieces,
        fast_settings,
        precompression_arm_settings(relaxed_settings),
        &checkpoint.incumbent,
        initial_state,
        checkpoint.target_ordinal,
        checkpoint.target_depth_mm,
        checkpoint.compression_split_mm,
        checkpoint.target_seed,
        checkpoint.compression_seed,
        checkpoint.attempt_diagnostics.worker_seeds.clone(),
        CoupledSeparatorArm::Treatment,
        CoupledRollbackRescorePolicy::StrictDerivedAgreement,
        false,
        checkpoint.catalog.clone(),
        checkpoint.hazard_catalog.clone(),
    );
    let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(error) => {
            diagnostics.treatment = GeneralConflictRuinArmDiagnostics {
                attempted: true,
                applied_rebuild: true,
                elapsed_ms,
                initial_state_fingerprint: Some(initial_state_fingerprint),
                failure_reason: Some(error.to_string()),
                ..GeneralConflictRuinArmDiagnostics::default()
            };
            return;
        }
    };
    diagnostics.treatment = precompression_target_arm_diagnostics(
        &outcome,
        initial_state_fingerprint,
        elapsed_ms,
        true,
    );
    diagnostics.mechanism_passed = outcome.accepted.is_some();
}

#[cfg(feature = "jagua-experimental")]
fn precompression_full_score<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    mut relaxed_settings: GeneralRelaxedSettings,
    checkpoint: &CoupledFailedCheckpoint,
    state: &RelaxedState,
    diagnostics: &mut GeneralPrecompressionFrontierVacancyDiagnostics,
) -> Result<PairTracker, String> {
    let pair_visits = pieces.len().saturating_mul(pieces.len().saturating_sub(1)) / 2;
    let next_scores = diagnostics.full_scores.saturating_add(1);
    let next_pair_visits = diagnostics
        .full_score_pair_visits
        .saturating_add(pair_visits);
    if next_scores > PRECOMPRESSION_FRONTIER_FULL_SCORE_CAP
        || next_pair_visits > PRECOMPRESSION_FRONTIER_PAIR_VISIT_CAP
    {
        return Err("cap: pre-compression full-score budget exhausted".to_owned());
    }
    diagnostics.full_scores = next_scores;
    diagnostics.full_score_pair_visits = next_pair_visits;
    relaxed_settings.collision_backend = GeneralRelaxedCollisionBackend::DynamicHazard;
    relaxed_settings.angle_seed_policy = GeneralRelaxedAngleSeedPolicy::ContinuousUniform;
    relaxed_settings.pressure_model = GeneralRelaxedPressureModel::DynamicPoles;
    relaxed_settings.angular_repair = GeneralAngularRepairSettings::disabled();
    relaxed_settings.synchronize_lanes = true;
    let mut search = LaneSearch::new(
        pieces,
        fast_settings,
        relaxed_settings,
        checkpoint.target_seed ^ PRECOMPRESSION_FRONTIER_SEED_DOMAIN,
        checkpoint.catalog.clone(),
    );
    search.hazard_catalog = Some(checkpoint.hazard_catalog.clone());
    search
        .prepare_dynamic_hazard(state)
        .map_err(|error| format!("pre-compression full-index rebuild: {error}"))?;
    search
        .score_state(state)
        .map_err(|error| format!("pre-compression full score: {error}"))
}

#[cfg(feature = "jagua-experimental")]
fn run_conflict_ruin_recreate_experiment<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    control: &CoupledArmOutcome,
    treatment: &CoupledArmOutcome,
) -> GeneralConflictRuinDiagnostics {
    let mut diagnostics = GeneralConflictRuinDiagnostics {
        seed_domain: CONFLICT_RUIN_SEED_DOMAIN,
        ..GeneralConflictRuinDiagnostics::default()
    };
    let checkpoint =
        match select_conflict_ruin_checkpoint(control, treatment, pieces, fast_settings) {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                diagnostics.skipped_reason = Some(error.to_string());
                return diagnostics;
            }
        };
    let Some(checkpoint) = checkpoint else {
        diagnostics.skipped_reason =
            Some("no uncapped failed rigid-separator checkpoint was retained".to_owned());
        return diagnostics;
    };
    diagnostics.target_depth_mm = Some(checkpoint.target_depth_mm);
    diagnostics.checkpoint_fingerprint = Some(conflict_ruin_checkpoint_fingerprint(checkpoint));
    if checkpoint.attempt_diagnostics.failure_reason.is_some()
        || checkpoint.attempt_diagnostics.cap_exhausted.is_some()
    {
        diagnostics.skipped_reason =
            Some("attempt-zero checkpoint was failed or cap-exhausted".to_owned());
        return diagnostics;
    }
    if checkpoint.minimum.score.feasible() || checkpoint.minimum.score.common_loss() <= 0.0 {
        diagnostics.skipped_reason = Some(
            "attempt-zero checkpoint did not retain a strictly positive-loss state".to_owned(),
        );
        return diagnostics;
    }
    if let Err(reason) = validate_conflict_ruin_state(&checkpoint.minimum.state, pieces.len()) {
        diagnostics.skipped_reason = Some(reason);
        return diagnostics;
    }
    if checkpoint.minimum.score.boundary_loss > 0.0 {
        let probe = match probe_conflict_ruin_boundary_blockers(checkpoint, pieces, fast_settings) {
            Ok(probe) => probe,
            Err(reason) => {
                diagnostics.skipped_reason = Some(reason);
                return diagnostics;
            }
        };
        diagnostics.selector_mode = Some("boundaryBlockerProbe".to_owned());
        diagnostics.root_piece_id = Some(pieces[probe.root].id.to_owned());
        diagnostics.root_boundary_loss =
            Some(checkpoint.minimum.score.boundaries[probe.root].raw_loss);
        diagnostics.root_probe_pose = Some(GeneralCoupledSeparatorPlacementDiagnostics {
            piece_id: pieces[probe.root].id.to_owned(),
            rotation_deg: probe.placement.rotation_deg,
            mirrored: probe.placement.mirrored,
            translate_short_axis: probe.placement.translate_x,
            translate_long_axis: probe.placement.translate_y,
        });
        diagnostics.root_probe_blockers = probe
            .blockers
            .iter()
            .map(
                |(piece_index, pressure)| GeneralConflictRuinBlockerDiagnostics {
                    piece_id: pieces[*piece_index].id.to_owned(),
                    proxy_pressure: *pressure,
                },
            )
            .collect();
        let mut projected_state = checkpoint.minimum.state.clone();
        projected_state.placements[probe.root] = probe.placement.clone();
        diagnostics.root_probe_state_fingerprint =
            Some(coupled_state_fingerprint(&projected_state));

        let mut probe_settings = relaxed_settings;
        probe_settings.collision_backend = GeneralRelaxedCollisionBackend::DynamicHazard;
        probe_settings.angle_seed_policy = GeneralRelaxedAngleSeedPolicy::ContinuousUniform;
        probe_settings.pressure_model = GeneralRelaxedPressureModel::DynamicPoles;
        probe_settings.angular_repair = GeneralAngularRepairSettings::disabled();
        probe_settings.synchronize_lanes = true;
        let mut probe_search = LaneSearch::new(
            pieces,
            fast_settings,
            probe_settings,
            checkpoint.target_seed ^ CONFLICT_RUIN_SEED_DOMAIN,
            checkpoint.catalog.clone(),
        );
        probe_search.hazard_catalog = Some(checkpoint.hazard_catalog.clone());
        if let Err(error) = probe_search.prepare_dynamic_hazard(&projected_state) {
            diagnostics.skipped_reason =
                Some(format!("boundary projection full-index rebuild: {error}"));
            return diagnostics;
        }
        let tracker = match probe_search.score_state(&projected_state) {
            Ok(tracker) => tracker,
            Err(error) => {
                diagnostics.skipped_reason =
                    Some(format!("boundary projection full score: {error}"));
                return diagnostics;
            }
        };
        diagnostics.root_probe_tracker_loss = Some(tracker.common_loss());
        diagnostics.root_probe_tracker_boundary_loss = Some(tracker.boundary_loss);
        diagnostics.root_probe_tracker_positive_pairs = Some(tracker.collision_pairs.len());
        diagnostics.root_probe_tracker_feasible = Some(tracker.feasible());

        let placements = to_fast_placements(&projected_state, pieces);
        match validate_and_measure_placements(pieces, &placements, fast_settings) {
            Ok(metrics) => {
                diagnostics.root_probe_exact_valid = Some(true);
                diagnostics.root_probe_exact_depth_mm = Some(metrics.used_long_axis_depth_mm);
                diagnostics.root_probe_improves_incumbent = Some(
                    metrics.used_long_axis_depth_mm < checkpoint.incumbent.used_long_axis_depth_mm,
                );
            }
            Err(error) => {
                diagnostics.root_probe_exact_valid = Some(false);
                diagnostics.root_probe_improves_incumbent = Some(false);
                diagnostics.root_probe_exact_rejection_reason = Some(error.to_string());
            }
        }
        diagnostics.skipped_reason = Some(
            "boundary projection audited; publication remains disabled pending causal review"
                .to_owned(),
        );
        return diagnostics;
    }
    diagnostics.selector_mode = Some("positivePairConflict".to_owned());
    let removal_order = match select_conflict_ruin_neighborhood(checkpoint, pieces) {
        Ok(order) => order,
        Err(reason) => {
            diagnostics.skipped_reason = Some(reason);
            return diagnostics;
        }
    };
    let mut removed = removal_order.clone();
    removed.sort_unstable();
    diagnostics.removed_piece_ids = removed
        .iter()
        .map(|index| pieces[*index].id.to_owned())
        .collect();
    diagnostics.removal_order_piece_ids = removal_order
        .iter()
        .map(|index| pieces[*index].id.to_owned())
        .collect();
    diagnostics.attempted = true;

    let rebuild_started = Instant::now();
    let rebuilt = build_conflict_ruin_state(
        checkpoint,
        pieces,
        fast_settings,
        relaxed_settings.seed,
        &removal_order,
        &mut diagnostics.rebuild,
    );
    diagnostics.rebuild.elapsed_ms = rebuild_started.elapsed().as_secs_f64() * 1_000.0;
    let rebuilt = match rebuilt {
        Ok(rebuilt) => rebuilt,
        Err(reason) => {
            diagnostics.rebuild.cap_exhausted = reason
                .strip_prefix("cap: ")
                .map(str::to_owned)
                .or_else(|| diagnostics.rebuild.cap_exhausted.clone());
            diagnostics.skipped_reason = Some(reason);
            return diagnostics;
        }
    };
    diagnostics.rebuild.selected_state_fingerprint = Some(coupled_state_fingerprint(&rebuilt));

    let retry_seed = relaxed_settings.seed ^ CONFLICT_RUIN_RETRY_SEED_DOMAIN;
    let worker_seeds = (0..COUPLED_SEPARATOR_WORKERS)
        .map(|worker| derive_seed(retry_seed, checkpoint.target_ordinal, worker))
        .collect::<Vec<_>>();
    diagnostics.retry_control = run_conflict_ruin_retry(
        checkpoint,
        pieces,
        fast_settings,
        relaxed_settings,
        checkpoint.minimum.state.clone(),
        false,
        worker_seeds.clone(),
    );
    diagnostics.treatment = run_conflict_ruin_retry(
        checkpoint,
        pieces,
        fast_settings,
        relaxed_settings,
        rebuilt,
        true,
        worker_seeds,
    );
    diagnostics
}

#[cfg(feature = "jagua-experimental")]
fn select_conflict_ruin_checkpoint<'a>(
    control: &'a CoupledArmOutcome,
    treatment: &'a CoupledArmOutcome,
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
) -> Result<Option<&'a CoupledFailedCheckpoint>, GeneralFastError> {
    let mut checkpoints = [control, treatment]
        .into_iter()
        .filter_map(|outcome| outcome.checkpoint.as_ref())
        .map(|checkpoint| {
            Ok::<_, GeneralFastError>((
                checkpoint,
                coupled_independent_source_depth(
                    pieces,
                    &checkpoint.incumbent.placements,
                    fast_settings,
                )?,
                coupled_fast_placement_fingerprint(&checkpoint.incumbent.placements),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    checkpoints.sort_by(|first, second| {
        first
            .1
            .total_cmp(&second.1)
            .then_with(|| first.2.cmp(&second.2))
    });
    Ok(checkpoints.first().map(|(checkpoint, _, _)| *checkpoint))
}

#[cfg(feature = "jagua-experimental")]
fn select_exact_boundary_projection(
    state: &RelaxedState,
    tracker: &PairTracker,
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
) -> Result<(usize, RelaxedPlacement), String> {
    let root = ordered_boundary_piece_indices(state, tracker, pieces)?
        .first()
        .copied()
        .ok_or_else(|| "boundary projection found no positive boundary row".to_owned())?;
    Ok((
        root,
        project_piece_into_exact_boundary(state, pieces, fast_settings, root)?,
    ))
}

#[cfg(feature = "jagua-experimental")]
fn select_all_exact_boundary_projections(
    state: &RelaxedState,
    tracker: &PairTracker,
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
) -> Result<Vec<(usize, RelaxedPlacement)>, String> {
    ordered_boundary_piece_indices(state, tracker, pieces)?
        .into_iter()
        .map(|root| {
            Ok((
                root,
                project_piece_into_exact_boundary(state, pieces, fast_settings, root)?,
            ))
        })
        .collect()
}

#[cfg(feature = "jagua-experimental")]
fn ordered_boundary_piece_indices(
    state: &RelaxedState,
    tracker: &PairTracker,
    pieces: &[GeneralFastPiece<'_>],
) -> Result<Vec<usize>, String> {
    let frontiers = state
        .placements
        .iter()
        .map(|placement| {
            conflict_ruin_material_frontier(pieces[placement.input_index], placement)
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut boundary_pieces = (0..pieces.len())
        .filter(|index| tracker.boundaries[*index].raw_loss > 0.0)
        .collect::<Vec<_>>();
    boundary_pieces.sort_by(|first, second| {
        tracker.boundaries[*second]
            .raw_loss
            .total_cmp(&tracker.boundaries[*first].raw_loss)
            .then_with(|| frontiers[*second].total_cmp(&frontiers[*first]))
            .then_with(|| {
                tracker.incident_raw_loss[*second].total_cmp(&tracker.incident_raw_loss[*first])
            })
            .then_with(|| pieces[*first].id.cmp(pieces[*second].id))
    });
    Ok(boundary_pieces)
}

#[cfg(feature = "jagua-experimental")]
fn project_piece_into_exact_boundary(
    state: &RelaxedState,
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    root: usize,
) -> Result<RelaxedPlacement, String> {
    let current = &state.placements[root];
    let local_collision = pieces[root]
        .polygon
        .transformed(current.rotation_deg, current.mirrored, 0.0, 0.0)
        .and_then(|polygon| polygon.offset(collision_expansion_mm(fast_settings)))
        .map_err(|error| format!("boundary projection geometry: {error}"))?;
    let bounds = local_collision
        .bounds()
        .ok_or_else(|| "boundary projection collision is empty".to_owned())?;
    let inset = collision_sheet_inset_mm(fast_settings);
    let minimum_x = grid_upper_bound_key(inset - bounds.min_x);
    let maximum_x = grid_lower_bound_key(fast_settings.sheet_short_axis_mm - inset - bounds.max_x);
    let minimum_y = grid_upper_bound_key(inset - bounds.min_y);
    let maximum_y = grid_lower_bound_key(state.strip_depth_mm - inset - bounds.max_y);
    if minimum_x > maximum_x || minimum_y > maximum_y {
        return Err("boundary projection has an empty canonical inner-fit rectangle".to_owned());
    }
    let current_x = grid_key(current.translate_x);
    let current_y = grid_key(current.translate_y);
    let placement = RelaxedPlacement {
        input_index: root,
        rotation_deg: current.rotation_deg,
        mirrored: current.mirrored,
        translate_x: from_grid(current_x.clamp(minimum_x, maximum_x) as f64),
        translate_y: from_grid(current_y.clamp(minimum_y, maximum_y) as f64),
    };
    let collision = pieces[root]
        .polygon
        .transformed(
            placement.rotation_deg,
            placement.mirrored,
            placement.translate_x,
            placement.translate_y,
        )
        .and_then(|polygon| polygon.offset(collision_expansion_mm(fast_settings)))
        .map_err(|error| format!("boundary projection placement: {error}"))?;
    if !collision.fits_rect(
        inset,
        inset,
        fast_settings.sheet_short_axis_mm - inset,
        state.strip_depth_mm - inset,
    ) {
        return Err("boundary projection is not exact-fit after canonical clamping".to_owned());
    }
    Ok(placement)
}

#[cfg(feature = "jagua-experimental")]
fn probe_conflict_ruin_boundary_blockers(
    checkpoint: &CoupledFailedCheckpoint,
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
) -> Result<ConflictRuinBoundaryProbe, String> {
    let state = &checkpoint.minimum.state;
    let (root, placement) =
        select_exact_boundary_projection(state, &checkpoint.minimum.score, pieces, fast_settings)?;
    let frontiers = state
        .placements
        .iter()
        .map(|placement| {
            conflict_ruin_material_frontier(pieces[placement.input_index], placement)
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let poses = state.placements.iter().map(hazard_pose).collect::<Vec<_>>();
    let mut active = vec![true; pieces.len()];
    active[root] = false;
    let mut index = JaguaHazardIndex::from_catalog_active(
        pieces,
        fast_settings,
        checkpoint.target_depth_mm,
        &poses,
        &active,
        &checkpoint.hazard_catalog,
    )
    .map_err(|error| format!("boundary-blocker probe index: {error}"))?;
    let pose = hazard_pose(&placement);
    let query = index
        .query_unplaced(root, pose)
        .map_err(|error| format!("boundary-blocker probe query: {error}"))?;
    let GeneralHazardQuery::Complete {
        boundary,
        colliding_piece_ids,
    } = query
    else {
        return Err("boundary-blocker probe unexpectedly pruned".to_owned());
    };
    if boundary {
        return Err("boundary-blocker probe remained outside the hazard envelope".to_owned());
    }
    let mut blockers = colliding_piece_ids
        .into_iter()
        .map(|fixed_piece_id| {
            Ok::<_, String>((
                fixed_piece_id,
                index
                    .collision_pressure(root, pose, fixed_piece_id)
                    .map_err(|error| format!("boundary-blocker probe pressure: {error}"))?,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    blockers.sort_by(|first, second| {
        second
            .1
            .total_cmp(&first.1)
            .then_with(|| frontiers[second.0].total_cmp(&frontiers[first.0]))
            .then_with(|| pieces[first.0].id.cmp(pieces[second.0].id))
    });
    Ok(ConflictRuinBoundaryProbe {
        root,
        placement,
        blockers,
    })
}

#[cfg(feature = "jagua-experimental")]
fn validate_conflict_ruin_state(state: &RelaxedState, piece_count: usize) -> Result<(), String> {
    if state.placements.len() != piece_count {
        return Err(format!(
            "checkpoint contains {} placements for {piece_count} pieces",
            state.placements.len()
        ));
    }
    let mut seen = vec![false; piece_count];
    for (state_index, placement) in state.placements.iter().enumerate() {
        if placement.input_index >= piece_count || seen[placement.input_index] {
            return Err("checkpoint contains an unknown or duplicate stable input ID".to_owned());
        }
        if placement.input_index != state_index {
            return Err("checkpoint stable input IDs are not stored at stable indices".to_owned());
        }
        seen[placement.input_index] = true;
    }
    if seen.iter().any(|present| !present) {
        return Err("checkpoint is missing a stable input ID".to_owned());
    }
    Ok(())
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_checkpoint_fingerprint(checkpoint: &CoupledFailedCheckpoint) -> String {
    let mut digest = Sha256::new();
    digest.update(b"conflict-ruin-reset-empty-weights-v1");
    digest.update(grid_key(checkpoint.target_depth_mm).to_le_bytes());
    digest.update(coupled_state_fingerprint(&checkpoint.minimum.state));
    digest.update(pair_tracker_fingerprint(&checkpoint.minimum.score));
    digest.update(coupled_fast_placement_fingerprint(
        &checkpoint.incumbent.placements,
    ));
    digest.update(checkpoint.target_seed.to_le_bytes());
    digest.update(checkpoint.compression_seed.to_le_bytes());
    format!("{:x}", digest.finalize())
}

#[cfg(feature = "jagua-experimental")]
fn pair_tracker_fingerprint(tracker: &PairTracker) -> String {
    let mut digest = Sha256::new();
    digest.update(tracker.piece_count.to_le_bytes());
    for boundary in &tracker.boundaries {
        digest.update(boundary.violations.to_le_bytes());
        digest.update(boundary.raw_loss.to_bits().to_le_bytes());
    }
    for pair in &tracker.pairs {
        digest.update(pair.raw_loss.to_bits().to_le_bytes());
        digest.update(pair.guided_weight.to_bits().to_le_bytes());
        digest.update(pair.normalization_scale.to_bits().to_le_bytes());
    }
    format!("{:x}", digest.finalize())
}

#[cfg(feature = "jagua-experimental")]
fn select_conflict_ruin_neighborhood(
    checkpoint: &CoupledFailedCheckpoint,
    pieces: &[GeneralFastPiece<'_>],
) -> Result<Vec<usize>, String> {
    let state = &checkpoint.minimum.state;
    let tracker = &checkpoint.minimum.score;
    let mut indices = (0..pieces.len()).collect::<Vec<_>>();
    indices.sort_by(|first, second| {
        tracker.incident_raw_loss[*second]
            .total_cmp(&tracker.incident_raw_loss[*first])
            .then_with(|| {
                tracker.boundaries[*second]
                    .raw_loss
                    .total_cmp(&tracker.boundaries[*first].raw_loss)
            })
            .then_with(|| {
                conflict_ruin_material_frontier(pieces[*second], &state.placements[*second])
                    .unwrap_or(f64::NEG_INFINITY)
                    .total_cmp(
                        &conflict_ruin_material_frontier(pieces[*first], &state.placements[*first])
                            .unwrap_or(f64::NEG_INFINITY),
                    )
            })
            .then_with(|| pieces[*first].id.cmp(pieces[*second].id))
    });
    let root = indices
        .into_iter()
        .find(|index| tracker.incident_raw_loss[*index] > 0.0)
        .ok_or_else(|| "checkpoint has no positive incident conflict loss".to_owned())?;
    let mut neighbors = tracker
        .collision_pairs
        .iter()
        .filter_map(|(first, second, loss)| {
            if *loss <= 0.0 {
                None
            } else if *first == root {
                Some((*second, *loss))
            } else if *second == root {
                Some((*first, *loss))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    neighbors.sort_by(|first, second| {
        second
            .1
            .total_cmp(&first.1)
            .then_with(|| {
                conflict_ruin_material_frontier(pieces[second.0], &state.placements[second.0])
                    .unwrap_or(f64::NEG_INFINITY)
                    .total_cmp(
                        &conflict_ruin_material_frontier(
                            pieces[first.0],
                            &state.placements[first.0],
                        )
                        .unwrap_or(f64::NEG_INFINITY),
                    )
            })
            .then_with(|| pieces[first.0].id.cmp(pieces[second.0].id))
    });
    neighbors.dedup_by_key(|(index, _)| *index);
    if neighbors.len() < CONFLICT_RUIN_REMOVED_PIECES - 1 {
        return Err("root has fewer than two positive conflict neighbors".to_owned());
    }
    let mut selected = vec![root, neighbors[0].0, neighbors[1].0];
    selected.sort_by(|first, second| {
        tracker.incident_raw_loss[*second]
            .total_cmp(&tracker.incident_raw_loss[*first])
            .then_with(|| pieces[*first].id.cmp(pieces[*second].id))
    });
    Ok(selected)
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_material_frontier(
    piece: GeneralFastPiece<'_>,
    placement: &RelaxedPlacement,
) -> Result<f64, GeneralFastError> {
    piece
        .polygon
        .transformed(
            placement.rotation_deg,
            placement.mirrored,
            placement.translate_x,
            placement.translate_y,
        )?
        .bounds()
        .map(|bounds| bounds.max_y)
        .ok_or_else(|| {
            GeneralPolygonError::from_message("conflict-ruin material contour is empty").into()
        })
}

#[cfg(feature = "jagua-experimental")]
fn build_conflict_ruin_state(
    checkpoint: &CoupledFailedCheckpoint,
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    seed: u64,
    removal_order: &[usize],
    diagnostics: &mut GeneralConflictRuinRebuildDiagnostics,
) -> Result<RelaxedState, String> {
    let mut work = ConflictRuinWork::for_piece_count(pieces.len());
    let outcome = build_conflict_ruin_state_inner(
        &checkpoint.minimum.state,
        checkpoint.target_depth_mm,
        &checkpoint.hazard_catalog,
        pieces,
        fast_settings,
        seed,
        removal_order,
        diagnostics,
        &mut work,
    );
    conflict_ruin_publish_work(diagnostics, work);
    let outcome = outcome?;
    let selected = outcome
        .beam
        .into_iter()
        .min_by(conflict_ruin_beam_order)
        .ok_or_else(|| "conflict rebuild retained no complete state".to_owned())?;
    let initial_fallback = ConflictRuinBeamState {
        state: checkpoint.minimum.state.clone(),
        active: vec![true; pieces.len()],
        collisions: vec![None; pieces.len()],
        score: outcome.initial_score,
    };
    let selected = if conflict_ruin_beam_order(&initial_fallback, &selected) == Ordering::Less {
        initial_fallback
    } else {
        selected
    };
    validate_complete_conflict_ruin_child(&selected, pieces.len())?;
    diagnostics.selected_exact_overlap_area_mm2 = Some(selected.score.total_overlap_area_mm2);
    diagnostics.selected_positive_overlap_pairs = Some(selected.score.positive_overlap_pairs);
    Ok(selected.state)
}

#[cfg(feature = "jagua-experimental")]
fn build_conflict_ruin_states(
    base_state: &RelaxedState,
    strip_depth_mm: f64,
    hazard_catalog: &Arc<JaguaHazardCatalog>,
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    seed: u64,
    removal_order: &[usize],
    diagnostics: &mut GeneralConflictRuinRebuildDiagnostics,
) -> Result<Vec<ConflictRuinBeamState>, String> {
    let mut work = ConflictRuinWork::for_piece_count(pieces.len());
    let outcome = build_conflict_ruin_state_inner(
        base_state,
        strip_depth_mm,
        hazard_catalog,
        pieces,
        fast_settings,
        seed,
        removal_order,
        diagnostics,
        &mut work,
    );
    conflict_ruin_publish_work(diagnostics, work);
    let outcome = outcome?;
    for child in &outcome.beam {
        validate_complete_conflict_ruin_child(child, pieces.len())?;
    }
    Ok(outcome.beam)
}

#[cfg(feature = "jagua-experimental")]
fn validate_complete_conflict_ruin_child(
    child: &ConflictRuinBeamState,
    piece_count: usize,
) -> Result<(), String> {
    if child.active.iter().any(|active| !active) {
        return Err("conflict rebuild did not restore every active bit".to_owned());
    }
    validate_conflict_ruin_state(&child.state, piece_count)
}

#[cfg(feature = "jagua-experimental")]
fn build_conflict_ruin_state_inner(
    base_state: &RelaxedState,
    strip_depth_mm: f64,
    hazard_catalog: &Arc<JaguaHazardCatalog>,
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    seed: u64,
    removal_order: &[usize],
    diagnostics: &mut GeneralConflictRuinRebuildDiagnostics,
    work: &mut ConflictRuinWork,
) -> Result<ConflictRuinBuildOutcome, String> {
    if removal_order.is_empty() || removal_order.len() > CONFLICT_RUIN_REMOVED_PIECES {
        return Err(format!(
            "conflict rebuild requires between one and {CONFLICT_RUIN_REMOVED_PIECES} removed pieces"
        ));
    }
    let mut collisions = vec![None; pieces.len()];
    for placement in &base_state.placements {
        collisions[placement.input_index] = Some(conflict_ruin_build_collision(
            pieces[placement.input_index],
            placement,
            fast_settings,
            work,
        )?);
    }
    let pair_count = pieces.len().saturating_mul(pieces.len().saturating_sub(1)) / 2;
    let mut initial_pair_areas = vec![0.0; pair_count];
    let mut initial_score = ConflictRuinExactScore {
        total_overlap_area_mm2: 0.0,
        positive_overlap_pairs: 0,
        maximum_pair_area_mm2: 0.0,
        frontier_depth_mm: conflict_ruin_state_frontier(
            pieces,
            base_state,
            &vec![true; pieces.len()],
        )?,
    };
    for first in 0..pieces.len() {
        for second in (first + 1)..pieces.len() {
            let area = conflict_ruin_intersection_area(
                collisions[first]
                    .as_ref()
                    .ok_or_else(|| format!("missing collision polygon for piece {first}"))?,
                collisions[second]
                    .as_ref()
                    .ok_or_else(|| format!("missing collision polygon for piece {second}"))?,
                work,
            )?;
            initial_pair_areas[pair_slot(pieces.len(), first, second)] = area;
            conflict_ruin_add_pair_area(&mut initial_score, area);
        }
    }
    diagnostics.initial_exact_overlap_area_mm2 = initial_score.total_overlap_area_mm2;
    diagnostics.initial_positive_overlap_pairs = initial_score.positive_overlap_pairs;

    let mut active = vec![true; pieces.len()];
    for piece_index in removal_order {
        if *piece_index >= pieces.len() || !active[*piece_index] {
            return Err(
                "conflict rebuild removal order contains an unknown or duplicate ID".to_owned(),
            );
        }
        active[*piece_index] = false;
        collisions[*piece_index] = None;
    }
    let survivor_score =
        conflict_ruin_score_active_pairs(pieces, base_state, &active, &initial_pair_areas)?;
    let mut beam = vec![ConflictRuinBeamState {
        state: base_state.clone(),
        active,
        collisions,
        score: survivor_score,
    }];

    for (layer, piece_index) in removal_order.iter().copied().enumerate() {
        let mut children = Vec::new();
        for (parent_ordinal, parent) in beam.iter().enumerate() {
            let poses = parent
                .state
                .placements
                .iter()
                .map(hazard_pose)
                .collect::<Vec<_>>();
            let mut index = JaguaHazardIndex::from_catalog_active(
                pieces,
                fast_settings,
                strip_depth_mm,
                &poses,
                &parent.active,
                hazard_catalog,
            )
            .map_err(|error| format!("partial hazard index: {error}"))?;
            let orientations = conflict_ruin_orientations(
                pieces[piece_index],
                &parent.state.placements[piece_index],
                derive_seed(
                    seed ^ CONFLICT_RUIN_ANGLE_SEED_DOMAIN,
                    layer,
                    parent_ordinal.saturating_mul(pieces.len()) + piece_index,
                ),
            );
            for (orientation_ordinal, (rotation_deg, mirrored)) in
                orientations.into_iter().enumerate()
            {
                work.parent_orientation_streams = work.parent_orientation_streams.saturating_add(1);
                if work.parent_orientation_streams > CONFLICT_RUIN_STREAM_CAP {
                    return Err("cap: parent-orientation stream budget exhausted".to_owned());
                }
                let orientation = RelaxedPlacement {
                    input_index: piece_index,
                    rotation_deg,
                    mirrored,
                    translate_x: 0.0,
                    translate_y: 0.0,
                };
                let local_collision = conflict_ruin_build_collision(
                    pieces[piece_index],
                    &orientation,
                    fast_settings,
                    work,
                )?;
                let position_seed = derive_seed(
                    seed ^ CONFLICT_RUIN_POSITION_SEED_DOMAIN,
                    layer.saturating_mul(CONFLICT_RUIN_BEAM_WIDTH) + parent_ordinal,
                    orientation_ordinal.saturating_mul(pieces.len()) + piece_index,
                );
                let proposals = conflict_ruin_positions(
                    &parent.state.placements[piece_index],
                    &orientation,
                    &local_collision,
                    parent,
                    fast_settings,
                    strip_depth_mm,
                    position_seed,
                    work,
                )?;
                let mut ranked = Vec::new();
                for placement in proposals {
                    work.cheap_queries = work.cheap_queries.saturating_add(1);
                    if work.cheap_queries > CONFLICT_RUIN_QUERY_CAP {
                        return Err("cap: cheap-query budget exhausted".to_owned());
                    }
                    let pose = hazard_pose(&placement);
                    let query = match index.query_unplaced(piece_index, pose) {
                        Ok(query) => query,
                        Err(error) if error.to_string().contains("query envelope") => continue,
                        Err(error) => return Err(format!("partial hazard query: {error}")),
                    };
                    let GeneralHazardQuery::Complete {
                        colliding_piece_ids,
                        ..
                    } = query
                    else {
                        return Err("partial unplaced query unexpectedly pruned".to_owned());
                    };
                    let mut proxy_loss = 0.0;
                    for fixed_piece_id in colliding_piece_ids {
                        if !parent.active[fixed_piece_id] {
                            return Err("inactive hazard leaked into a partial query".to_owned());
                        }
                        proxy_loss += index
                            .collision_pressure(piece_index, pose, fixed_piece_id)
                            .map_err(|error| format!("partial hazard pressure: {error}"))?;
                    }
                    ranked.push(ConflictRuinCandidate {
                        placement,
                        proxy_loss,
                    });
                }
                let required_current = ranked
                    .iter()
                    .find(|candidate| {
                        placement_key(&candidate.placement)
                            == placement_key(&parent.state.placements[piece_index])
                    })
                    .cloned();
                let mut finalists = conflict_ruin_diverse_finalists(
                    ranked,
                    fast_settings,
                    derive_seed(
                        seed ^ CONFLICT_RUIN_DIVERSITY_SEED_DOMAIN,
                        layer.saturating_mul(CONFLICT_RUIN_BEAM_WIDTH) + parent_ordinal,
                        orientation_ordinal.saturating_mul(pieces.len()) + piece_index,
                    ),
                );
                if let Some(required_current) = required_current {
                    if !finalists.iter().any(|candidate| {
                        placement_key(&candidate.placement)
                            == placement_key(&required_current.placement)
                    }) {
                        if finalists.len() == CONFLICT_RUIN_FINALISTS_PER_STREAM {
                            finalists.pop();
                        }
                        finalists.push(required_current);
                        work.required_current_finalists =
                            work.required_current_finalists.saturating_add(1);
                    }
                }
                for finalist in finalists {
                    work.exact_finalists = work.exact_finalists.saturating_add(1);
                    if work.exact_finalists > CONFLICT_RUIN_FINALIST_CAP {
                        return Err("cap: exact-finalist budget exhausted".to_owned());
                    }
                    children.push(conflict_ruin_exact_child(
                        parent,
                        pieces,
                        piece_index,
                        finalist.placement,
                        fast_settings,
                        work,
                    )?);
                }
            }
        }
        if children.is_empty() {
            return Err(format!(
                "conflict rebuild layer {layer} produced no exact-scored child"
            ));
        }
        children.sort_by(conflict_ruin_beam_order);
        let mut fingerprints = BTreeSet::new();
        children.retain(|child| fingerprints.insert(coupled_state_fingerprint(&child.state)));
        children.truncate(CONFLICT_RUIN_BEAM_WIDTH);
        work.partials_retained = work.partials_retained.saturating_add(children.len());
        beam = children;
    }
    Ok(ConflictRuinBuildOutcome {
        beam,
        initial_score,
    })
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_publish_work(
    diagnostics: &mut GeneralConflictRuinRebuildDiagnostics,
    work: ConflictRuinWork,
) {
    diagnostics.parent_orientation_streams = work.parent_orientation_streams;
    diagnostics.cheap_queries = work.cheap_queries;
    diagnostics.exact_finalists = work.exact_finalists;
    diagnostics.exact_pair_intersection_limit = work.pair_intersection_limit;
    diagnostics.exact_pair_intersections = work.exact_pair_intersections;
    diagnostics.required_current_finalists = work.required_current_finalists;
    diagnostics.orientation_build_limit = work.orientation_build_limit;
    diagnostics.orientation_builds = work.orientation_builds;
    diagnostics.transformed_output_vertices = work.transformed_output_vertices;
    diagnostics.feature_visits = work.feature_visits;
    diagnostics.pre_dedup_contact_attempts = work.pre_dedup_contact_attempts;
    diagnostics.deduplicated_proposals = work.deduplicated_proposals;
    diagnostics.clipper_input_vertices = work.clipper_input_vertices;
    diagnostics.clipper_output_vertices = work.clipper_output_vertices;
    diagnostics.partials_retained = work.partials_retained;
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_build_collision(
    piece: GeneralFastPiece<'_>,
    placement: &RelaxedPlacement,
    fast_settings: GeneralFastSettings,
    work: &mut ConflictRuinWork,
) -> Result<PolygonSet, String> {
    if work.orientation_builds >= work.orientation_build_limit {
        return Err("cap: transformed-orientation build budget exhausted".to_owned());
    }
    work.orientation_builds += 1;
    let collision = piece
        .polygon
        .transformed(
            placement.rotation_deg,
            placement.mirrored,
            placement.translate_x,
            placement.translate_y,
        )
        .and_then(|polygon| polygon.offset(collision_expansion_mm(fast_settings)))
        .map_err(|error| format!("conflict collision geometry: {error}"))?;
    work.transformed_output_vertices = work
        .transformed_output_vertices
        .saturating_add(collision.vertex_count());
    if work.transformed_output_vertices > CONFLICT_RUIN_TRANSFORMED_VERTEX_CAP {
        return Err("cap: transformed-output vertex budget exhausted".to_owned());
    }
    Ok(collision)
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_intersection_area(
    first: &PolygonSet,
    second: &PolygonSet,
    work: &mut ConflictRuinWork,
) -> Result<f64, String> {
    if work.exact_pair_intersections >= work.pair_intersection_limit {
        return Err("cap: exact pair-intersection budget exhausted".to_owned());
    }
    let input_vertices = first.vertex_count().saturating_add(second.vertex_count());
    if work.clipper_input_vertices.saturating_add(input_vertices)
        > CONFLICT_RUIN_CLIPPER_INPUT_VERTEX_CAP
    {
        return Err("cap: aggregate Clipper input-vertex budget exhausted".to_owned());
    }
    let result = first
        .intersection_area_with_complexity(second)
        .map_err(|error| format!("exact conflict intersection: {error}"))?;
    work.exact_pair_intersections += 1;
    work.clipper_input_vertices = work
        .clipper_input_vertices
        .saturating_add(result.input_vertices);
    work.clipper_output_vertices = work
        .clipper_output_vertices
        .saturating_add(result.output_vertices);
    if work.clipper_output_vertices > CONFLICT_RUIN_CLIPPER_OUTPUT_VERTEX_CAP {
        return Err("cap: aggregate Clipper output-vertex budget exhausted".to_owned());
    }
    Ok(result.area_mm2)
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_add_pair_area(score: &mut ConflictRuinExactScore, area_mm2: f64) {
    score.total_overlap_area_mm2 += area_mm2;
    if area_mm2 > 0.0 {
        score.positive_overlap_pairs = score.positive_overlap_pairs.saturating_add(1);
        score.maximum_pair_area_mm2 = score.maximum_pair_area_mm2.max(area_mm2);
    }
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_score_active_pairs(
    pieces: &[GeneralFastPiece<'_>],
    state: &RelaxedState,
    active: &[bool],
    pair_areas: &[f64],
) -> Result<ConflictRuinExactScore, String> {
    let mut score = ConflictRuinExactScore {
        total_overlap_area_mm2: 0.0,
        positive_overlap_pairs: 0,
        maximum_pair_area_mm2: 0.0,
        frontier_depth_mm: conflict_ruin_state_frontier(pieces, state, active)?,
    };
    for first in 0..pieces.len() {
        if !active[first] {
            continue;
        }
        for second in (first + 1)..pieces.len() {
            if active[second] {
                conflict_ruin_add_pair_area(
                    &mut score,
                    pair_areas[pair_slot(pieces.len(), first, second)],
                );
            }
        }
    }
    Ok(score)
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_state_frontier(
    pieces: &[GeneralFastPiece<'_>],
    state: &RelaxedState,
    active: &[bool],
) -> Result<f64, String> {
    state
        .placements
        .iter()
        .filter(|placement| active[placement.input_index])
        .map(|placement| {
            conflict_ruin_material_frontier(pieces[placement.input_index], placement)
                .map_err(|error| error.to_string())
        })
        .try_fold(f64::NEG_INFINITY, |frontier, candidate| {
            candidate.map(|candidate| frontier.max(candidate))
        })
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_orientations(
    piece: GeneralFastPiece<'_>,
    current: &RelaxedPlacement,
    seed: u64,
) -> Vec<(f64, bool)> {
    let mut orientations = Vec::with_capacity(CONFLICT_RUIN_ORIENTATIONS_PER_PARENT);
    let push = |orientations: &mut Vec<(f64, bool)>, angle: f64, mirrored: bool| {
        let angle = if piece.allow_rotation {
            continuous_angle(angle)
        } else {
            0.0
        };
        let mirrored = piece.allow_mirror && mirrored;
        let key = (angle_key(angle), mirrored);
        if !orientations
            .iter()
            .any(|(existing_angle, existing_mirror)| {
                (angle_key(*existing_angle), *existing_mirror) == key
            })
        {
            orientations.push((angle, mirrored));
        }
    };
    push(&mut orientations, current.rotation_deg, current.mirrored);
    let mirrors = if piece.allow_mirror {
        vec![current.mirrored, !current.mirrored]
    } else {
        vec![false]
    };
    for mirrored in mirrors.iter().copied() {
        for angle in [0.0, 90.0, 180.0, 270.0] {
            push(&mut orientations, angle, mirrored);
        }
    }
    if piece.allow_rotation {
        for mirrored in mirrors.iter().copied() {
            for region in piece.polygon.regions() {
                let points = region.outer.source_points();
                for index in 0..points.len() {
                    let first = points[index];
                    let second = points[(index + 1) % points.len()];
                    let delta_x = if mirrored {
                        first.x - second.x
                    } else {
                        second.x - first.x
                    };
                    let delta_y = second.y - first.y;
                    let edge_angle = delta_y.atan2(delta_x).to_degrees();
                    push(&mut orientations, -edge_angle, mirrored);
                    push(&mut orientations, 90.0 - edge_angle, mirrored);
                    if orientations.len() >= CONFLICT_RUIN_ORIENTATIONS_PER_PARENT {
                        break;
                    }
                }
                if orientations.len() >= CONFLICT_RUIN_ORIENTATIONS_PER_PARENT {
                    break;
                }
            }
            if orientations.len() >= CONFLICT_RUIN_ORIENTATIONS_PER_PARENT {
                break;
            }
        }
        let mut rng = SplitMix64::new(seed);
        let mut attempts = 0usize;
        while orientations.len() < CONFLICT_RUIN_ORIENTATIONS_PER_PARENT && attempts < 128 {
            let mirrored = piece.allow_mirror && rng.next_u64() & 1 == 1;
            push(&mut orientations, rng.range(0.0, 360.0), mirrored);
            attempts += 1;
        }
    }
    orientations.truncate(CONFLICT_RUIN_ORIENTATIONS_PER_PARENT);
    orientations
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_positions(
    current: &RelaxedPlacement,
    orientation: &RelaxedPlacement,
    local_collision: &PolygonSet,
    parent: &ConflictRuinBeamState,
    fast_settings: GeneralFastSettings,
    target_depth_mm: f64,
    seed: u64,
    work: &mut ConflictRuinWork,
) -> Result<Vec<RelaxedPlacement>, String> {
    let bounds = local_collision
        .bounds()
        .ok_or_else(|| "conflict orientation has empty collision geometry".to_owned())?;
    let inset = collision_sheet_inset_mm(fast_settings);
    let min_x = inset - bounds.min_x;
    let max_x = fast_settings.sheet_short_axis_mm - inset - bounds.max_x;
    let min_y = inset - bounds.min_y;
    let max_y = target_depth_mm - inset - bounds.max_y;
    if min_x > max_x || min_y > max_y {
        return Ok(Vec::new());
    }
    let current_x = current.translate_x.clamp(min_x, max_x);
    let current_y = current.translate_y.clamp(min_y, max_y);
    let center_x = (min_x + max_x) * 0.5;
    let center_y = (min_y + max_y) * 0.5;
    let mut categories = vec![Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    categories[0].push((current_x, current_y));
    categories[1].extend([
        (min_x, min_y),
        (min_x, max_y),
        (max_x, min_y),
        (max_x, max_y),
        (min_x, center_y),
        (max_x, center_y),
        (center_x, min_y),
        (center_x, max_y),
    ]);
    for (fixed_index, fixed_collision) in parent.collisions.iter().enumerate() {
        if !parent.active[fixed_index] {
            continue;
        }
        let fixed_bounds = fixed_collision
            .as_ref()
            .and_then(PolygonSet::bounds)
            .ok_or_else(|| format!("active piece {fixed_index} has no collision bounds"))?;
        work.feature_visits = work.feature_visits.saturating_add(4);
        if work.feature_visits > CONFLICT_RUIN_FEATURE_VISIT_CAP {
            return Err("cap: source/fixed feature-visit budget exhausted".to_owned());
        }
        let left = (fixed_bounds.min_x - bounds.max_x).clamp(min_x, max_x);
        let right = (fixed_bounds.max_x - bounds.min_x).clamp(min_x, max_x);
        let below = (fixed_bounds.min_y - bounds.max_y).clamp(min_y, max_y);
        let above = (fixed_bounds.max_y - bounds.min_y).clamp(min_y, max_y);
        let contact_positions = [
            (left, current_y),
            (right, current_y),
            (current_x, below),
            (current_x, above),
            (left, below),
            (left, above),
            (right, below),
            (right, above),
        ];
        work.pre_dedup_contact_attempts = work
            .pre_dedup_contact_attempts
            .saturating_add(contact_positions.len());
        if work.pre_dedup_contact_attempts > CONFLICT_RUIN_CONTACT_ATTEMPT_CAP {
            return Err("cap: pre-dedup contact-attempt budget exhausted".to_owned());
        }
        categories[2].extend(contact_positions);
    }
    let width = (bounds.max_x - bounds.min_x).max(fast_settings.total_padding_mm);
    let height = (bounds.max_y - bounds.min_y).max(fast_settings.total_padding_mm);
    let mut focused_rng = SplitMix64::new(seed ^ 0xF0C5_5EED_0000_0001);
    for _ in 0..16 {
        categories[3].push((
            (current_x + focused_rng.range(-2.0 * width, 2.0 * width)).clamp(min_x, max_x),
            (current_y + focused_rng.range(-2.0 * height, 2.0 * height)).clamp(min_y, max_y),
        ));
    }
    let mut global_rng = SplitMix64::new(seed ^ 0x610B_A11E_0000_0001);
    for _ in 0..16 {
        categories[4].push((
            global_rng.range(min_x, max_x),
            global_rng.range(min_y, max_y),
        ));
    }
    let mut category_indices = vec![0usize; categories.len()];
    let mut deduplicated = BTreeSet::new();
    let mut placements = Vec::with_capacity(CONFLICT_RUIN_POSES_PER_STREAM);
    while placements.len() < CONFLICT_RUIN_POSES_PER_STREAM {
        let mut progressed = false;
        for category in 0..categories.len() {
            let Some((x, y)) = categories[category]
                .get(category_indices[category])
                .copied()
            else {
                continue;
            };
            category_indices[category] += 1;
            progressed = true;
            let placement = RelaxedPlacement {
                input_index: orientation.input_index,
                rotation_deg: orientation.rotation_deg,
                mirrored: orientation.mirrored,
                translate_x: snap_mm(x),
                translate_y: snap_mm(y),
            };
            if deduplicated.insert(placement_key(&placement)) {
                placements.push(placement);
                if placements.len() >= CONFLICT_RUIN_POSES_PER_STREAM {
                    break;
                }
            }
        }
        if !progressed {
            break;
        }
    }
    work.deduplicated_proposals = work.deduplicated_proposals.saturating_add(placements.len());
    if work.deduplicated_proposals > CONFLICT_RUIN_PROPOSAL_CAP {
        return Err("cap: deduplicated-proposal budget exhausted".to_owned());
    }
    Ok(placements)
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_diverse_finalists(
    mut candidates: Vec<ConflictRuinCandidate>,
    fast_settings: GeneralFastSettings,
    seed: u64,
) -> Vec<ConflictRuinCandidate> {
    candidates.sort_by(|first, second| {
        first
            .proxy_loss
            .total_cmp(&second.proxy_loss)
            .then_with(|| {
                conflict_ruin_diversity_key(&first.placement, seed)
                    .cmp(&conflict_ruin_diversity_key(&second.placement, seed))
            })
            .then_with(|| placement_key(&first.placement).cmp(&placement_key(&second.placement)))
    });
    candidates.truncate(CONFLICT_RUIN_FINALISTS_PER_STREAM.saturating_mul(4));
    let threshold = fast_settings
        .sheet_short_axis_mm
        .hypot(fast_settings.sheet_long_axis_mm)
        * 0.01;
    let mut selected = Vec::with_capacity(CONFLICT_RUIN_FINALISTS_PER_STREAM);
    for candidate in &candidates {
        if selected.iter().all(|selected: &ConflictRuinCandidate| {
            (selected.placement.translate_x - candidate.placement.translate_x)
                .hypot(selected.placement.translate_y - candidate.placement.translate_y)
                >= threshold
        }) {
            selected.push(candidate.clone());
            if selected.len() == CONFLICT_RUIN_FINALISTS_PER_STREAM {
                return selected;
            }
        }
    }
    for candidate in candidates {
        if selected.iter().any(|selected| {
            placement_key(&selected.placement) == placement_key(&candidate.placement)
        }) {
            continue;
        }
        selected.push(candidate);
        if selected.len() == CONFLICT_RUIN_FINALISTS_PER_STREAM {
            break;
        }
    }
    selected
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_diversity_key(placement: &RelaxedPlacement, seed: u64) -> u64 {
    let (_, angle, mirrored, x, y) = placement_key(placement);
    let mixed = seed
        ^ (angle as u64).rotate_left(7)
        ^ (x as u64).rotate_left(23)
        ^ (y as u64).rotate_left(41)
        ^ u64::from(mirrored);
    SplitMix64::new(mixed).next_u64()
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_exact_child(
    parent: &ConflictRuinBeamState,
    pieces: &[GeneralFastPiece<'_>],
    piece_index: usize,
    placement: RelaxedPlacement,
    fast_settings: GeneralFastSettings,
    work: &mut ConflictRuinWork,
) -> Result<ConflictRuinBeamState, String> {
    let collision =
        conflict_ruin_build_collision(pieces[piece_index], &placement, fast_settings, work)?;
    let inset = collision_sheet_inset_mm(fast_settings);
    if !collision.fits_rect(
        inset,
        inset,
        fast_settings.sheet_short_axis_mm - inset,
        parent.state.strip_depth_mm - inset,
    ) {
        return Err("exact finalist lies outside the target strip".to_owned());
    }
    let active_pair_count = parent.active.iter().filter(|active| **active).count();
    if work
        .exact_pair_intersections
        .saturating_add(active_pair_count)
        > work.pair_intersection_limit
    {
        return Err("cap: exact pair-intersection budget cannot fund a finalist".to_owned());
    }
    let candidate_vertices = collision.vertex_count();
    let required_input_vertices = parent
        .collisions
        .iter()
        .enumerate()
        .filter(|(index, _)| parent.active[*index])
        .map(|(_, fixed)| {
            candidate_vertices.saturating_add(fixed.as_ref().map_or(0, PolygonSet::vertex_count))
        })
        .sum::<usize>();
    if work
        .clipper_input_vertices
        .saturating_add(required_input_vertices)
        > CONFLICT_RUIN_CLIPPER_INPUT_VERTEX_CAP
    {
        return Err("cap: aggregate Clipper input budget cannot fund a finalist".to_owned());
    }
    let mut score = parent.score;
    for (fixed_index, fixed_collision) in parent.collisions.iter().enumerate() {
        if !parent.active[fixed_index] {
            continue;
        }
        let area = conflict_ruin_intersection_area(
            &collision,
            fixed_collision
                .as_ref()
                .ok_or_else(|| format!("active piece {fixed_index} has no collision polygon"))?,
            work,
        )?;
        conflict_ruin_add_pair_area(&mut score, area);
    }
    score.frontier_depth_mm = score.frontier_depth_mm.max(
        conflict_ruin_material_frontier(pieces[piece_index], &placement)
            .map_err(|error| error.to_string())?,
    );
    let mut child = parent.clone();
    child.state.placements[piece_index] = placement;
    child.active[piece_index] = true;
    child.collisions[piece_index] = Some(collision);
    child.score = score;
    Ok(child)
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_beam_order(
    first: &ConflictRuinBeamState,
    second: &ConflictRuinBeamState,
) -> Ordering {
    first
        .score
        .total_overlap_area_mm2
        .total_cmp(&second.score.total_overlap_area_mm2)
        .then_with(|| {
            first
                .score
                .positive_overlap_pairs
                .cmp(&second.score.positive_overlap_pairs)
        })
        .then_with(|| {
            first
                .score
                .maximum_pair_area_mm2
                .total_cmp(&second.score.maximum_pair_area_mm2)
        })
        .then_with(|| {
            first
                .score
                .frontier_depth_mm
                .total_cmp(&second.score.frontier_depth_mm)
        })
        .then_with(|| canonical_state_key(&first.state).cmp(&canonical_state_key(&second.state)))
}

#[cfg(feature = "jagua-experimental")]
fn run_conflict_ruin_retry<'a>(
    checkpoint: &CoupledFailedCheckpoint,
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    mut relaxed_settings: GeneralRelaxedSettings,
    initial_state: RelaxedState,
    applied_rebuild: bool,
    worker_seeds: Vec<u64>,
) -> GeneralConflictRuinArmDiagnostics {
    let started = Instant::now();
    let mut diagnostics = GeneralConflictRuinArmDiagnostics {
        attempted: true,
        applied_rebuild,
        initial_state_fingerprint: Some(coupled_state_fingerprint(&initial_state)),
        ..GeneralConflictRuinArmDiagnostics::default()
    };
    relaxed_settings.collision_backend = GeneralRelaxedCollisionBackend::DynamicHazard;
    relaxed_settings.angle_seed_policy = GeneralRelaxedAngleSeedPolicy::ContinuousUniform;
    relaxed_settings.pressure_model = GeneralRelaxedPressureModel::DynamicPoles;
    relaxed_settings.angular_repair = GeneralAngularRepairSettings::disabled();
    relaxed_settings.synchronize_lanes = true;
    relaxed_settings.sweeps_per_epoch = COUPLED_SEPARATOR_ROUNDS;
    let outcome = run_coupled_separator_target(
        pieces,
        fast_settings,
        relaxed_settings,
        &checkpoint.incumbent,
        initial_state,
        checkpoint
            .target_ordinal
            .saturating_add(COUPLED_SEPARATOR_TARGETS),
        checkpoint.target_depth_mm,
        checkpoint.compression_split_mm,
        checkpoint.target_seed ^ CONFLICT_RUIN_SEED_DOMAIN,
        checkpoint.compression_seed ^ CONFLICT_RUIN_SEED_DOMAIN,
        worker_seeds,
        CoupledSeparatorArm::Treatment,
        CoupledRollbackRescorePolicy::StrictDerivedAgreement,
        false,
        checkpoint.catalog.clone(),
        checkpoint.hazard_catalog.clone(),
    );
    diagnostics.elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(error) => {
            diagnostics.failure_reason = Some(error.to_string());
            return diagnostics;
        }
    };
    diagnostics.final_state_fingerprint = Some(outcome.diagnostics.final_state_fingerprint.clone());
    diagnostics.exact_valid = outcome.diagnostics.exact_valid;
    diagnostics.accepted_depth_mm = outcome.diagnostics.accepted_depth_mm;
    diagnostics.failure_reason = outcome
        .diagnostics
        .failure_reason
        .clone()
        .or_else(|| outcome.diagnostics.cap_exhausted.clone());
    diagnostics.work = conflict_ruin_retry_work(outcome.work);
    if let Some(accepted) = outcome.accepted {
        diagnostics.final_placement_fingerprint =
            Some(coupled_fast_placement_fingerprint(&accepted.placements));
        diagnostics.final_placements = coupled_placement_diagnostics(&accepted.placements);
    }
    diagnostics.target = Some(outcome.diagnostics);
    diagnostics
}

#[cfg(feature = "jagua-experimental")]
fn conflict_ruin_retry_work(work: CoupledSeparatorWork) -> GeneralConflictRuinRetryWorkDiagnostics {
    GeneralConflictRuinRetryWorkDiagnostics {
        worker_sweeps: work.worker_sweeps,
        dynamic_queries: work.dynamic_queries,
        pressure_evaluations: work.pressure_evaluations,
        retained_confirmations: work.retained_confirmations,
        hazard_updates: work.hazard_updates,
        layout_loads: work.layout_loads,
        index_builds: work.index_builds,
        worker_full_score_pair_visits: work.worker_full_score_pair_visits,
        auditor_full_score_pair_visits: work.auditor_full_score_pair_visits,
        auditor_dynamic_queries: work.auditor_dynamic_queries,
        auditor_pressure_evaluations: work.auditor_pressure_evaluations,
        auditor_layout_loads: work.auditor_layout_loads,
        auditor_index_builds: work.auditor_index_builds,
    }
}

#[cfg(feature = "jagua-experimental")]
#[derive(Clone, Copy, Debug, Default)]
struct CoupledSeparatorWork {
    worker_sweeps: usize,
    dynamic_queries: usize,
    pressure_evaluations: usize,
    retained_confirmations: usize,
    hazard_updates: usize,
    layout_loads: usize,
    index_builds: usize,
    worker_full_score_pair_visits: usize,
    auditor_full_score_pair_visits: usize,
    auditor_dynamic_queries: usize,
    auditor_pressure_evaluations: usize,
    auditor_layout_loads: usize,
    auditor_index_builds: usize,
}

#[cfg(feature = "jagua-experimental")]
impl CoupledSeparatorWork {
    fn accumulate(&mut self, other: Self) {
        self.worker_sweeps = self.worker_sweeps.saturating_add(other.worker_sweeps);
        self.dynamic_queries = self.dynamic_queries.saturating_add(other.dynamic_queries);
        self.pressure_evaluations = self
            .pressure_evaluations
            .saturating_add(other.pressure_evaluations);
        self.retained_confirmations = self
            .retained_confirmations
            .saturating_add(other.retained_confirmations);
        self.hazard_updates = self.hazard_updates.saturating_add(other.hazard_updates);
        self.layout_loads = self.layout_loads.saturating_add(other.layout_loads);
        self.index_builds = self.index_builds.saturating_add(other.index_builds);
        self.worker_full_score_pair_visits = self
            .worker_full_score_pair_visits
            .saturating_add(other.worker_full_score_pair_visits);
        self.auditor_full_score_pair_visits = self
            .auditor_full_score_pair_visits
            .saturating_add(other.auditor_full_score_pair_visits);
        self.auditor_dynamic_queries = self
            .auditor_dynamic_queries
            .saturating_add(other.auditor_dynamic_queries);
        self.auditor_pressure_evaluations = self
            .auditor_pressure_evaluations
            .saturating_add(other.auditor_pressure_evaluations);
        self.auditor_layout_loads = self
            .auditor_layout_loads
            .saturating_add(other.auditor_layout_loads);
        self.auditor_index_builds = self
            .auditor_index_builds
            .saturating_add(other.auditor_index_builds);
    }
}

#[cfg(feature = "jagua-experimental")]
fn coupled_auditor_score(
    worker: &Mutex<LaneSearch<'_>>,
    state: &RelaxedState,
    weights: &BTreeMap<(usize, usize), f64>,
    pair_visits_per_score: usize,
) -> (Result<PairTracker, GeneralFastError>, CoupledSeparatorWork) {
    let mut worker = match worker.lock() {
        Ok(worker) => worker,
        Err(_) => {
            return (
                Err(GeneralFastError::InvalidInput(
                    "coupled separator worker lock was poisoned".to_owned(),
                )),
                CoupledSeparatorWork::default(),
            );
        }
    };
    let saved_counters = worker.counters;
    worker.weights = weights.clone();
    let result = worker
        .prepare_dynamic_hazard(state)
        .and_then(|()| worker.score_state(state));
    let auditor_dynamic_queries = worker
        .counters
        .dynamic_hazard_queries
        .saturating_sub(saved_counters.dynamic_hazard_queries);
    let auditor_pressure_evaluations = worker
        .counters
        .dynamic_pressure_evaluations
        .saturating_sub(saved_counters.dynamic_pressure_evaluations);
    let auditor_layout_loads = worker
        .counters
        .dynamic_layout_loads
        .saturating_sub(saved_counters.dynamic_layout_loads);
    let auditor_index_builds = worker
        .counters
        .dynamic_index_builds
        .saturating_sub(saved_counters.dynamic_index_builds);
    worker.counters = saved_counters;
    (
        result,
        CoupledSeparatorWork {
            dynamic_queries: auditor_dynamic_queries,
            pressure_evaluations: auditor_pressure_evaluations,
            layout_loads: auditor_layout_loads,
            index_builds: auditor_index_builds,
            auditor_full_score_pair_visits: usize::from(auditor_layout_loads > 0)
                .saturating_mul(pair_visits_per_score),
            auditor_dynamic_queries,
            auditor_pressure_evaluations,
            auditor_layout_loads,
            auditor_index_builds,
            ..CoupledSeparatorWork::default()
        },
    )
}

#[cfg(feature = "jagua-experimental")]
fn run_coupled_separator_target<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    incumbent: &GeneralFastResult,
    initial_state: RelaxedState,
    target_ordinal: usize,
    target_depth_mm: f64,
    compression_split_mm: f64,
    target_seed: u64,
    compression_seed: u64,
    worker_seeds: Vec<u64>,
    arm: CoupledSeparatorArm,
    rollback_rescore_policy: CoupledRollbackRescorePolicy,
    independent_final_audit: bool,
    catalog: Arc<SurrogateCatalog>,
    hazard_catalog: Arc<JaguaHazardCatalog>,
) -> Result<CoupledTargetOutcome, GeneralFastError> {
    let pair_visits_per_score = pieces.len().saturating_mul(pieces.len().saturating_sub(1)) / 2;
    let workers = worker_seeds
        .iter()
        .map(|seed| {
            let mut worker = LaneSearch::new(
                pieces,
                fast_settings,
                relaxed_settings,
                *seed,
                catalog.clone(),
            );
            worker.dynamic_query_limit = Some(COUPLED_SEPARATOR_WORKER_QUERY_CAP);
            worker.hazard_catalog = Some(hazard_catalog.clone());
            worker.refine_rotation = arm.refines_rotation();
            Mutex::new(worker)
        })
        .collect::<Vec<_>>();
    let mut weights = BTreeMap::new();
    let initial_state_fingerprint = coupled_state_fingerprint(&initial_state);
    let mut auditor_work = CoupledSeparatorWork::default();
    let (initial_score, initial_audit_work) =
        coupled_auditor_score(&workers[0], &initial_state, &weights, pair_visits_per_score);
    auditor_work.accumulate(initial_audit_work);
    let initial_score = match initial_score {
        Ok(score) => score,
        Err(error) => {
            let (mut work, accounting_failure) =
                match coupled_separator_work(&workers, 0, pair_visits_per_score) {
                    Ok(work) => (work, None),
                    Err(accounting_error) => (
                        CoupledSeparatorWork::default(),
                        Some(format!("; work accounting: {accounting_error}")),
                    ),
                };
            work.accumulate(auditor_work);
            return Ok(CoupledTargetOutcome {
                diagnostics: GeneralCoupledSeparatorTargetDiagnostics {
                    ordinal: target_ordinal,
                    target_depth_mm,
                    compression_split_mm,
                    target_seed,
                    compression_seed,
                    worker_seeds,
                    initial_state_fingerprint: initial_state_fingerprint.clone(),
                    final_state_fingerprint: initial_state_fingerprint,
                    rounds: 0,
                    strikes: 0,
                    rollbacks: 0,
                    full_rescore_agreements: 0,
                    initial_raw_loss: 0.0,
                    minimum_raw_loss: 0.0,
                    final_raw_loss: 0.0,
                    final_weighted_loss: 0.0,
                    feasible: false,
                    exact_valid: false,
                    exact_accepted: false,
                    exact_rejection_reason: None,
                    accepted_depth_mm: None,
                    boundary_projection: None,
                    cap_exhausted: None,
                    failure_reason: Some(format!(
                        "initial full score: {error}{}",
                        accounting_failure.unwrap_or_default()
                    )),
                },
                accepted: None,
                work,
                minimum: None,
                final_state: initial_state,
                exact_metrics: None,
                independent_audit: None,
            });
        }
    };
    let initial_raw_loss = initial_score.common_loss();
    let mut master = LaneOutcome {
        state: initial_state.clone(),
        score: initial_score.clone(),
        weights: weights.clone(),
        counters: WorkCounters::default(),
        selected_lane: 0,
        restart_disruptions: 0,
    };
    let mut minimum_raw_state = initial_state;
    let mut minimum_raw_score = initial_score;
    let mut no_improvement = 0usize;
    let mut strikes = 0usize;
    let mut strike_start_raw_loss = initial_raw_loss;
    let mut rollbacks = 0usize;
    let mut full_rescore_agreements = 0usize;
    let mut rounds = 0usize;
    let mut cap_exhausted = None;
    let mut failure_reason = None;
    for round in 0..COUPLED_SEPARATOR_ROUNDS {
        let ordinals = (0..COUPLED_SEPARATOR_WORKERS).collect::<Vec<_>>();
        let outcomes = map_slice_with_job_pool(&ordinals, |ordinal| {
            let mut worker = workers[*ordinal].lock().map_err(|_| {
                GeneralFastError::InvalidInput(
                    "coupled separator worker lock was poisoned".to_owned(),
                )
            })?;
            worker.weights = weights.clone();
            worker.run_sweep(master.state.clone(), round)
        });
        let mut selected = None::<(usize, LaneOutcome)>;
        for (ordinal, outcome) in outcomes.into_iter().enumerate() {
            let outcome = match outcome {
                Ok(outcome) => outcome,
                Err(error) => {
                    failure_reason = Some(format!("worker {ordinal}: {error}"));
                    break;
                }
            };
            if selected
                .as_ref()
                .is_none_or(|(selected_ordinal, selected_outcome)| {
                    compare_coupled_separator_outcomes(
                        ordinal,
                        &outcome,
                        *selected_ordinal,
                        selected_outcome,
                    ) == Ordering::Less
                })
            {
                selected = Some((ordinal, outcome));
            }
        }
        rounds = rounds.saturating_add(1);
        if failure_reason.is_some() {
            break;
        }
        match coupled_separator_cap_reason(&workers, rounds, pair_visits_per_score) {
            Ok(Some(reason)) => {
                cap_exhausted = Some(reason);
                break;
            }
            Ok(None) => {}
            Err(error) => {
                failure_reason = Some(format!("cap accounting: {error}"));
                break;
            }
        }
        let Some((selected_ordinal, mut selected)) = selected else {
            failure_reason = Some("no worker outcome was available".to_owned());
            break;
        };
        selected.selected_lane = selected_ordinal;
        selected.weights = weights.clone();
        let previous_minimum = minimum_raw_score.common_loss();
        let transition = match coupled_round_disposition(&selected.score, previous_minimum) {
            CoupledRoundDisposition::AcceptFeasible => {
                master = selected;
                break;
            }
            CoupledRoundDisposition::ContinueInfeasible(transition) => transition,
        };
        match transition {
            RawMinimumTransition::SubstantialImprovement => {
                minimum_raw_state = selected.state.clone();
                minimum_raw_score = selected.score.clone();
                no_improvement = 0;
            }
            RawMinimumTransition::MinorImprovement => {
                minimum_raw_state = selected.state.clone();
                minimum_raw_score = selected.score.clone();
            }
            RawMinimumTransition::NoImprovement => {
                no_improvement = no_improvement.saturating_add(1);
            }
        }
        apply_coupled_gls_update(&mut weights, &mut selected);
        master = selected;

        let mut reached_strike_limit = false;
        if no_improvement >= COUPLED_SEPARATOR_NO_IMPROVEMENT_LIMIT {
            no_improvement = 0;
            match rollback_rescore_policy {
                CoupledRollbackRescorePolicy::StrictDerivedAgreement => {
                    if minimum_raw_score.common_loss()
                        < strike_start_raw_loss * COUPLED_SEPARATOR_SUBSTANTIAL_RATIO
                    {
                        strikes = 0;
                    } else {
                        strikes = strikes.saturating_add(1);
                    }
                    strike_start_raw_loss = minimum_raw_score.common_loss();
                    let (restored_score, rollback_audit_work) = coupled_auditor_score(
                        &workers[0],
                        &minimum_raw_state,
                        &weights,
                        pair_visits_per_score,
                    );
                    auditor_work.accumulate(rollback_audit_work);
                    let restored_score = match restored_score {
                        Ok(score) => score,
                        Err(error) => {
                            failure_reason = Some(format!("rollback full score: {error}"));
                            break;
                        }
                    };
                    if let Some(disagreement) =
                        raw_tracker_disagreement(&restored_score, &minimum_raw_score)
                    {
                        failure_reason = Some(format!(
                            "rollback tracker disagrees with a complete rescore: {disagreement}"
                        ));
                        break;
                    }
                    master = LaneOutcome {
                        state: minimum_raw_state.clone(),
                        score: restored_score,
                        weights: weights.clone(),
                        counters: WorkCounters::default(),
                        selected_lane: 0,
                        restart_disruptions: 0,
                    };
                    reached_strike_limit = strikes >= COUPLED_SEPARATOR_STRIKE_LIMIT;
                }
                CoupledRollbackRescorePolicy::CanonicalAuthoritativeRows => {
                    let (restored_score, rollback_audit_work) = coupled_auditor_score(
                        &workers[0],
                        &minimum_raw_state,
                        &weights,
                        pair_visits_per_score,
                    );
                    auditor_work.accumulate(rollback_audit_work);
                    let restored_score = match restored_score {
                        Ok(score) => score,
                        Err(error) => {
                            failure_reason = Some(format!("rollback full score: {error}"));
                            break;
                        }
                    };
                    if let Some(disagreement) =
                        authoritative_raw_tracker_disagreement(&restored_score, &minimum_raw_score)
                    {
                        failure_reason = Some(format!(
                            "rollback tracker disagrees with a complete rescore: {disagreement}"
                        ));
                        break;
                    }
                    reached_strike_limit = install_canonical_coupled_rollback(
                        restored_score,
                        &minimum_raw_state,
                        &mut minimum_raw_score,
                        &mut master,
                        &weights,
                        &mut strikes,
                        &mut strike_start_raw_loss,
                    );
                }
            }
            full_rescore_agreements = full_rescore_agreements.saturating_add(1);
            rollbacks = rollbacks.saturating_add(1);
        }

        if reached_strike_limit {
            break;
        }
    }

    let mut work = match coupled_separator_work(&workers, rounds, pair_visits_per_score) {
        Ok(work) => work,
        Err(error) => {
            failure_reason.get_or_insert_with(|| format!("work accounting: {error}"));
            CoupledSeparatorWork::default()
        }
    };
    work.accumulate(auditor_work);
    if work.auditor_layout_loads > COUPLED_SEPARATOR_AUDITOR_FULL_SCORES {
        cap_exhausted = Some("auditor full-score cap".to_owned());
    }
    let mut independent_audit = None;
    if independent_final_audit
        && failure_reason.is_none()
        && cap_exhausted.is_none()
        && !master.score.feasible()
    {
        let mut audit_diagnostics = GeneralPrecompressionIndependentAuditDiagnostics {
            attempted: true,
            ..GeneralPrecompressionIndependentAuditDiagnostics::default()
        };
        let (fresh_score, fresh_audit_work) =
            coupled_auditor_score(&workers[0], &master.state, &weights, pair_visits_per_score);
        work.accumulate(fresh_audit_work);
        if work.auditor_layout_loads > COUPLED_SEPARATOR_AUDITOR_FULL_SCORES {
            cap_exhausted = Some("auditor full-score cap".to_owned());
            audit_diagnostics.rejection_reason = Some("auditor full-score cap".to_owned());
            independent_audit = Some(CoupledIndependentAuditOutcome {
                diagnostics: audit_diagnostics,
                metrics: None,
            });
        } else {
            match fresh_score {
                Err(error) => {
                    audit_diagnostics.rejection_reason = Some(format!("final full score: {error}"));
                    independent_audit = Some(CoupledIndependentAuditOutcome {
                        diagnostics: audit_diagnostics,
                        metrics: None,
                    });
                }
                Ok(fresh_score) => {
                    if let Some(disagreement) = coupled_tracker_disagreement(
                        &fresh_score,
                        &master.score,
                        rollback_rescore_policy,
                    ) {
                        audit_diagnostics.rejection_reason = Some(format!(
                            "final tracker disagrees with a complete rescore: {disagreement}"
                        ));
                        independent_audit = Some(CoupledIndependentAuditOutcome {
                            diagnostics: audit_diagnostics,
                            metrics: None,
                        });
                    } else {
                        audit_diagnostics.fresh_score_agreement = true;
                        audit_diagnostics.final_positive_pairs =
                            Some(fresh_score.collision_pairs.len());
                        audit_diagnostics.final_boundary_violations =
                            Some(fresh_score.boundary_violations);
                        audit_diagnostics.final_boundary_loss = Some(fresh_score.boundary_loss);
                        audit_diagnostics.positive_boundary_rows = fresh_score
                            .boundaries
                            .iter()
                            .enumerate()
                            .filter(|(_, boundary)| {
                                boundary.violations > 0 || boundary.raw_loss > 0.0
                            })
                            .map(|(piece_index, boundary)| {
                                GeneralPrecompressionBoundaryRowDiagnostics {
                                    piece_id: pieces[piece_index].id.to_owned(),
                                    violations: boundary.violations,
                                    raw_loss: boundary.raw_loss,
                                }
                            })
                            .collect();
                        let placements = to_fast_placements(&master.state, pieces);
                        audit_diagnostics.audited_placement_fingerprint =
                            Some(coupled_fast_placement_fingerprint(&placements));
                        if fresh_score.collision_pairs.is_empty() {
                            audit_diagnostics.independent_audit_count = 1;
                            match validate_and_measure_placements(
                                pieces,
                                &placements,
                                fast_settings,
                            ) {
                                Ok(metrics) => {
                                    audit_diagnostics.independent_audit_valid = true;
                                    audit_diagnostics.used_short_axis_span_mm =
                                        Some(metrics.used_short_axis_span_mm);
                                    audit_diagnostics.used_long_axis_depth_mm =
                                        Some(metrics.used_long_axis_depth_mm);
                                    audit_diagnostics.unused_short_axis_projection_mm =
                                        Some(metrics.unused_short_axis_projection_mm);
                                    audit_diagnostics.occupied_envelope_area_mm2 =
                                        Some(metrics.occupied_envelope_area_mm2);
                                    independent_audit = Some(CoupledIndependentAuditOutcome {
                                        diagnostics: audit_diagnostics,
                                        metrics: Some(metrics),
                                    });
                                }
                                Err(error) => {
                                    audit_diagnostics.rejection_reason = Some(error.to_string());
                                    independent_audit = Some(CoupledIndependentAuditOutcome {
                                        diagnostics: audit_diagnostics,
                                        metrics: None,
                                    });
                                }
                            }
                        } else {
                            audit_diagnostics.rejection_reason = Some(
                                "fresh final score retained positive collision pairs".to_owned(),
                            );
                            independent_audit = Some(CoupledIndependentAuditOutcome {
                                diagnostics: audit_diagnostics,
                                metrics: None,
                            });
                        }
                    }
                }
            }
        }
    }
    let mut exact_valid = false;
    let mut exact_accepted = false;
    let mut exact_rejection_reason = None;
    let mut accepted_depth_mm = None;
    let mut exact_metrics = None;
    let accepted = if failure_reason.is_none() && cap_exhausted.is_none() && master.score.feasible()
    {
        let placements = to_fast_placements(&master.state, pieces);
        match validate_and_measure_placements(pieces, &placements, fast_settings) {
            Ok(metrics) => {
                exact_valid = true;
                exact_metrics = Some(metrics);
                if metrics.used_long_axis_depth_mm < incumbent.used_long_axis_depth_mm {
                    exact_accepted = true;
                    accepted_depth_mm = Some(metrics.used_long_axis_depth_mm);
                    let mut result = incumbent.clone();
                    result.placements = placements;
                    result.unplaced_piece_ids.clear();
                    result.used_short_axis_span_mm = metrics.used_short_axis_span_mm;
                    result.used_long_axis_depth_mm = metrics.used_long_axis_depth_mm;
                    result.unused_short_axis_projection_mm =
                        metrics.unused_short_axis_projection_mm;
                    result.occupied_envelope_area_mm2 = metrics.occupied_envelope_area_mm2;
                    Some(result)
                } else {
                    exact_rejection_reason =
                        Some("exact-valid endpoint did not improve the incumbent".to_owned());
                    None
                }
            }
            Err(error) => {
                exact_rejection_reason = Some(error.to_string());
                None
            }
        }
    } else {
        None
    };
    let diagnostics = GeneralCoupledSeparatorTargetDiagnostics {
        ordinal: target_ordinal,
        target_depth_mm,
        compression_split_mm,
        target_seed,
        compression_seed,
        worker_seeds,
        initial_state_fingerprint,
        final_state_fingerprint: coupled_state_fingerprint(&master.state),
        rounds,
        strikes,
        rollbacks,
        full_rescore_agreements,
        initial_raw_loss,
        minimum_raw_loss: minimum_raw_score.common_loss(),
        final_raw_loss: master.score.common_loss(),
        final_weighted_loss: master.score.weighted_loss,
        feasible: master.score.feasible(),
        exact_valid,
        exact_accepted,
        exact_rejection_reason,
        accepted_depth_mm,
        boundary_projection: None,
        cap_exhausted,
        failure_reason,
    };
    Ok(CoupledTargetOutcome {
        diagnostics,
        accepted,
        work,
        minimum: Some(CoupledMinimumCheckpoint {
            state: minimum_raw_state,
            score: minimum_raw_score,
        }),
        final_state: master.state,
        exact_metrics,
        independent_audit,
    })
}

#[cfg(feature = "jagua-experimental")]
fn raw_minimum_transition(candidate: f64, retained: f64) -> RawMinimumTransition {
    if candidate >= retained {
        RawMinimumTransition::NoImprovement
    } else if candidate < retained * COUPLED_SEPARATOR_SUBSTANTIAL_RATIO {
        RawMinimumTransition::SubstantialImprovement
    } else {
        RawMinimumTransition::MinorImprovement
    }
}

#[cfg(feature = "jagua-experimental")]
fn coupled_round_disposition(
    score: &PairTracker,
    retained_raw_loss: f64,
) -> CoupledRoundDisposition {
    if score.feasible() {
        CoupledRoundDisposition::AcceptFeasible
    } else {
        CoupledRoundDisposition::ContinueInfeasible(raw_minimum_transition(
            score.common_loss(),
            retained_raw_loss,
        ))
    }
}

#[cfg(feature = "jagua-experimental")]
fn apply_coupled_gls_update(
    weights: &mut BTreeMap<(usize, usize), f64>,
    selected: &mut LaneOutcome,
) {
    update_weights(weights, &selected.score.collision_pairs);
    selected.weights = weights.clone();
    refresh_weighted_loss(&mut selected.score, weights);
}

#[cfg(feature = "jagua-experimental")]
fn compare_coupled_separator_outcomes(
    first_ordinal: usize,
    first: &LaneOutcome,
    second_ordinal: usize,
    second: &LaneOutcome,
) -> Ordering {
    first
        .score
        .weighted_loss
        .total_cmp(&second.score.weighted_loss)
        .then_with(|| {
            first
                .score
                .common_loss()
                .total_cmp(&second.score.common_loss())
        })
        .then_with(|| {
            first
                .score
                .boundary_loss
                .total_cmp(&second.score.boundary_loss)
        })
        .then_with(|| {
            first
                .score
                .boundary_violations
                .cmp(&second.score.boundary_violations)
        })
        .then_with(|| {
            first
                .score
                .collision_pairs
                .len()
                .cmp(&second.score.collision_pairs.len())
        })
        .then_with(|| canonical_state_key(&first.state).cmp(&canonical_state_key(&second.state)))
        .then_with(|| first_ordinal.cmp(&second_ordinal))
}

#[cfg(feature = "jagua-experimental")]
fn install_canonical_coupled_rollback(
    restored_score: PairTracker,
    minimum_raw_state: &RelaxedState,
    minimum_raw_score: &mut PairTracker,
    master: &mut LaneOutcome,
    weights: &BTreeMap<(usize, usize), f64>,
    strikes: &mut usize,
    strike_start_raw_loss: &mut f64,
) -> bool {
    let canonical_raw_loss = restored_score.common_loss();
    if canonical_raw_loss < *strike_start_raw_loss * COUPLED_SEPARATOR_SUBSTANTIAL_RATIO {
        *strikes = 0;
    } else {
        *strikes = strikes.saturating_add(1);
    }
    *strike_start_raw_loss = canonical_raw_loss;
    *minimum_raw_score = restored_score.clone();
    *master = LaneOutcome {
        state: minimum_raw_state.clone(),
        score: restored_score,
        weights: weights.clone(),
        counters: WorkCounters::default(),
        selected_lane: 0,
        restart_disruptions: 0,
    };
    *strikes >= COUPLED_SEPARATOR_STRIKE_LIMIT
}

#[cfg(feature = "jagua-experimental")]
fn raw_tracker_disagreement(first: &PairTracker, second: &PairTracker) -> Option<String> {
    if let Some(disagreement) = authoritative_raw_tracker_disagreement(first, second) {
        return Some(disagreement);
    }
    if first.incident_raw_loss.len() != second.incident_raw_loss.len()
        || first
            .incident_raw_loss
            .iter()
            .zip(&second.incident_raw_loss)
            .any(|(first, second)| !equal_within_one_ulp(*first, *second))
    {
        let difference = first
            .incident_raw_loss
            .iter()
            .zip(&second.incident_raw_loss)
            .enumerate()
            .find(|(_, (first, second))| first != second)
            .map(|(index, (first, second))| {
                format!("incident loss {index}: {first:.17e} != {second:.17e}")
            })
            .unwrap_or_else(|| "incident loss vector length differs".to_owned());
        return Some(difference);
    }
    if !equal_within_one_ulp(first.boundary_loss, second.boundary_loss) {
        return Some(format!(
            "boundary loss {:.17e} != {:.17e}",
            first.boundary_loss, second.boundary_loss
        ));
    }
    None
}

#[cfg(feature = "jagua-experimental")]
fn authoritative_raw_tracker_disagreement(
    first: &PairTracker,
    second: &PairTracker,
) -> Option<String> {
    if first.piece_count != second.piece_count {
        return Some(format!(
            "piece count {} != {}",
            first.piece_count, second.piece_count
        ));
    }
    if first.boundaries != second.boundaries {
        return Some("boundary rows differ".to_owned());
    }
    if first.boundary_violations != second.boundary_violations {
        return Some(format!(
            "boundary violation count {} != {}",
            first.boundary_violations, second.boundary_violations
        ));
    }
    if first.collision_pairs != second.collision_pairs {
        return Some("collision rows differ".to_owned());
    }
    if first.pairs.len() != second.pairs.len()
        || first
            .pairs
            .iter()
            .zip(&second.pairs)
            .any(|(first, second)| {
                first.raw_loss != second.raw_loss
                    || first.normalization_scale != second.normalization_scale
            })
    {
        return Some("pair rows differ".to_owned());
    }
    None
}

#[cfg(feature = "jagua-experimental")]
fn coupled_tracker_disagreement(
    first: &PairTracker,
    second: &PairTracker,
    policy: CoupledRollbackRescorePolicy,
) -> Option<String> {
    match policy {
        CoupledRollbackRescorePolicy::StrictDerivedAgreement => {
            raw_tracker_disagreement(first, second)
        }
        CoupledRollbackRescorePolicy::CanonicalAuthoritativeRows => {
            authoritative_raw_tracker_disagreement(first, second)
        }
    }
}

#[cfg(feature = "jagua-experimental")]
fn equal_within_one_ulp(first: f64, second: f64) -> bool {
    first == second
        || (first.is_finite()
            && second.is_finite()
            && first.is_sign_negative() == second.is_sign_negative()
            && first.to_bits().abs_diff(second.to_bits()) <= 1)
}

#[cfg(feature = "jagua-experimental")]
fn coupled_separator_cap_reason(
    workers: &[Mutex<LaneSearch<'_>>],
    rounds: usize,
    pair_visits_per_score: usize,
) -> Result<Option<String>, GeneralFastError> {
    let full_score_pair_visits = rounds.saturating_mul(pair_visits_per_score);
    if full_score_pair_visits > COUPLED_SEPARATOR_WORKER_FULL_SCORE_PAIR_VISIT_CAP {
        return Ok(Some("worker full-score pair-visit cap".to_owned()));
    }
    for (ordinal, worker) in workers.iter().enumerate() {
        let worker = worker.lock().map_err(|_| {
            GeneralFastError::InvalidInput("coupled separator worker lock was poisoned".to_owned())
        })?;
        let counters = worker.counters;
        let reason = if counters.dynamic_hazard_queries > COUPLED_SEPARATOR_WORKER_QUERY_CAP {
            Some("complete-query cap")
        } else if counters.dynamic_pressure_evaluations > COUPLED_SEPARATOR_WORKER_PRESSURE_CAP {
            Some("pressure-evaluation cap")
        } else if counters.retained_f64_confirmations > COUPLED_SEPARATOR_WORKER_CONFIRMATION_CAP {
            Some("retained-confirmation cap")
        } else if counters.dynamic_hazard_updates > COUPLED_SEPARATOR_WORKER_UPDATE_CAP {
            Some("hazard-update cap")
        } else if counters.dynamic_layout_loads > COUPLED_SEPARATOR_WORKER_LAYOUT_LOAD_CAP {
            Some("layout-load cap")
        } else if counters.dynamic_index_builds > 1 {
            Some("index-build cap")
        } else {
            None
        };
        if let Some(reason) = reason {
            return Ok(Some(format!("worker {ordinal} {reason}")));
        }
    }
    Ok(None)
}

#[cfg(feature = "jagua-experimental")]
fn coupled_separator_work(
    workers: &[Mutex<LaneSearch<'_>>],
    rounds: usize,
    pair_visits_per_score: usize,
) -> Result<CoupledSeparatorWork, GeneralFastError> {
    let mut work = CoupledSeparatorWork::default();
    for worker in workers {
        let worker = worker.lock().map_err(|_| {
            GeneralFastError::InvalidInput("coupled separator worker lock was poisoned".to_owned())
        })?;
        work.dynamic_queries = work
            .dynamic_queries
            .saturating_add(worker.counters.dynamic_hazard_queries);
        work.pressure_evaluations = work
            .pressure_evaluations
            .saturating_add(worker.counters.dynamic_pressure_evaluations);
        work.retained_confirmations = work
            .retained_confirmations
            .saturating_add(worker.counters.retained_f64_confirmations);
        work.hazard_updates = work
            .hazard_updates
            .saturating_add(worker.counters.dynamic_hazard_updates);
        work.layout_loads = work
            .layout_loads
            .saturating_add(worker.counters.dynamic_layout_loads);
        work.index_builds = work
            .index_builds
            .saturating_add(worker.counters.dynamic_index_builds);
    }
    work.worker_sweeps = rounds.saturating_mul(workers.len());
    work.worker_full_score_pair_visits = work.worker_sweeps.saturating_mul(pair_visits_per_score);
    Ok(work)
}

#[cfg(feature = "jagua-experimental")]
fn coupled_state_fingerprint(state: &RelaxedState) -> String {
    let mut digest = Sha256::new();
    digest.update(grid_key(state.strip_depth_mm).to_le_bytes());
    for (input_index, angle, mirrored, translate_x, translate_y) in canonical_state_key(state) {
        digest.update(input_index.to_le_bytes());
        digest.update(angle.to_le_bytes());
        digest.update([u8::from(mirrored)]);
        digest.update(translate_x.to_le_bytes());
        digest.update(translate_y.to_le_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn coupled_placement_diagnostics(
    placements: &[GeneralFastPlacement],
) -> Vec<GeneralCoupledSeparatorPlacementDiagnostics> {
    placements
        .iter()
        .map(|placement| GeneralCoupledSeparatorPlacementDiagnostics {
            piece_id: placement.piece_id.clone(),
            rotation_deg: placement.rotation_deg,
            mirrored: placement.mirrored,
            translate_short_axis: placement.translate_short_axis,
            translate_long_axis: placement.translate_long_axis,
        })
        .collect()
}

fn coupled_fast_placement_fingerprint(placements: &[GeneralFastPlacement]) -> String {
    let mut canonical = placements.iter().collect::<Vec<_>>();
    canonical.sort_by(|first, second| first.piece_id.cmp(&second.piece_id));
    let mut digest = Sha256::new();
    for placement in canonical {
        digest.update((placement.piece_id.len() as u64).to_le_bytes());
        digest.update(placement.piece_id.as_bytes());
        digest.update(angle_key(placement.rotation_deg).to_le_bytes());
        digest.update([u8::from(placement.mirrored)]);
        digest.update(grid_key(placement.translate_short_axis).to_le_bytes());
        digest.update(grid_key(placement.translate_long_axis).to_le_bytes());
    }
    format!("{:x}", digest.finalize())
}

#[cfg(feature = "jagua-experimental")]
fn coupled_independent_source_depth(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
) -> Result<f64, GeneralFastError> {
    let pieces_by_id = pieces
        .iter()
        .map(|piece| (piece.id, piece))
        .collect::<BTreeMap<_, _>>();
    let edge_clearance_mm = settings
        .sheet_edge_clearance_mm
        .unwrap_or(settings.total_padding_mm / 2.0);
    placements
        .iter()
        .map(|placement| {
            let piece = pieces_by_id
                .get(placement.piece_id.as_str())
                .ok_or_else(|| {
                    GeneralFastError::InvalidInput(format!(
                        "a coupled placement references unknown piece {}",
                        placement.piece_id
                    ))
                })?;
            let transformed = piece.polygon.transformed(
                placement.rotation_deg,
                placement.mirrored,
                placement.translate_short_axis,
                placement.translate_long_axis,
            )?;
            let bounds = transformed.bounds().ok_or_else(|| {
                GeneralFastError::InvalidInput(
                    "a coupled source polygon must be non-empty".to_owned(),
                )
            })?;
            Ok(bounds.max_y + edge_clearance_mm)
        })
        .collect::<Result<Vec<_>, GeneralFastError>>()?
        .into_iter()
        .max_by(f64::total_cmp)
        .ok_or_else(|| {
            GeneralFastError::InvalidInput(
                "coupled diagnostics must retain at least one placement".to_owned(),
            )
        })
}

fn run_independent_lanes<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    initial_state: &RelaxedState,
    target_depth_mm: f64,
    epoch: usize,
    catalog: Arc<SurrogateCatalog>,
) -> Result<LaneBatch, GeneralFastError> {
    let lane_ordinals = (0..relaxed_settings.lanes).collect::<Vec<_>>();
    let lane_results = map_slice_with_job_pool(&lane_ordinals, |lane| {
        let seed = derive_seed(relaxed_settings.seed, epoch, *lane);
        let mut lane_state = initial_state.clone();
        let directional =
            relaxed_settings.pressure_model == GeneralRelaxedPressureModel::DirectionalPenetration;
        let disruption_count = if directional {
            0
        } else {
            lane_disruption_count(*lane)
        };
        if disruption_count > 0 {
            for disruption in 0..disruption_count {
                lane_state =
                    disrupt_state_legacy(lane_state, pieces, derive_seed(seed, disruption, *lane))?;
            }
        }
        let mut search = LaneSearch::new(
            pieces,
            fast_settings,
            relaxed_settings,
            seed,
            catalog.clone(),
        );
        let compressed = if directional {
            search
                .compress_directional_state(&lane_state, target_depth_mm)?
                .unwrap_or(lane_state)
        } else {
            compress_state_at_split(
                &lane_state,
                target_depth_mm,
                compression_split(seed, lane_state.strip_depth_mm, fast_settings),
                pieces,
            )?
        };
        search.run(compressed)
    });
    let mut outcomes = Vec::with_capacity(lane_results.len());
    let mut total = WorkCounters::default();
    let mut first_directional_rejection = None;
    for (lane_ordinal, lane_result) in lane_results.into_iter().enumerate() {
        let mut lane = match lane_result {
            Ok(lane) => lane,
            Err(error) if is_directional_lane_unscorable(&error) => {
                total.directional_lane_rejections =
                    total.directional_lane_rejections.saturating_add(1);
                if first_directional_rejection.is_none() {
                    first_directional_rejection = Some(error);
                }
                continue;
            }
            Err(error) => return Err(error),
        };
        total.accumulate(lane.counters);
        lane.selected_lane = lane_ordinal;
        lane.restart_disruptions = if relaxed_settings.pressure_model
            == GeneralRelaxedPressureModel::DirectionalPenetration
        {
            0
        } else {
            lane_disruption_count(lane_ordinal)
        };
        outcomes.push(lane);
    }
    if outcomes.is_empty() {
        if relaxed_settings.pressure_model == GeneralRelaxedPressureModel::DirectionalPenetration {
            return Err(first_directional_rejection
                .unwrap_or_else(|| directional_lane_unscorable_error("all lanes rejected")));
        }
        return Err(GeneralFastError::InvalidSettings(
            "relaxed search requires at least one lane".to_owned(),
        ));
    }
    Ok(LaneBatch {
        outcomes,
        counters: total,
    })
}

fn select_lane_for_publication(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    outcomes: Vec<LaneOutcome>,
    diagnostics: &mut GeneralRelaxedDiagnostics,
) -> SelectedLane {
    let mut outcomes = outcomes.into_iter().map(Some).collect::<Vec<_>>();
    let mut valid = Vec::<(usize, Vec<GeneralFastPlacement>, GeneralPlacementMetrics)>::new();
    for (index, outcome) in outcomes.iter().enumerate() {
        let outcome = outcome.as_ref().expect("lane outcomes are present");
        if !outcome.score.feasible() {
            continue;
        }
        diagnostics.surrogate_feasible_states =
            diagnostics.surrogate_feasible_states.saturating_add(1);
        let placements = to_fast_placements(&outcome.state, pieces);
        match validate_and_measure_placements(pieces, &placements, fast_settings) {
            Ok(metrics) => valid.push((index, placements, metrics)),
            Err(error) => {
                diagnostics.exact_rejected_states =
                    diagnostics.exact_rejected_states.saturating_add(1);
                diagnostics.exact_rejection_reasons.push(error.to_string());
            }
        }
    }
    if let Some((index, placements, metrics)) = valid.into_iter().min_by(
        |(first_index, _, first_metrics), (second_index, _, second_metrics)| {
            compare_exact_metrics(*first_metrics, *second_metrics).then_with(|| {
                let first = outcomes[*first_index]
                    .as_ref()
                    .expect("lane outcome is present");
                let second = outcomes[*second_index]
                    .as_ref()
                    .expect("lane outcome is present");
                canonical_state_key(&first.state)
                    .cmp(&canonical_state_key(&second.state))
                    .then_with(|| first.selected_lane.cmp(&second.selected_lane))
            })
        },
    ) {
        return SelectedLane {
            outcome: outcomes[index]
                .take()
                .expect("selected lane outcome is present"),
            validation: ExactLaneValidation::Accepted {
                placements,
                metrics,
            },
        };
    }
    let index = outcomes
        .iter()
        .enumerate()
        .filter_map(|(index, outcome)| outcome.as_ref().map(|outcome| (index, outcome)))
        .min_by(|(_, first), (_, second)| {
            compare_lane_outcomes(first.selected_lane, first, second.selected_lane, second)
        })
        .map(|(index, _)| index)
        .expect("relaxed search produces at least one lane");
    let outcome = outcomes[index]
        .take()
        .expect("selected lane outcome is present");
    let validation = if outcome.score.feasible() {
        ExactLaneValidation::Rejected
    } else {
        ExactLaneValidation::Infeasible
    };
    SelectedLane {
        outcome,
        validation,
    }
}

fn validate_selected_lane(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    lane: &LaneOutcome,
    diagnostics: &mut GeneralRelaxedDiagnostics,
) -> ExactLaneValidation {
    if !lane.score.feasible() {
        return ExactLaneValidation::Infeasible;
    }
    diagnostics.surrogate_feasible_states = diagnostics.surrogate_feasible_states.saturating_add(1);
    let placements = to_fast_placements(&lane.state, pieces);
    match validate_and_measure_placements(pieces, &placements, fast_settings) {
        Ok(metrics) => ExactLaneValidation::Accepted {
            placements,
            metrics,
        },
        Err(error) => {
            diagnostics.exact_rejected_states = diagnostics.exact_rejected_states.saturating_add(1);
            diagnostics.exact_rejection_reasons.push(error.to_string());
            ExactLaneValidation::Rejected
        }
    }
}

fn compare_exact_metrics(
    first: GeneralPlacementMetrics,
    second: GeneralPlacementMetrics,
) -> Ordering {
    first
        .used_long_axis_depth_mm
        .total_cmp(&second.used_long_axis_depth_mm)
        .then_with(|| {
            first
                .unused_short_axis_projection_mm
                .total_cmp(&second.unused_short_axis_projection_mm)
        })
        .then_with(|| {
            first
                .occupied_envelope_area_mm2
                .total_cmp(&second.occupied_envelope_area_mm2)
        })
}

fn lane_disruption_count(lane: usize) -> usize {
    if lane == 0 {
        0
    } else {
        1 + (lane - 1) % 3
    }
}

fn run_synchronized_lanes<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    relaxed_settings: GeneralRelaxedSettings,
    initial_state: &RelaxedState,
    target_depth_mm: f64,
    epoch: usize,
    catalog: Arc<SurrogateCatalog>,
) -> Result<LaneOutcome, GeneralFastError> {
    let lane_ordinals = (0..relaxed_settings.lanes).collect::<Vec<_>>();
    let workers = lane_ordinals
        .iter()
        .map(|lane| {
            Mutex::new(LaneSearch::new(
                pieces,
                fast_settings,
                relaxed_settings,
                derive_seed(relaxed_settings.seed, epoch, *lane),
                catalog.clone(),
            ))
        })
        .collect::<Vec<_>>();
    let mut master = None::<LaneOutcome>;
    let mut weights = BTreeMap::new();

    for sweep in 0..relaxed_settings.sweeps_per_epoch {
        let lane_results = map_slice_with_job_pool(&lane_ordinals, |lane| {
            let seed = derive_seed(relaxed_settings.seed, epoch, *lane);
            let state = if let Some(master) = &master {
                master.state.clone()
            } else {
                compress_state_at_split(
                    initial_state,
                    target_depth_mm,
                    compression_split(seed, initial_state.strip_depth_mm, fast_settings),
                    pieces,
                )?
            };
            let mut worker = workers[*lane].lock().map_err(|_| {
                GeneralFastError::InvalidInput("relaxed lane worker lock was poisoned".to_owned())
            })?;
            worker.weights = weights.clone();
            worker.run_sweep(state, sweep)
        });
        let mut best = None::<(usize, LaneOutcome)>;
        for (lane_ordinal, lane_result) in lane_results.into_iter().enumerate() {
            let lane = lane_result?;
            if best.as_ref().is_none_or(|(best_ordinal, best_lane)| {
                compare_lane_outcomes(lane_ordinal, &lane, *best_ordinal, best_lane)
                    == Ordering::Less
            }) {
                best = Some((lane_ordinal, lane));
            }
        }
        let Some((lane_ordinal, mut selected)) = best else {
            return Err(GeneralFastError::InvalidSettings(
                "relaxed search requires at least one lane".to_owned(),
            ));
        };
        selected.selected_lane = lane_ordinal;
        selected.restart_disruptions = 0;
        update_weights(&mut weights, &selected.score.collision_pairs);
        let feasible = selected.score.feasible();
        master = Some(selected);
        if feasible {
            break;
        }
    }

    let mut outcome = master.ok_or_else(|| {
        GeneralFastError::InvalidSettings("relaxed search requires at least one sweep".to_owned())
    })?;
    outcome.weights = weights;
    outcome.counters = workers.iter().try_fold(
        WorkCounters::default(),
        |mut total, worker| -> Result<_, GeneralFastError> {
            let worker = worker.lock().map_err(|_| {
                GeneralFastError::InvalidInput("relaxed lane worker lock was poisoned".to_owned())
            })?;
            total.ejection_chain_evaluations = total
                .ejection_chain_evaluations
                .saturating_add(worker.counters.ejection_chain_evaluations);
            total.ejection_chain_accepts = total
                .ejection_chain_accepts
                .saturating_add(worker.counters.ejection_chain_accepts);
            total.surrogate_evaluations = total
                .surrogate_evaluations
                .saturating_add(worker.counters.surrogate_evaluations);
            total.piece_broad_phase_probes = total
                .piece_broad_phase_probes
                .saturating_add(worker.counters.piece_broad_phase_probes);
            total.cell_index_probes = total
                .cell_index_probes
                .saturating_add(worker.counters.cell_index_probes);
            total.sat_tests = total.sat_tests.saturating_add(worker.counters.sat_tests);
            total.pair_nfp_builds = total
                .pair_nfp_builds
                .saturating_add(worker.counters.pair_nfp_builds);
            total.pair_nfp_components = total
                .pair_nfp_components
                .saturating_add(worker.counters.pair_nfp_components);
            total.shared_pair_nfp_adoptions = total
                .shared_pair_nfp_adoptions
                .saturating_add(worker.counters.shared_pair_nfp_adoptions);
            total.axis_events = total
                .axis_events
                .saturating_add(worker.counters.axis_events);
            total.axis_candidate_evaluations = total
                .axis_candidate_evaluations
                .saturating_add(worker.counters.axis_candidate_evaluations);
            total.dynamic_hazard_queries = total
                .dynamic_hazard_queries
                .saturating_add(worker.counters.dynamic_hazard_queries);
            total.dynamic_hazard_updates = total
                .dynamic_hazard_updates
                .saturating_add(worker.counters.dynamic_hazard_updates);
            total.dynamic_pressure_evaluations = total
                .dynamic_pressure_evaluations
                .saturating_add(worker.counters.dynamic_pressure_evaluations);
            total.translation_evaluations = total
                .translation_evaluations
                .saturating_add(worker.counters.translation_evaluations);
            total.rotation_evaluations = total
                .rotation_evaluations
                .saturating_add(worker.counters.rotation_evaluations);
            total.retained_f64_confirmations = total
                .retained_f64_confirmations
                .saturating_add(worker.counters.retained_f64_confirmations);
            total.confirmed_pair_additions = total
                .confirmed_pair_additions
                .saturating_add(worker.counters.confirmed_pair_additions);
            total.confirmed_pair_removals = total
                .confirmed_pair_removals
                .saturating_add(worker.counters.confirmed_pair_removals);
            total.accepted_moves = total
                .accepted_moves
                .saturating_add(worker.counters.accepted_moves);
            Ok(total)
        },
    )?;
    Ok(outcome)
}

impl<'a> LaneSearch<'a> {
    fn new(
        pieces: &'a [GeneralFastPiece<'a>],
        fast_settings: GeneralFastSettings,
        relaxed_settings: GeneralRelaxedSettings,
        seed: u64,
        catalog: Arc<SurrogateCatalog>,
    ) -> Self {
        Self {
            pieces,
            fast_settings,
            relaxed_settings,
            catalog,
            rng: SplitMix64::new(seed),
            weights: BTreeMap::new(),
            counters: WorkCounters::default(),
            allow_worsening_chain: false,
            piece_query_scratch: PieceQueryScratch::new(pieces.len()),
            pair_nfp_cache: BTreeMap::new(),
            pair_nfp_cache_components: 0,
            #[cfg(feature = "jagua-experimental")]
            hazard_index: None,
            #[cfg(feature = "jagua-experimental")]
            hazard_catalog: None,
            dynamic_query_limit: None,
            refine_rotation: false,
        }
    }

    fn uses_dynamic_hazard(&self) -> bool {
        self.relaxed_settings.collision_backend == GeneralRelaxedCollisionBackend::DynamicHazard
    }

    #[cfg(feature = "jagua-experimental")]
    fn uses_dynamic_pressure(&self) -> bool {
        self.uses_dynamic_hazard()
            && self.relaxed_settings.pressure_model == GeneralRelaxedPressureModel::DynamicPoles
    }

    fn uses_continuous_triangle_pressure(&self) -> bool {
        self.uses_dynamic_hazard()
            && self.relaxed_settings.pressure_model
                == GeneralRelaxedPressureModel::ContinuousTrianglePoles
    }

    fn uses_directional_pressure(&self) -> bool {
        self.relaxed_settings.collision_backend == GeneralRelaxedCollisionBackend::RollbackTriangle
            && self.relaxed_settings.pressure_model
                == GeneralRelaxedPressureModel::DirectionalPenetration
    }

    fn directional_inner_fit(
        &self,
        placement: &RelaxedPlacement,
        strip_depth_mm: f64,
    ) -> Result<Option<GridInnerFit>, GeneralFastError> {
        let shape = self.oriented(
            placement.input_index,
            placement.rotation_deg,
            placement.mirrored,
        )?;
        let coordinate = |value: f64, label: &str| {
            grid_coordinate(value).ok_or_else(|| {
                GeneralFastError::InvalidInput(format!(
                    "directional {label} is outside the canonical grid"
                ))
            })
        };
        let inset = coordinate(collision_sheet_inset_mm(self.fast_settings), "sheet inset")?;
        let sheet_short = coordinate(self.fast_settings.sheet_short_axis_mm, "sheet short axis")?;
        let strip_depth = coordinate(strip_depth_mm, "strip depth")?;
        let local_min_x = coordinate(shape.bounds.min_x, "local minimum x")?;
        let local_max_x = coordinate(shape.bounds.max_x, "local maximum x")?;
        let local_min_y = coordinate(shape.bounds.min_y, "local minimum y")?;
        let local_max_y = coordinate(shape.bounds.max_y, "local maximum y")?;
        let min_x = inset.checked_sub(local_min_x);
        let max_x = sheet_short
            .checked_sub(inset)
            .and_then(|value| value.checked_sub(local_max_x));
        let min_y = inset.checked_sub(local_min_y);
        let max_y = strip_depth
            .checked_sub(inset)
            .and_then(|value| value.checked_sub(local_max_y));
        let (Some(min_x), Some(max_x), Some(min_y), Some(max_y)) = (min_x, max_x, min_y, max_y)
        else {
            return Err(GeneralFastError::InvalidInput(
                "directional inner-fit arithmetic overflowed".to_owned(),
            ));
        };
        Ok((min_x <= max_x && min_y <= max_y).then_some(GridInnerFit {
            min_x,
            max_x,
            min_y,
            max_y,
        }))
    }

    fn directional_position(
        &self,
        placement: &RelaxedPlacement,
    ) -> Result<(i128, i128), GeneralFastError> {
        let x = grid_coordinate(placement.translate_x).ok_or_else(|| {
            GeneralFastError::InvalidInput(
                "directional horizontal placement is outside the canonical grid".to_owned(),
            )
        })?;
        let y = grid_coordinate(placement.translate_y).ok_or_else(|| {
            GeneralFastError::InvalidInput(
                "directional vertical placement is outside the canonical grid".to_owned(),
            )
        })?;
        Ok((x, y))
    }

    fn directional_contains(
        &self,
        placement: &RelaxedPlacement,
        strip_depth_mm: f64,
    ) -> Result<bool, GeneralFastError> {
        let Some(inner_fit) = self.directional_inner_fit(placement, strip_depth_mm)? else {
            return Ok(false);
        };
        let (x, y) = self.directional_position(placement)?;
        Ok(inner_fit.contains(x, y))
    }

    fn directional_relative_point(
        &self,
        fixed: &RelaxedPlacement,
        moving: &RelaxedPlacement,
    ) -> Result<IrregularPoint, GeneralFastError> {
        let relative_x = relative_grid_coordinate(fixed.translate_x, moving.translate_x)
            .ok_or_else(|| {
                GeneralFastError::InvalidInput(
                    "directional horizontal translation difference is outside the canonical grid"
                        .to_owned(),
                )
            })?;
        let relative_y = relative_grid_coordinate(fixed.translate_y, moving.translate_y)
            .ok_or_else(|| {
                GeneralFastError::InvalidInput(
                    "directional vertical translation difference is outside the canonical grid"
                        .to_owned(),
                )
            })?;
        Ok(IrregularPoint::new(
            from_grid(relative_x as f64),
            from_grid(relative_y as f64),
        ))
    }

    fn compress_directional_state(
        &mut self,
        state: &RelaxedState,
        target_depth_mm: f64,
    ) -> Result<Option<RelaxedState>, GeneralFastError> {
        let mut order = (0..state.placements.len()).collect::<Vec<_>>();
        order.sort_by(|first, second| {
            self.pieces[state.placements[*first].input_index]
                .id
                .cmp(self.pieces[state.placements[*second].input_index].id)
        });
        let mut replacements = Vec::new();
        for input_index in order {
            let placement = &state.placements[input_index];
            let Some(inner_fit) = self.directional_inner_fit(placement, target_depth_mm)? else {
                self.counters.directional_rejected_contractions = self
                    .counters
                    .directional_rejected_contractions
                    .saturating_add(1);
                return Ok(None);
            };
            let (x, y) = self.directional_position(placement)?;
            if inner_fit.contains(x, y) {
                continue;
            }
            let x = x.clamp(inner_fit.min_x, inner_fit.max_x);
            let y = y.clamp(inner_fit.min_y, inner_fit.max_y);
            replacements.push((input_index, x, y));
        }
        let mut compressed = state.clone();
        compressed.strip_depth_mm = target_depth_mm;
        for (input_index, x, y) in replacements.iter().copied() {
            compressed.placements[input_index].translate_x = from_grid(x as f64);
            compressed.placements[input_index].translate_y = from_grid(y as f64);
        }
        for placement in &compressed.placements {
            if !self.directional_contains(placement, target_depth_mm)? {
                self.counters.directional_containment_rejections = self
                    .counters
                    .directional_containment_rejections
                    .saturating_add(1);
                return Ok(None);
            }
        }
        self.counters.directional_relocations = self
            .counters
            .directional_relocations
            .saturating_add(replacements.len());
        Ok(Some(compressed))
    }

    fn seed_angle(&self, angle_deg: f64) -> f64 {
        match self.relaxed_settings.angle_seed_policy {
            GeneralRelaxedAngleSeedPolicy::CurrentOnly => continuous_angle(angle_deg),
            GeneralRelaxedAngleSeedPolicy::StructuredGrid => canonical_angle(angle_deg),
            GeneralRelaxedAngleSeedPolicy::ContinuousUniform => continuous_angle(angle_deg),
        }
    }

    fn dynamic_query_budget_exhausted(&self) -> bool {
        self.dynamic_query_limit
            .is_some_and(|limit| self.counters.dynamic_hazard_queries >= limit)
    }

    fn count_seed_evaluation(&mut self, current: &RelaxedPlacement, candidate: &RelaxedPlacement) {
        if angle_key(current.rotation_deg) != angle_key(candidate.rotation_deg)
            || current.mirrored != candidate.mirrored
        {
            self.counters.rotation_evaluations =
                self.counters.rotation_evaluations.saturating_add(1);
        } else {
            self.counters.translation_evaluations =
                self.counters.translation_evaluations.saturating_add(1);
        }
    }

    fn prepare_dynamic_hazard(&mut self, state: &RelaxedState) -> Result<(), GeneralFastError> {
        if !self.uses_dynamic_hazard() {
            return Ok(());
        }
        #[cfg(feature = "jagua-experimental")]
        {
            let poses = state.placements.iter().map(hazard_pose).collect::<Vec<_>>();
            self.counters.dynamic_layout_loads =
                self.counters.dynamic_layout_loads.saturating_add(1);
            if let Some(index) = self.hazard_index.as_mut() {
                index
                    .rebuild(state.strip_depth_mm, &poses)
                    .map_err(dynamic_hazard_error)?;
            } else {
                self.hazard_index = Some(if let Some(catalog) = &self.hazard_catalog {
                    JaguaHazardIndex::from_catalog(
                        self.pieces,
                        self.fast_settings,
                        state.strip_depth_mm,
                        &poses,
                        catalog,
                    )
                    .map_err(dynamic_hazard_error)?
                } else {
                    JaguaHazardIndex::new(
                        self.pieces,
                        self.fast_settings,
                        state.strip_depth_mm,
                        &poses,
                    )
                    .map_err(dynamic_hazard_error)?
                });
                self.counters.dynamic_index_builds =
                    self.counters.dynamic_index_builds.saturating_add(1);
            }
            return Ok(());
        }
        #[cfg(not(feature = "jagua-experimental"))]
        {
            let _ = state;
            Err(GeneralFastError::InvalidSettings(
                "dynamic hazard search requires the jagua-experimental feature".to_owned(),
            ))
        }
    }

    fn local_shape_bounds(
        &mut self,
        input_index: usize,
        rotation_deg: f64,
        mirrored: bool,
    ) -> Result<IrregularBounds, GeneralFastError> {
        if self.uses_dynamic_hazard() {
            #[cfg(feature = "jagua-experimental")]
            {
                return self
                    .hazard_index
                    .as_mut()
                    .expect("dynamic hazard index is prepared before search")
                    .pose_bounds(
                        input_index,
                        GeneralHazardPose {
                            rotation_deg: continuous_angle(rotation_deg),
                            mirrored,
                            translate_short_axis: 0.0,
                            translate_long_axis: 0.0,
                        },
                    )
                    .map_err(dynamic_hazard_error);
            }
            #[cfg(not(feature = "jagua-experimental"))]
            {
                return Err(GeneralFastError::InvalidSettings(
                    "dynamic hazard search requires the jagua-experimental feature".to_owned(),
                ));
            }
        }
        Ok(self.oriented(input_index, rotation_deg, mirrored)?.bounds)
    }

    fn commit_dynamic_hazard(
        &mut self,
        placement: &RelaxedPlacement,
    ) -> Result<(), GeneralFastError> {
        if !self.uses_dynamic_hazard() {
            return Ok(());
        }
        #[cfg(feature = "jagua-experimental")]
        {
            self.hazard_index
                .as_mut()
                .expect("dynamic hazard index is prepared before search")
                .commit(placement.input_index, hazard_pose(placement))
                .map_err(dynamic_hazard_error)?;
            self.counters.dynamic_hazard_updates =
                self.counters.dynamic_hazard_updates.saturating_add(1);
            return Ok(());
        }
        #[cfg(not(feature = "jagua-experimental"))]
        {
            let _ = placement;
            Err(GeneralFastError::InvalidSettings(
                "dynamic hazard search requires the jagua-experimental feature".to_owned(),
            ))
        }
    }

    fn run(&mut self, mut state: RelaxedState) -> Result<LaneOutcome, GeneralFastError> {
        self.prepare_dynamic_hazard(&state)?;
        if self.uses_directional_pressure() {
            self.preflight_directional_assignment(&state)?;
        }
        let mut score = self.score_state(&state)?;
        for sweep in 0..self.relaxed_settings.sweeps_per_epoch {
            if score.feasible() {
                break;
            }
            self.move_sweep(&mut state, &mut score, sweep)?;
            if ENABLE_EJECTION_CHAIN
                && !score.feasible()
                && sweep == self.relaxed_settings.sweeps_per_epoch / 2
            {
                self.try_ejection_chain(&mut state, &mut score)?;
            }
            if !score.feasible() && sweep + 1 < self.relaxed_settings.sweeps_per_epoch {
                update_weights(&mut self.weights, &score.collision_pairs);
                refresh_weighted_loss(&mut score, &self.weights);
            }
        }
        Ok(LaneOutcome {
            state,
            score,
            weights: self.weights.clone(),
            counters: self.counters,
            selected_lane: 0,
            restart_disruptions: 0,
        })
    }

    fn preflight_directional_assignment(
        &mut self,
        state: &RelaxedState,
    ) -> Result<(), GeneralFastError> {
        let mut keys = Vec::with_capacity(
            state
                .placements
                .len()
                .saturating_mul(state.placements.len().saturating_sub(1)),
        );
        for fixed_index in 0..state.placements.len() {
            for moving_index in 0..state.placements.len() {
                if fixed_index == moving_index {
                    continue;
                }
                keys.push(self.pair_nfp_key(
                    &state.placements[fixed_index],
                    &state.placements[moving_index],
                )?);
            }
        }
        if !self.preflight_directional_pair_nfps(&keys, false)? {
            return Err(directional_lane_unscorable_error(
                "fixed-orientation translation cache budget",
            ));
        }
        Ok(())
    }

    fn run_repair_arm(
        &mut self,
        mut state: RelaxedState,
        allow_rotation: bool,
        neighborhood_size: usize,
        query_budget: usize,
        confirmation_budget: usize,
    ) -> Result<LaneOutcome, GeneralFastError> {
        self.prepare_dynamic_hazard(&state)?;
        let initial_score = self.score_state_dynamic(&state)?;
        let active = repair_active_indices(
            &state,
            &initial_score,
            self.pieces,
            &self.weights,
            neighborhood_size,
        );
        let reinsertion_budget = query_budget / 2;
        for (ordinal, input_index) in active.iter().copied().enumerate() {
            if self.counters.dynamic_hazard_queries >= reinsertion_budget
                || self.counters.retained_f64_confirmations >= confirmation_budget
            {
                break;
            }
            let ignored = active[(ordinal + 1)..]
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            let pieces_left = active.len().saturating_sub(ordinal).max(1);
            let piece_budget = reinsertion_budget
                .saturating_sub(self.counters.dynamic_hazard_queries)
                / pieces_left;
            let angles = if allow_rotation {
                repair_angles(self.pieces[input_index], &state.placements[input_index])
            } else {
                vec![state.placements[input_index].rotation_deg]
            };
            let replacement = self.search_repair_piece(
                &state,
                input_index,
                &angles,
                &ignored,
                piece_budget,
                true,
            )?;
            self.commit_dynamic_hazard(&replacement)?;
            if move_tie_key(&replacement) != move_tie_key(&state.placements[input_index]) {
                self.counters.accepted_moves = self.counters.accepted_moves.saturating_add(1);
            }
            state.placements[input_index] = replacement;
        }

        self.relaxed_settings.angle_seed_policy = GeneralRelaxedAngleSeedPolicy::CurrentOnly;
        self.relaxed_settings.global_samples_per_move = 2;
        self.relaxed_settings.focused_samples_per_move = 2;
        self.relaxed_settings.refinement_rounds = 1;
        self.dynamic_query_limit = Some(query_budget);
        let mut score = self.score_state_dynamic(&state)?;
        for sweep in 0..4 {
            if score.feasible() || self.dynamic_query_budget_exhausted() {
                break;
            }
            self.move_sweep(&mut state, &mut score, sweep)?;
            if !score.feasible() && !self.dynamic_query_budget_exhausted() {
                update_weights(&mut self.weights, &score.collision_pairs);
                refresh_weighted_loss(&mut score, &self.weights);
            }
        }
        self.dynamic_query_limit = None;
        score = self.score_state_dynamic(&state)?;
        Ok(LaneOutcome {
            state,
            score,
            weights: self.weights.clone(),
            counters: self.counters,
            selected_lane: 0,
            restart_disruptions: 0,
        })
    }

    fn search_repair_piece(
        &mut self,
        state: &RelaxedState,
        input_index: usize,
        angles: &[f64],
        ignored: &BTreeSet<usize>,
        query_budget: usize,
        allow_worsening: bool,
    ) -> Result<RelaxedPlacement, GeneralFastError> {
        let query_limit = self
            .counters
            .dynamic_hazard_queries
            .saturating_add(query_budget);
        let current = state.placements[input_index].clone();
        let current_score =
            self.confirm_repair_candidate(state, input_index, &current, ignored, false)?;
        let mut best = current.clone();
        let mut best_score = self.score_repair_candidate(state, input_index, &best, ignored)?;
        let contact_limit = self
            .counters
            .dynamic_hazard_queries
            .saturating_add(query_budget.saturating_mul(3) / 4);
        let mut contact_sets = angles
            .iter()
            .copied()
            .map(|angle| {
                self.repair_contact_candidates(
                    state,
                    input_index,
                    continuous_angle(angle),
                    current.mirrored,
                    ignored,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut contact_index = 0usize;
        while self.counters.dynamic_hazard_queries < contact_limit
            && contact_sets.iter().any(|candidates| !candidates.is_empty())
        {
            let set_index = contact_index % contact_sets.len();
            if contact_sets[set_index].is_empty() {
                contact_index = contact_index.saturating_add(1);
                continue;
            }
            let candidate = contact_sets[set_index].remove(0);
            let score = self.score_repair_candidate(state, input_index, &candidate, ignored)?;
            if angle_key(candidate.rotation_deg) != angle_key(current.rotation_deg) {
                self.counters.rotation_evaluations =
                    self.counters.rotation_evaluations.saturating_add(1);
            } else {
                self.counters.translation_evaluations =
                    self.counters.translation_evaluations.saturating_add(1);
            }
            if compare_move_score(&score, &candidate, &best_score, &best) == Ordering::Less {
                best = candidate;
                best_score = score;
            }
            contact_index = contact_index.saturating_add(1);
        }

        let best_bounds = self.local_shape_bounds(input_index, best.rotation_deg, best.mirrored)?;
        let mut step_x = ((best_bounds.max_x - best_bounds.min_x) * 0.25).max(0.001);
        let mut step_y = ((best_bounds.max_y - best_bounds.min_y) * 0.25).max(0.001);
        let mut horizontal = true;
        while self.counters.dynamic_hazard_queries.saturating_add(2) <= query_limit
            && (step_x >= 0.001 || step_y >= 0.001)
        {
            let offsets = if horizontal {
                [(step_x, 0.0), (-step_x, 0.0)]
            } else {
                [(0.0, step_y), (0.0, -step_y)]
            };
            let mut improved = false;
            for (offset_x, offset_y) in offsets {
                let candidate = RelaxedPlacement {
                    translate_x: snap_mm(best.translate_x + offset_x),
                    translate_y: snap_mm(best.translate_y + offset_y),
                    ..best.clone()
                };
                let score = self.score_repair_candidate(state, input_index, &candidate, ignored)?;
                self.counters.translation_evaluations =
                    self.counters.translation_evaluations.saturating_add(1);
                if compare_move_score(&score, &candidate, &best_score, &best) == Ordering::Less {
                    best = candidate;
                    best_score = score;
                    improved = true;
                }
            }
            if !improved {
                if horizontal {
                    step_x *= 0.5;
                } else {
                    step_y *= 0.5;
                }
            }
            horizontal = !horizontal;
        }
        let confirmed = self.confirm_repair_candidate(state, input_index, &best, ignored, true)?;
        if !allow_worsening
            && compare_score_objective(&confirmed, &current_score) == Ordering::Greater
        {
            Ok(current)
        } else {
            Ok(best)
        }
    }

    fn repair_contact_candidates(
        &mut self,
        state: &RelaxedState,
        input_index: usize,
        angle: f64,
        mirrored: bool,
        ignored: &BTreeSet<usize>,
    ) -> Result<Vec<RelaxedPlacement>, GeneralFastError> {
        let local = self.local_shape_bounds(input_index, angle, mirrored)?;
        let inset = collision_sheet_inset_mm(self.fast_settings);
        let min_x = inset - local.min_x;
        let max_x = self.fast_settings.sheet_short_axis_mm - inset - local.max_x;
        let min_y = inset - local.min_y;
        let max_y = state.strip_depth_mm - inset - local.max_y;
        if min_x > max_x || min_y > max_y {
            return Ok(Vec::new());
        }
        let current = &state.placements[input_index];
        let mut positions = vec![
            (
                current.translate_x.clamp(min_x, max_x),
                current.translate_y.clamp(min_y, max_y),
            ),
            (min_x, min_y),
            (max_x, min_y),
        ];
        for (fixed_index, fixed) in state.placements.iter().enumerate() {
            if fixed_index == input_index || ignored.contains(&fixed_index) {
                continue;
            }
            let bounds = self.placement_bounds(fixed)?;
            let left = bounds.min_x - local.max_x;
            let right = bounds.max_x - local.min_x;
            let below = bounds.min_y - local.max_y;
            let above = bounds.max_y - local.min_y;
            let align_left = bounds.min_x - local.min_x;
            let align_right = bounds.max_x - local.max_x;
            let align_bottom = bounds.min_y - local.min_y;
            let align_top = bounds.max_y - local.max_y;
            positions.extend([
                (left, align_bottom),
                (left, align_top),
                (right, align_bottom),
                (right, align_top),
                (align_left, below),
                (align_right, below),
                (align_left, above),
                (align_right, above),
            ]);
        }
        let mut unique = BTreeMap::new();
        for (x, y) in positions {
            if x < min_x || x > max_x || y < min_y || y > max_y {
                continue;
            }
            let x = snap_mm(x);
            let y = snap_mm(y);
            unique
                .entry((grid_key(y), grid_key(x)))
                .or_insert(RelaxedPlacement {
                    input_index,
                    rotation_deg: angle,
                    mirrored,
                    translate_x: x,
                    translate_y: y,
                });
        }
        Ok(unique.into_values().collect())
    }

    fn score_repair_candidate(
        &mut self,
        state: &RelaxedState,
        input_index: usize,
        candidate: &RelaxedPlacement,
        ignored: &BTreeSet<usize>,
    ) -> Result<PlacementScore, GeneralFastError> {
        #[cfg(feature = "jagua-experimental")]
        {
            let (boundary_violations, boundary_loss) =
                self.boundary_penalty(candidate, state.strip_depth_mm)?;
            if self.dynamic_query_budget_exhausted() {
                return Ok(PlacementScore {
                    boundary_violations,
                    boundary_loss,
                    collision_pairs: Vec::new(),
                    weighted_loss: f64::INFINITY,
                });
            }
            let query = self
                .hazard_index
                .as_mut()
                .expect("repair prepares the dynamic hazard index")
                .query(input_index, hazard_pose(candidate), None)
                .map_err(dynamic_hazard_error)?;
            self.counters.dynamic_hazard_queries =
                self.counters.dynamic_hazard_queries.saturating_add(1);
            let GeneralHazardQuery::Complete {
                colliding_piece_ids,
                ..
            } = query
            else {
                return Err(GeneralFastError::InvalidInput(
                    "repair requires complete hazard rows".to_owned(),
                ));
            };
            let mut collision_pairs = Vec::new();
            let mut weighted_loss = boundary_loss;
            for fixed_index in colliding_piece_ids {
                if ignored.contains(&fixed_index) {
                    continue;
                }
                let penalty =
                    self.rollback_pair_pressure(candidate, &state.placements[fixed_index])?;
                let pair = ordered_pair(input_index, fixed_index);
                collision_pairs.push((pair.0, pair.1, penalty));
                weighted_loss += self.pair_weight(pair.0, pair.1) * penalty;
                self.counters.dynamic_pressure_evaluations =
                    self.counters.dynamic_pressure_evaluations.saturating_add(1);
            }
            collision_pairs.sort_by_key(|(first, second, _)| (*first, *second));
            Ok(PlacementScore {
                boundary_violations,
                boundary_loss,
                collision_pairs,
                weighted_loss,
            })
        }
        #[cfg(not(feature = "jagua-experimental"))]
        {
            let _ = (state, input_index, candidate, ignored);
            Err(GeneralFastError::InvalidSettings(
                "angular repair requires the jagua-experimental feature".to_owned(),
            ))
        }
    }

    fn confirm_repair_candidate(
        &mut self,
        state: &RelaxedState,
        input_index: usize,
        candidate: &RelaxedPlacement,
        ignored: &BTreeSet<usize>,
        retained: bool,
    ) -> Result<PlacementScore, GeneralFastError> {
        let (boundary_violations, boundary_loss) =
            self.boundary_penalty(candidate, state.strip_depth_mm)?;
        let mut collision_pairs = Vec::new();
        let mut weighted_loss = boundary_loss;
        for fixed_index in 0..state.placements.len() {
            if fixed_index == input_index || ignored.contains(&fixed_index) {
                continue;
            }
            let penalty =
                self.confirmed_pair_pressure(candidate, &state.placements[fixed_index])?;
            if penalty == 0.0 {
                continue;
            }
            let pair = ordered_pair(input_index, fixed_index);
            collision_pairs.push((pair.0, pair.1, penalty));
            weighted_loss += self.pair_weight(pair.0, pair.1) * penalty;
        }
        collision_pairs.sort_by_key(|(first, second, _)| (*first, *second));
        if retained {
            self.counters.retained_f64_confirmations =
                self.counters.retained_f64_confirmations.saturating_add(1);
        }
        Ok(PlacementScore {
            boundary_violations,
            boundary_loss,
            collision_pairs,
            weighted_loss,
        })
    }

    fn run_sweep(
        &mut self,
        mut state: RelaxedState,
        sweep: usize,
    ) -> Result<LaneOutcome, GeneralFastError> {
        self.prepare_dynamic_hazard(&state)?;
        let mut score = self.score_state(&state)?;
        self.move_sweep(&mut state, &mut score, sweep)?;
        if ENABLE_EJECTION_CHAIN
            && !score.feasible()
            && sweep == self.relaxed_settings.sweeps_per_epoch / 2
        {
            self.try_ejection_chain(&mut state, &mut score)?;
        }
        Ok(LaneOutcome {
            state,
            score,
            weights: self.weights.clone(),
            counters: self.counters,
            selected_lane: 0,
            restart_disruptions: 0,
        })
    }

    fn move_sweep(
        &mut self,
        state: &mut RelaxedState,
        score: &mut PairTracker,
        sweep: usize,
    ) -> Result<(), GeneralFastError> {
        if !score.feasible() {
            let mut forced = BTreeSet::new();
            let mut active = score
                .collision_pairs
                .iter()
                .flat_map(|(first, second, _)| [*first, *second])
                .collect::<BTreeSet<_>>();
            if !self.uses_directional_pressure() {
                for (index, placement) in state.placements.iter().enumerate() {
                    if self.boundary_penalty(placement, state.strip_depth_mm)?.0 > 0 {
                        active.insert(index);
                    }
                }
            }
            if active.is_empty() {
                forced.extend(legacy_forced_blockers(state, self.pieces, 4));
                active.extend(forced.iter().copied());
            }
            let mut order = active.into_iter().collect::<Vec<_>>();
            shuffle(&mut order, &mut self.rng);
            if sweep > 0 && sweep % 4 == 0 {
                for blocker in legacy_forced_blockers(&state, self.pieces, 2) {
                    if !order.contains(&blocker) {
                        order.push(blocker);
                    }
                    forced.insert(blocker);
                }
            }
            let mut piece_index = self.build_piece_index(state)?;
            for input_index in order {
                if self.dynamic_query_budget_exhausted() {
                    break;
                }
                if !forced.contains(&input_index)
                    && !self.piece_is_active(state, score, input_index)?
                {
                    continue;
                }
                let current = state.placements[input_index].clone();
                let old_boundary = self.boundary_penalty(&current, state.strip_depth_mm)?;
                let (mut replacement, replacement_score) =
                    self.search_piece(state, score, input_index, &piece_index)?;
                let mut replacement_score = if self.uses_dynamic_hazard() {
                    self.confirm_dynamic_replacement(
                        state,
                        input_index,
                        &replacement,
                        &replacement_score,
                    )?
                } else {
                    replacement_score
                };
                if self.uses_dynamic_hazard() {
                    let current_score = tracked_piece_score(score, input_index, &self.weights);
                    if compare_score_objective(&replacement_score, &current_score)
                        == Ordering::Greater
                    {
                        replacement = current.clone();
                        replacement_score = current_score;
                    }
                }
                if move_tie_key(&replacement) != move_tie_key(&current) {
                    self.counters.accepted_moves = self.counters.accepted_moves.saturating_add(1);
                    if self.uses_directional_pressure() {
                        for (_, _, penalty) in &replacement_score.collision_pairs {
                            self.counters
                                .directional_accepted_pair_loss
                                .observe(*penalty);
                        }
                        self.counters
                            .directional_accepted_boundary_loss
                            .observe(replacement_score.boundary_loss);
                    }
                }
                let old_bounds = self.placement_bounds(&current)?;
                let replacement_bounds = self.placement_bounds(&replacement)?;
                piece_index.remove(input_index, old_bounds);
                piece_index.insert(input_index, replacement_bounds);
                self.commit_dynamic_hazard(&replacement)?;
                state.placements[input_index] = replacement;
                update_score_after_move(
                    score,
                    input_index,
                    old_boundary,
                    replacement_score,
                    &self.weights,
                );
            }
        }
        Ok(())
    }

    fn try_ejection_chain(
        &mut self,
        state: &mut RelaxedState,
        score: &mut PairTracker,
    ) -> Result<(), GeneralFastError> {
        let Some((first, second, _)) = score
            .collision_pairs
            .iter()
            .max_by(|first, second| {
                let first_pressure = first.2 * self.pair_weight(first.0, first.1);
                let second_pressure = second.2 * self.pair_weight(second.0, second.1);
                first_pressure.total_cmp(&second_pressure).then_with(|| {
                    ordered_pair(second.0, second.1).cmp(&ordered_pair(first.0, first.1))
                })
            })
            .copied()
        else {
            return Ok(());
        };

        let mut candidates = Vec::new();
        for root in [first, second] {
            let donors = self.chain_donors(state, root)?;
            for donor in donors.iter().copied() {
                let root_orientations = self.chain_orientations(
                    root,
                    &state.placements[root],
                    &state.placements[donor],
                );
                let donor_orientations = self.chain_orientations(
                    donor,
                    &state.placements[donor],
                    &state.placements[root],
                );
                for (root_angle, root_mirror) in root_orientations.iter().copied() {
                    for (donor_angle, donor_mirror) in donor_orientations.iter().copied() {
                        let replacements = vec![
                            (
                                root,
                                self.placement_in_slot(
                                    root,
                                    root_angle,
                                    root_mirror,
                                    &state.placements[donor],
                                )?,
                            ),
                            (
                                donor,
                                self.placement_in_slot(
                                    donor,
                                    donor_angle,
                                    donor_mirror,
                                    &state.placements[root],
                                )?,
                            ),
                        ];
                        self.report_ejection_candidate(
                            state,
                            score,
                            replacements,
                            &mut candidates,
                        )?;
                    }
                }
            }

            for donor_pair in donors.windows(2) {
                let first_donor = donor_pair[0];
                let second_donor = donor_pair[1];
                let root_orientation = self.chain_orientations(
                    root,
                    &state.placements[root],
                    &state.placements[first_donor],
                )[0];
                let first_orientation = self.chain_orientations(
                    first_donor,
                    &state.placements[first_donor],
                    &state.placements[second_donor],
                )[0];
                let second_orientation = self.chain_orientations(
                    second_donor,
                    &state.placements[second_donor],
                    &state.placements[root],
                )[0];
                let replacements = vec![
                    (
                        root,
                        self.placement_in_slot(
                            root,
                            root_orientation.0,
                            root_orientation.1,
                            &state.placements[first_donor],
                        )?,
                    ),
                    (
                        first_donor,
                        self.placement_in_slot(
                            first_donor,
                            first_orientation.0,
                            first_orientation.1,
                            &state.placements[second_donor],
                        )?,
                    ),
                    (
                        second_donor,
                        self.placement_in_slot(
                            second_donor,
                            second_orientation.0,
                            second_orientation.1,
                            &state.placements[root],
                        )?,
                    ),
                ];
                self.report_ejection_candidate(state, score, replacements, &mut candidates)?;
            }
        }

        if candidates.is_empty() {
            return Ok(());
        }
        candidates.sort_by(compare_ejection_candidates);
        candidates.dedup_by(|first, second| {
            ejection_candidate_key(first) == ejection_candidate_key(second)
        });
        let improving = compare_chain_score(&candidates[0].score, score) == Ordering::Less;
        let selected = if improving {
            0
        } else if self.allow_worsening_chain {
            let eligible = candidates
                .iter()
                .take(EJECTION_CHAIN_DIVERSITY)
                .take_while(|candidate| {
                    candidate.score.weighted_loss <= score.weighted_loss * 2.0
                        && candidate.score.collision_pairs.len()
                            <= score.collision_pairs.len().saturating_add(3)
                })
                .count();
            if eligible == 0 {
                return Ok(());
            }
            (self.rng.next_u64() as usize) % eligible
        } else {
            return Ok(());
        };
        let selected = candidates.swap_remove(selected);
        for (index, placement) in selected.replacements {
            state.placements[index] = placement;
        }
        *score = selected.score;
        self.counters.ejection_chain_accepts += 1;
        Ok(())
    }

    fn report_ejection_candidate(
        &mut self,
        state: &RelaxedState,
        score: &PairTracker,
        replacements: Vec<(usize, RelaxedPlacement)>,
        candidates: &mut Vec<EjectionCandidate>,
    ) -> Result<(), GeneralFastError> {
        self.counters.ejection_chain_evaluations += 1;
        let candidate_score = self.score_after_replacements(state, score, &replacements)?;
        candidates.push(EjectionCandidate {
            replacements,
            score: candidate_score,
        });
        Ok(())
    }

    fn chain_donors(
        &mut self,
        state: &RelaxedState,
        root: usize,
    ) -> Result<Vec<usize>, GeneralFastError> {
        let root_placement = &state.placements[root];
        let root_bounds = self
            .oriented(
                root_placement.input_index,
                root_placement.rotation_deg,
                root_placement.mirrored,
            )?
            .bounds;
        let root_width = (root_bounds.max_x - root_bounds.min_x).max(0.001);
        let root_height = (root_bounds.max_y - root_bounds.min_y).max(0.001);
        let root_area = root_width * root_height;
        let root_aspect = (root_width / root_height).ln().abs();
        let mut ranked = Vec::new();
        for (index, placement) in state.placements.iter().enumerate() {
            if index == root
                || same_piece_geometry(
                    self.pieces[root_placement.input_index],
                    self.pieces[placement.input_index],
                )
            {
                continue;
            }
            let bounds = self
                .oriented(
                    placement.input_index,
                    placement.rotation_deg,
                    placement.mirrored,
                )?
                .bounds;
            let width = (bounds.max_x - bounds.min_x).max(0.001);
            let height = (bounds.max_y - bounds.min_y).max(0.001);
            let area = width * height;
            let aspect = (width / height).ln().abs();
            let fit = (area / root_area).ln().abs() + (aspect - root_aspect).abs() * 0.5;
            let frontier = placement.translate_y + bounds.max_y;
            ranked.push((index, fit, frontier, self.pieces[index].id));
        }
        ranked.sort_by(|first, second| {
            first
                .1
                .total_cmp(&second.1)
                .then_with(|| second.2.total_cmp(&first.2))
                .then_with(|| first.3.cmp(second.3))
        });
        Ok(ranked
            .into_iter()
            .take(EJECTION_CHAIN_MAX_DONORS)
            .map(|(index, _, _, _)| index)
            .collect())
    }

    fn chain_orientations(
        &self,
        input_index: usize,
        current: &RelaxedPlacement,
        slot: &RelaxedPlacement,
    ) -> Vec<(f64, bool)> {
        let piece = self.pieces[input_index];
        let mut orientations = BTreeSet::new();
        orientations.insert((
            angle_key(if piece.allow_rotation {
                current.rotation_deg
            } else {
                0.0
            }),
            piece.allow_mirror && current.mirrored,
        ));
        orientations.insert((
            angle_key(if piece.allow_rotation {
                slot.rotation_deg
            } else {
                0.0
            }),
            piece.allow_mirror && slot.mirrored,
        ));
        orientations
            .into_iter()
            .map(|(angle, mirrored)| (angle_from_key(angle), mirrored))
            .collect()
    }

    fn placement_in_slot(
        &mut self,
        input_index: usize,
        rotation_deg: f64,
        mirrored: bool,
        slot: &RelaxedPlacement,
    ) -> Result<RelaxedPlacement, GeneralFastError> {
        let slot_bounds =
            self.local_shape_bounds(slot.input_index, slot.rotation_deg, slot.mirrored)?;
        let target_x = slot.translate_x + (slot_bounds.min_x + slot_bounds.max_x) * 0.5;
        let target_y = slot.translate_y + (slot_bounds.min_y + slot_bounds.max_y) * 0.5;
        let shape_bounds = self.local_shape_bounds(input_index, rotation_deg, mirrored)?;
        Ok(RelaxedPlacement {
            input_index,
            rotation_deg,
            mirrored,
            translate_x: snap_mm(target_x - (shape_bounds.min_x + shape_bounds.max_x) * 0.5),
            translate_y: snap_mm(target_y - (shape_bounds.min_y + shape_bounds.max_y) * 0.5),
        })
    }

    fn score_after_replacements(
        &mut self,
        state: &RelaxedState,
        base: &PairTracker,
        replacements: &[(usize, RelaxedPlacement)],
    ) -> Result<PairTracker, GeneralFastError> {
        let moved = replacements
            .iter()
            .map(|(index, _)| *index)
            .collect::<BTreeSet<_>>();
        let replacement_map = replacements
            .iter()
            .map(|(index, placement)| (*index, placement))
            .collect::<BTreeMap<_, _>>();
        let mut result = base.clone();
        for index in moved.iter().copied() {
            let old_boundary =
                self.boundary_penalty(&state.placements[index], state.strip_depth_mm)?;
            result.boundary_violations = result.boundary_violations.saturating_sub(old_boundary.0);
            result.boundary_loss = (result.boundary_loss - old_boundary.1).max(0.0);
            result.weighted_loss = (result.weighted_loss - old_boundary.1).max(0.0);
        }
        let mut removed_pair_loss = 0.0;
        result.collision_pairs.retain(|(first, second, penalty)| {
            let remove = moved.contains(first) || moved.contains(second);
            if remove {
                removed_pair_loss += self.pair_weight(*first, *second) * *penalty;
            }
            !remove
        });
        result.weighted_loss = (result.weighted_loss - removed_pair_loss).max(0.0);

        for index in moved.iter().copied() {
            let placement = replacement_map[&index];
            let (violations, loss) = self.boundary_penalty(placement, state.strip_depth_mm)?;
            result.replace_boundary(
                index,
                BoundaryEntry {
                    violations,
                    raw_loss: loss,
                },
            );
            result.boundary_violations = result.boundary_violations.saturating_add(violations);
            result.boundary_loss += loss;
            result.weighted_loss += loss;
            for fixed in 0..state.placements.len() {
                if fixed == index || moved.contains(&fixed) {
                    continue;
                }
                let penalty = self.pair_penalty(placement, &state.placements[fixed])?;
                if penalty > 0.0 {
                    let pair = ordered_pair(index, fixed);
                    result.collision_pairs.push((pair.0, pair.1, penalty));
                    result.weighted_loss += self.pair_weight(pair.0, pair.1) * penalty;
                }
            }
        }
        let moved = moved.into_iter().collect::<Vec<_>>();
        for first_position in 0..moved.len() {
            for second_position in (first_position + 1)..moved.len() {
                let first = moved[first_position];
                let second = moved[second_position];
                let penalty =
                    self.pair_penalty(replacement_map[&first], replacement_map[&second])?;
                if penalty > 0.0 {
                    let pair = ordered_pair(first, second);
                    result.collision_pairs.push((pair.0, pair.1, penalty));
                    result.weighted_loss += self.pair_weight(pair.0, pair.1) * penalty;
                }
            }
        }
        result
            .collision_pairs
            .sort_by_key(|(first, second, _)| (*first, *second));
        for index in moved.iter().copied() {
            for fixed in 0..result.piece_count {
                if fixed == index {
                    continue;
                }
                let pair = ordered_pair(index, fixed);
                let raw_loss = result
                    .collision_pairs
                    .iter()
                    .find(|(first, second, _)| (*first, *second) == pair)
                    .map(|(_, _, penalty)| *penalty)
                    .unwrap_or(0.0);
                result.replace_pair(pair.0, pair.1, raw_loss, self.pair_weight(pair.0, pair.1));
            }
        }
        Ok(result)
    }

    fn search_piece(
        &mut self,
        state: &RelaxedState,
        tracker: &PairTracker,
        input_index: usize,
        piece_index: &PieceIndex,
    ) -> Result<(RelaxedPlacement, PlacementScore), GeneralFastError> {
        let current = state.placements[input_index].clone();
        let current_bounds =
            self.local_shape_bounds(input_index, current.rotation_deg, current.mirrored)?;
        let unique_position_threshold = ((current_bounds.max_x - current_bounds.min_x)
            .min(current_bounds.max_y - current_bounds.min_y)
            * UNIQUE_SAMPLE_POSITION_RATIO)
            .max(0.001);
        let current_score =
            self.score_placement(state, input_index, &current, piece_index, None)?;
        if self.uses_directional_pressure() {
            let evaluation_budget =
                AXIS_MINIMIZATION_PASSES.saturating_mul(AXIS_RETAINED_CANDIDATES);
            return Ok(self
                .minimize_candidate_axes(
                    state,
                    tracker,
                    input_index,
                    current.clone(),
                    current_score.clone(),
                    piece_index,
                    evaluation_budget,
                )?
                .map(|(placement, score, _)| (placement, score))
                .unwrap_or((current, current_score)));
        }
        let mut starts = vec![(current.clone(), current_score)];
        let focused_radius_x = (current_bounds.max_x - current_bounds.min_x) * 1.5;
        let focused_radius_y = (current_bounds.max_y - current_bounds.min_y) * 1.5;
        for _ in 0..self.relaxed_settings.focused_samples_per_move {
            let candidate = self.random_candidate(
                &current,
                input_index,
                state.strip_depth_mm,
                Some((focused_radius_x, focused_radius_y)),
            )?;
            let score = self.score_placement(
                state,
                input_index,
                &candidate,
                piece_index,
                sample_upper_bound(&starts),
            )?;
            self.count_seed_evaluation(&current, &candidate);
            report_diverse_sample(&mut starts, candidate, score, unique_position_threshold);
        }
        for _ in 0..self.relaxed_settings.global_samples_per_move {
            let candidate =
                self.random_candidate(&current, input_index, state.strip_depth_mm, None)?;
            let score = self.score_placement(
                state,
                input_index,
                &candidate,
                piece_index,
                sample_upper_bound(&starts),
            )?;
            self.count_seed_evaluation(&current, &candidate);
            report_diverse_sample(&mut starts, candidate, score, unique_position_threshold);
        }

        let refinement_budget = self
            .relaxed_settings
            .refinement_rounds
            .saturating_mul(10)
            .saturating_mul(starts.len());
        let pre_refinement_budget = refinement_budget.saturating_mul(3) / 4;
        let per_start_budget = even_floor(pre_refinement_budget / starts.len().max(1));
        let mut refined = Vec::with_capacity(starts.len());
        let mut refinement_evaluations = 0usize;
        for (start, start_score) in starts {
            let (candidate, score, evaluations) = self.refine_candidate(
                state,
                input_index,
                start,
                start_score,
                piece_index,
                unique_position_threshold / UNIQUE_SAMPLE_POSITION_RATIO,
                PRE_REFINEMENT_INITIAL_RATIO,
                PRE_REFINEMENT_LIMIT_RATIO,
                5.0,
                1.0,
                per_start_budget,
            )?;
            refinement_evaluations = refinement_evaluations.saturating_add(evaluations);
            refined.push((candidate, score));
        }
        refined.sort_by(|(first, first_score), (second, second_score)| {
            compare_move_score(first_score, first, second_score, second)
        });
        let (best, best_score) = refined
            .into_iter()
            .next()
            .expect("the current placement always provides a refinement start");
        let final_budget = even_floor(refinement_budget.saturating_sub(refinement_evaluations));
        if ENABLE_NFP_AXIS_MINIMIZER {
            if let Some((best, best_score, axis_evaluations)) = self.minimize_candidate_axes(
                state,
                tracker,
                input_index,
                best.clone(),
                best_score.clone(),
                piece_index,
                final_budget,
            )? {
                let remaining = even_floor(final_budget.saturating_sub(axis_evaluations));
                let (best, best_score, _) = self.refine_candidate(
                    state,
                    input_index,
                    best,
                    best_score,
                    piece_index,
                    unique_position_threshold / UNIQUE_SAMPLE_POSITION_RATIO,
                    FINAL_REFINEMENT_INITIAL_RATIO,
                    FINAL_REFINEMENT_LIMIT_RATIO,
                    0.5,
                    0.05,
                    remaining,
                )?;
                return Ok((best, best_score));
            }
        }
        let (best, best_score, _) = self.refine_candidate(
            state,
            input_index,
            best,
            best_score,
            piece_index,
            unique_position_threshold / UNIQUE_SAMPLE_POSITION_RATIO,
            FINAL_REFINEMENT_INITIAL_RATIO,
            FINAL_REFINEMENT_LIMIT_RATIO,
            0.5,
            0.05,
            final_budget,
        )?;
        Ok((best, best_score))
    }

    fn piece_is_active(
        &mut self,
        state: &RelaxedState,
        score: &PairTracker,
        input_index: usize,
    ) -> Result<bool, GeneralFastError> {
        if score
            .collision_pairs
            .iter()
            .any(|(first, second, _)| *first == input_index || *second == input_index)
        {
            return Ok(true);
        }
        Ok(self
            .boundary_penalty(&state.placements[input_index], state.strip_depth_mm)?
            .0
            > 0)
    }

    fn refine_candidate(
        &mut self,
        state: &RelaxedState,
        input_index: usize,
        mut best: RelaxedPlacement,
        mut best_score: PlacementScore,
        piece_index: &PieceIndex,
        minimum_dimension: f64,
        initial_step_ratio: f64,
        limit_step_ratio: f64,
        initial_rotation_step_deg: f64,
        rotation_step_limit_deg: f64,
        evaluation_budget: usize,
    ) -> Result<(RelaxedPlacement, PlacementScore, usize), GeneralFastError> {
        let mut step_x = minimum_dimension * initial_step_ratio;
        let mut step_y = minimum_dimension * initial_step_ratio;
        let step_limit = (minimum_dimension * limit_step_ratio).max(0.001);
        let can_refine_rotation = self.refine_rotation
            && self.uses_dynamic_pressure()
            && self.pieces[input_index].allow_rotation;
        let mut rotation_step_deg = initial_rotation_step_deg;
        let mut axis = self.random_coordinate_axis(
            step_x,
            step_y,
            step_limit,
            rotation_step_deg,
            rotation_step_limit_deg,
            can_refine_rotation,
        );
        let mut evaluations = 0usize;
        while evaluations + 2 <= evaluation_budget
            && (step_x >= step_limit
                || step_y >= step_limit
                || (can_refine_rotation && rotation_step_deg >= rotation_step_limit_deg))
        {
            let offsets = coordinate_offsets(axis, step_x, step_y, rotation_step_deg);
            let first_candidate = RelaxedPlacement {
                input_index,
                rotation_deg: continuous_angle(best.rotation_deg + offsets[0].2),
                mirrored: best.mirrored,
                translate_x: snap_mm(best.translate_x + offsets[0].0),
                translate_y: snap_mm(best.translate_y + offsets[0].1),
            };
            let first_score = self.score_placement(
                state,
                input_index,
                &first_candidate,
                piece_index,
                Some(best_score.weighted_loss),
            )?;
            let second_candidate = RelaxedPlacement {
                input_index,
                rotation_deg: continuous_angle(best.rotation_deg + offsets[1].2),
                mirrored: best.mirrored,
                translate_x: snap_mm(best.translate_x + offsets[1].0),
                translate_y: snap_mm(best.translate_y + offsets[1].1),
            };
            let second_score = self.score_placement(
                state,
                input_index,
                &second_candidate,
                piece_index,
                Some(best_score.weighted_loss),
            )?;
            evaluations += 2;
            if axis == CoordinateAxis::Rotation {
                self.counters.rotation_evaluations =
                    self.counters.rotation_evaluations.saturating_add(2);
            } else {
                self.counters.translation_evaluations =
                    self.counters.translation_evaluations.saturating_add(2);
            }
            let selected = match compare_score_objective(&first_score, &second_score) {
                Ordering::Less => 0,
                Ordering::Greater => 1,
                Ordering::Equal => (self.rng.next_u64() as usize) & 1,
            };
            let (candidate, score) = if selected == 0 {
                (first_candidate, first_score)
            } else {
                (second_candidate, second_score)
            };
            let comparison = compare_score_objective(&score, &best_score);
            if comparison != Ordering::Greater {
                best = candidate;
                best_score = score;
            }
            let multiplier = if comparison == Ordering::Less {
                REFINEMENT_SUCCESS_MULTIPLIER
            } else {
                REFINEMENT_FAILURE_MULTIPLIER
            };
            apply_coordinate_multiplier(
                axis,
                &mut step_x,
                &mut step_y,
                &mut rotation_step_deg,
                multiplier,
            );
            if comparison != Ordering::Less
                && (step_x >= step_limit
                    || step_y >= step_limit
                    || (can_refine_rotation && rotation_step_deg >= rotation_step_limit_deg))
            {
                axis = self.random_coordinate_axis(
                    step_x,
                    step_y,
                    step_limit,
                    rotation_step_deg,
                    rotation_step_limit_deg,
                    can_refine_rotation,
                );
            }
        }
        Ok((best, best_score, evaluations))
    }

    fn minimize_candidate_axes(
        &mut self,
        state: &RelaxedState,
        tracker: &PairTracker,
        input_index: usize,
        mut best: RelaxedPlacement,
        mut best_score: PlacementScore,
        piece_index: &PieceIndex,
        evaluation_budget: usize,
    ) -> Result<Option<(RelaxedPlacement, PlacementScore, usize)>, GeneralFastError> {
        if evaluation_budget < 2 {
            return Ok(None);
        }
        let mut evaluations = 0usize;
        for pass in 0..AXIS_MINIMIZATION_PASSES {
            let axis = if pass % 2 == 0 {
                CoordinateAxis::Horizontal
            } else {
                CoordinateAxis::Vertical
            };
            let remaining = evaluation_budget.saturating_sub(evaluations);
            if remaining == 0 {
                break;
            }
            let Some(axis_values) = self.axis_minima(
                state,
                tracker,
                input_index,
                &best,
                axis,
                AXIS_RETAINED_CANDIDATES.min(remaining),
            )?
            else {
                return Ok(None);
            };
            let mut improved = false;
            for value in axis_values {
                let mut candidate = best.clone();
                match axis {
                    CoordinateAxis::Horizontal => candidate.translate_x = value,
                    CoordinateAxis::Vertical => candidate.translate_y = value,
                    CoordinateAxis::ForwardDiagonal
                    | CoordinateAxis::BackwardDiagonal
                    | CoordinateAxis::Rotation => {
                        unreachable!("axis minimization only uses cardinal axes")
                    }
                }
                if move_tie_key(&candidate) == move_tie_key(&best) {
                    continue;
                }
                let score = self.score_placement(
                    state,
                    input_index,
                    &candidate,
                    piece_index,
                    Some(best_score.weighted_loss),
                )?;
                evaluations = evaluations.saturating_add(1);
                self.counters.axis_candidate_evaluations =
                    self.counters.axis_candidate_evaluations.saturating_add(1);
                self.counters.translation_evaluations =
                    self.counters.translation_evaluations.saturating_add(1);
                if compare_score_objective(&score, &best_score) == Ordering::Less {
                    best = candidate;
                    best_score = score;
                    improved = true;
                }
                if evaluations >= evaluation_budget {
                    break;
                }
            }
            if !improved && pass >= 1 {
                break;
            }
        }
        Ok(Some((best, best_score, evaluations)))
    }

    fn axis_minima(
        &mut self,
        state: &RelaxedState,
        tracker: &PairTracker,
        input_index: usize,
        moving: &RelaxedPlacement,
        axis: CoordinateAxis,
        retained: usize,
    ) -> Result<Option<Vec<f64>>, GeneralFastError> {
        let moving_bounds =
            self.local_shape_bounds(moving.input_index, moving.rotation_deg, moving.mirrored)?;
        let moving_diameter = ((moving_bounds.max_x - moving_bounds.min_x).powi(2)
            + (moving_bounds.max_y - moving_bounds.min_y).powi(2))
        .sqrt();
        let inset = collision_sheet_inset_mm(self.fast_settings);
        let (minimum, maximum, current, direction) = match axis {
            CoordinateAxis::Horizontal => (
                inset - moving_bounds.min_x,
                self.fast_settings.sheet_short_axis_mm - inset - moving_bounds.max_x,
                moving.translate_x,
                (1.0, 0.0),
            ),
            CoordinateAxis::Vertical => (
                inset - moving_bounds.min_y,
                state.strip_depth_mm - inset - moving_bounds.max_y,
                moving.translate_y,
                (0.0, 1.0),
            ),
            CoordinateAxis::ForwardDiagonal
            | CoordinateAxis::BackwardDiagonal
            | CoordinateAxis::Rotation => {
                unreachable!("axis minimization only uses cardinal axes")
            }
        };
        if minimum > maximum {
            return Ok(Some(Vec::new()));
        }

        let mut pair_functions = Vec::with_capacity(state.placements.len().saturating_sub(1));
        let mut events = grid_neighbors_clamped(minimum, minimum, maximum);
        events.extend(grid_neighbors_clamped(maximum, minimum, maximum));
        events.extend(grid_neighbors_clamped(current, minimum, maximum));
        let mut logical_components = 0usize;
        for (fixed_index, fixed) in state.placements.iter().enumerate() {
            if fixed_index == input_index {
                continue;
            }
            let fixed_bounds =
                self.local_shape_bounds(fixed.input_index, fixed.rotation_deg, fixed.mirrored)?;
            let fixed_diameter = ((fixed_bounds.max_x - fixed_bounds.min_x).powi(2)
                + (fixed_bounds.max_y - fixed_bounds.min_y).powi(2))
            .sqrt();
            let relative = IrregularPoint::new(
                moving.translate_x - fixed.translate_x,
                moving.translate_y - fixed.translate_y,
            );
            let nfp_key = self.pair_nfp_key(fixed, moving)?;
            let Some(pair_nfp) = self.resolve_pair_nfp(fixed, moving)? else {
                return Ok(None);
            };
            logical_components = logical_components.saturating_add(pair_nfp.components.len());
            if logical_components > MAX_NFP_COMPONENTS_PER_MOVE {
                return Ok(None);
            }
            let mut intervals = Vec::new();
            for component in &pair_nfp.components {
                for point in &component.points {
                    let coordinate = if direction.0 == 1.0 {
                        fixed.translate_x + point.x
                    } else {
                        fixed.translate_y + point.y
                    };
                    if coordinate >= minimum && coordinate <= maximum {
                        events.extend(grid_neighbors_clamped(coordinate, minimum, maximum));
                    }
                }
                let orthogonal_outside = if direction.0 == 1.0 {
                    relative.y < component.bounds.min_y || relative.y > component.bounds.max_y
                } else {
                    relative.x < component.bounds.min_x || relative.x > component.bounds.max_x
                };
                if orthogonal_outside {
                    continue;
                }
                if let Some((start, end)) =
                    convex_line_interval(&component.points, relative, direction)
                {
                    let start = (current + start).max(minimum);
                    let end = (current + end).min(maximum);
                    if start <= end {
                        intervals.push((start, end));
                    }
                }
            }
            if events.len() > MAX_AXIS_EVENTS_PER_MOVE {
                return Ok(None);
            }
            merge_intervals(&mut intervals);
            if intervals.is_empty() {
                continue;
            }
            for (start, end) in intervals.iter().copied() {
                events.extend(grid_neighbors_clamped(start, minimum, maximum));
                events.extend(grid_neighbors_clamped(end, minimum, maximum));
                events.push(grid_predecessor_clamped(start, minimum, maximum));
                events.push(grid_successor_clamped(end, minimum, maximum));
                events.extend(grid_neighbors_clamped(
                    start + (end - start) * 0.5,
                    minimum,
                    maximum,
                ));
                if events.len() > MAX_AXIS_EVENTS_PER_MOVE {
                    return Ok(None);
                }
            }
            pair_functions.push(PairAxisIntervals {
                nfp_key,
                fixed_translate_x: fixed.translate_x,
                fixed_translate_y: fixed.translate_y,
                guided_weight: tracker.pair(input_index, fixed_index).guided_weight,
                normalization_scale: if self.uses_directional_pressure() {
                    1.0
                } else {
                    moving_diameter.max(fixed_diameter).max(0.001)
                },
                intervals,
            });
        }
        events.sort_by(f64::total_cmp);
        events.dedup_by(|first, second| grid_key(*first) == grid_key(*second));
        if events.len() > MAX_AXIS_EVENTS_PER_MOVE {
            return Ok(None);
        }
        self.counters.axis_events = self.counters.axis_events.saturating_add(events.len());
        let scored = events
            .iter()
            .copied()
            .map(|value| {
                let loss = pair_functions
                    .iter()
                    .map(|pair| {
                        pair.guided_weight * interval_penetration(value, &pair.intervals)
                            / pair.normalization_scale
                    })
                    .sum::<f64>();
                (value, loss)
            })
            .collect::<Vec<_>>();
        let mut minima = scored
            .iter()
            .enumerate()
            .filter(|(index, (_, loss))| {
                (*index == 0 || *loss <= scored[*index - 1].1)
                    && (*index + 1 == scored.len() || *loss <= scored[*index + 1].1)
            })
            .map(|(_, value)| *value)
            .collect::<Vec<_>>();
        let prefer_distance = self.uses_directional_pressure();
        minima.sort_by(|first, second| {
            compare_axis_candidate(first, second, current, prefer_distance)
        });
        minima.truncate(retained.saturating_mul(4).max(retained));
        for candidate in &mut minima {
            candidate.1 = pair_functions
                .iter()
                .map(|pair| {
                    let relative = match axis {
                        CoordinateAxis::Horizontal => IrregularPoint::new(
                            candidate.0 - pair.fixed_translate_x,
                            moving.translate_y - pair.fixed_translate_y,
                        ),
                        CoordinateAxis::Vertical => IrregularPoint::new(
                            moving.translate_x - pair.fixed_translate_x,
                            candidate.0 - pair.fixed_translate_y,
                        ),
                        CoordinateAxis::ForwardDiagonal
                        | CoordinateAxis::BackwardDiagonal
                        | CoordinateAxis::Rotation => {
                            unreachable!("axis minimization only uses cardinal axes")
                        }
                    };
                    pair.guided_weight * self.pair_directional_penetration(pair.nfp_key, relative)
                        / pair.normalization_scale
                })
                .sum::<f64>();
        }
        minima.sort_by(|first, second| {
            compare_axis_candidate(first, second, current, prefer_distance)
        });
        minima.truncate(retained);
        Ok(Some(minima.into_iter().map(|(value, _)| value).collect()))
    }

    fn pair_directional_penetration(&self, nfp_key: PairNfpKey, relative: IrregularPoint) -> f64 {
        let pair_nfp = self
            .pair_nfp_cache
            .get(&nfp_key)
            .expect("axis pair NFP is cached before scoring");
        let mut horizontal = Vec::new();
        let mut vertical = Vec::new();
        for component in &pair_nfp.components {
            if relative.y >= component.bounds.min_y && relative.y <= component.bounds.max_y {
                if let Some(interval) =
                    convex_line_interval(&component.points, relative, (1.0, 0.0))
                {
                    horizontal.push(interval);
                }
            }
            if relative.x >= component.bounds.min_x && relative.x <= component.bounds.max_x {
                if let Some(interval) =
                    convex_line_interval(&component.points, relative, (0.0, 1.0))
                {
                    vertical.push(interval);
                }
            }
        }
        merge_intervals(&mut horizontal);
        let horizontal = interval_penetration(0.0, &horizontal);
        if horizontal == 0.0 {
            return 0.0;
        }
        merge_intervals(&mut vertical);
        horizontal.min(interval_penetration(0.0, &vertical))
    }

    fn grid_directional_pair_penetration(
        &mut self,
        nfp_key: PairNfpKey,
        relative: IrregularPoint,
    ) -> GridDirectionalPenetration {
        let pair_nfp = self
            .pair_nfp_cache
            .get(&nfp_key)
            .expect("directional pair NFP is cached after preflight");
        let relative_x = grid_key(relative.x);
        let relative_y = grid_key(relative.y);
        let mut horizontal = Vec::new();
        let mut vertical = Vec::new();
        for component in &pair_nfp.components {
            if relative_y >= grid_lower_bound_key(component.bounds.min_y)
                && relative_y <= grid_upper_bound_key(component.bounds.max_y)
            {
                if let Some(interval) =
                    convex_line_interval(&component.points, relative, (1.0, 0.0))
                {
                    horizontal.push(grid_interval_bounds(interval));
                }
            }
            if relative_x >= grid_lower_bound_key(component.bounds.min_x)
                && relative_x <= grid_upper_bound_key(component.bounds.max_x)
            {
                if let Some(interval) =
                    convex_line_interval(&component.points, relative, (0.0, 1.0))
                {
                    vertical.push(grid_interval_bounds(interval));
                }
            }
        }
        let horizontal_intervals = horizontal.len();
        let vertical_intervals = vertical.len();
        let produced = horizontal_intervals.saturating_add(vertical_intervals);
        merge_grid_intervals(&mut horizontal);
        merge_grid_intervals(&mut vertical);
        let merged = horizontal.len().saturating_add(vertical.len());
        let horizontal = grid_interval_penetration(0, &horizontal);
        let vertical = grid_interval_penetration(0, &vertical);
        self.counters.directional_pair_evaluations =
            self.counters.directional_pair_evaluations.saturating_add(1);
        self.counters.directional_component_visits = self
            .counters
            .directional_component_visits
            .saturating_add(pair_nfp.components.len());
        self.counters.directional_intervals_produced = self
            .counters
            .directional_intervals_produced
            .saturating_add(produced);
        self.counters.directional_intervals_merged = self
            .counters
            .directional_intervals_merged
            .saturating_add(merged);
        if horizontal.min(vertical) == 0 {
            self.counters.directional_zero_penetration_inconsistencies = self
                .counters
                .directional_zero_penetration_inconsistencies
                .saturating_add(1);
        }
        GridDirectionalPenetration {
            horizontal_grid: horizontal,
            vertical_grid: vertical,
            horizontal_intervals,
            vertical_intervals,
        }
    }

    fn random_coordinate_axis(
        &mut self,
        step_x: f64,
        step_y: f64,
        step_limit: f64,
        rotation_step_deg: f64,
        rotation_step_limit_deg: f64,
        can_refine_rotation: bool,
    ) -> CoordinateAxis {
        let mut axes = Vec::with_capacity(6);
        if step_x >= step_limit {
            axes.push(CoordinateAxis::Horizontal);
        }
        if step_y >= step_limit {
            axes.push(CoordinateAxis::Vertical);
        }
        if step_x >= step_limit || step_y >= step_limit {
            axes.push(CoordinateAxis::ForwardDiagonal);
            axes.push(CoordinateAxis::BackwardDiagonal);
        }
        if can_refine_rotation && rotation_step_deg >= rotation_step_limit_deg {
            axes.push(CoordinateAxis::Rotation);
            axes.push(CoordinateAxis::Rotation);
        }
        axes[(self.rng.next_u64() as usize) % axes.len()]
    }

    fn random_candidate(
        &mut self,
        current: &RelaxedPlacement,
        input_index: usize,
        strip_depth_mm: f64,
        focused: Option<(f64, f64)>,
    ) -> Result<RelaxedPlacement, GeneralFastError> {
        if self.uses_directional_pressure() {
            return self.random_directional_candidate(current, strip_depth_mm, focused);
        }
        let piece = self.pieces[input_index];
        let rotation_deg = if self.relaxed_settings.angle_seed_policy
            == GeneralRelaxedAngleSeedPolicy::CurrentOnly
        {
            current.rotation_deg
        } else if piece.allow_rotation {
            if focused.is_some() {
                let sampled = current.rotation_deg + self.rng.range(-15.0, 15.0);
                self.seed_angle(sampled)
            } else {
                let sampled = self.rng.range(0.0, 360.0);
                self.seed_angle(sampled)
            }
        } else {
            current.rotation_deg
        };
        let mirrored = if piece.allow_mirror && focused.is_none() {
            self.rng.next_u64() & 1 == 1
        } else {
            current.mirrored
        };
        let bounds = self.local_shape_bounds(input_index, rotation_deg, mirrored)?;
        let inset = collision_sheet_inset_mm(self.fast_settings);
        let min_x = inset - bounds.min_x;
        let max_x = self.fast_settings.sheet_short_axis_mm - inset - bounds.max_x;
        let min_y = inset - bounds.min_y;
        let max_y = strip_depth_mm - inset - bounds.max_y;
        let (translate_x, translate_y) = if let Some((radius_x, radius_y)) = focused {
            (
                clamp_or_center(
                    current.translate_x + self.rng.range(-radius_x, radius_x),
                    min_x,
                    max_x,
                ),
                clamp_or_center(
                    current.translate_y + self.rng.range(-radius_y, radius_y),
                    min_y,
                    max_y,
                ),
            )
        } else {
            (
                sample_or_center(&mut self.rng, min_x, max_x),
                sample_or_center(&mut self.rng, min_y, max_y),
            )
        };
        Ok(RelaxedPlacement {
            input_index,
            rotation_deg,
            mirrored,
            translate_x: snap_mm(translate_x),
            translate_y: snap_mm(translate_y),
        })
    }

    fn random_directional_candidate(
        &mut self,
        current: &RelaxedPlacement,
        strip_depth_mm: f64,
        focused: Option<(f64, f64)>,
    ) -> Result<RelaxedPlacement, GeneralFastError> {
        let Some(mut inner_fit) = self.directional_inner_fit(current, strip_depth_mm)? else {
            return Err(directional_lane_unscorable_error(
                "fixed orientation has an empty inner-fit rectangle",
            ));
        };
        if let Some((radius_x, radius_y)) = focused {
            let (current_x, current_y) = self.directional_position(current)?;
            let radius_x = grid_coordinate(radius_x.abs()).ok_or_else(|| {
                GeneralFastError::InvalidInput(
                    "directional focus radius x is outside the canonical grid".to_owned(),
                )
            })?;
            let radius_y = grid_coordinate(radius_y.abs()).ok_or_else(|| {
                GeneralFastError::InvalidInput(
                    "directional focus radius y is outside the canonical grid".to_owned(),
                )
            })?;
            inner_fit.min_x = inner_fit.min_x.max(current_x.saturating_sub(radius_x));
            inner_fit.max_x = inner_fit.max_x.min(current_x.saturating_add(radius_x));
            inner_fit.min_y = inner_fit.min_y.max(current_y.saturating_sub(radius_y));
            inner_fit.max_y = inner_fit.max_y.min(current_y.saturating_add(radius_y));
        }
        let x = sample_grid_coordinate_with_rng(&mut self.rng, inner_fit.min_x, inner_fit.max_x)?;
        let y = sample_grid_coordinate_with_rng(&mut self.rng, inner_fit.min_y, inner_fit.max_y)?;
        Ok(RelaxedPlacement {
            input_index: current.input_index,
            rotation_deg: current.rotation_deg,
            mirrored: current.mirrored,
            translate_x: from_grid(x as f64),
            translate_y: from_grid(y as f64),
        })
    }

    fn score_state(&mut self, state: &RelaxedState) -> Result<PairTracker, GeneralFastError> {
        if self.uses_directional_pressure() {
            return self.score_state_directional(state);
        }
        if self.uses_dynamic_hazard() {
            return self.score_state_dynamic(state);
        }
        let piece_count = state.placements.len();
        let mut collision_pairs = Vec::new();
        let mut boundaries = Vec::with_capacity(piece_count);
        let mut pairs = vec![
            PairEntry {
                raw_loss: 0.0,
                guided_weight: 1.0,
                normalization_scale: 1.0,
            };
            piece_count.saturating_mul(piece_count.saturating_sub(1)) / 2
        ];
        for first in 0..piece_count {
            for second in (first + 1)..piece_count {
                pairs[pair_slot(piece_count, first, second)].guided_weight =
                    self.pair_weight(first, second);
            }
        }
        let mut incident_raw_loss = vec![0.0; piece_count];
        let mut boundary_violations = 0usize;
        let mut boundary_loss = 0.0;
        let mut weighted_loss = 0.0;
        for (index, placement) in state.placements.iter().enumerate() {
            let (violations, loss) = self.boundary_penalty(placement, state.strip_depth_mm)?;
            boundaries.push(BoundaryEntry {
                violations,
                raw_loss: loss,
            });
            boundary_violations = boundary_violations.saturating_add(violations);
            boundary_loss += loss;
            weighted_loss += loss;
            for second in (index + 1)..state.placements.len() {
                let penalty = self.pair_penalty(placement, &state.placements[second])?;
                let guided_weight = self.pair_weight(index, second);
                pairs[pair_slot(piece_count, index, second)] = PairEntry {
                    raw_loss: penalty,
                    guided_weight,
                    normalization_scale: 1.0,
                };
                if penalty > 0.0 {
                    incident_raw_loss[index] += penalty;
                    incident_raw_loss[second] += penalty;
                    collision_pairs.push((index, second, penalty));
                    weighted_loss += guided_weight * penalty;
                }
            }
        }
        Ok(PairTracker {
            piece_count,
            boundaries,
            pairs,
            incident_raw_loss,
            boundary_violations,
            boundary_loss,
            collision_pairs,
            weighted_loss,
        })
    }

    fn score_state_directional(
        &mut self,
        state: &RelaxedState,
    ) -> Result<PairTracker, GeneralFastError> {
        let piece_count = state.placements.len();
        let mut boundaries = Vec::with_capacity(piece_count);
        let boundary_violations = 0usize;
        let boundary_loss = 0.0;
        let mut colliding = Vec::new();
        for (index, placement) in state.placements.iter().enumerate() {
            if !self.directional_contains(placement, state.strip_depth_mm)? {
                self.counters.directional_containment_rejections = self
                    .counters
                    .directional_containment_rejections
                    .saturating_add(1);
                return Err(directional_lane_unscorable_error(
                    "initial state violates the canonical inner-fit rectangle",
                ));
            }
            boundaries.push(BoundaryEntry {
                violations: 0,
                raw_loss: 0.0,
            });
            self.counters.directional_initial_boundary_loss.observe(0.0);
            for second in (index + 1)..piece_count {
                if !self.pair_collides(placement, &state.placements[second])? {
                    continue;
                }
                let key = self.pair_nfp_key(placement, &state.placements[second])?;
                let relative =
                    self.directional_relative_point(placement, &state.placements[second])?;
                colliding.push((index, second, key, relative));
            }
        }
        let keys = colliding
            .iter()
            .map(|(_, _, key, _)| *key)
            .collect::<Vec<_>>();
        if !self.preflight_directional_pair_nfps(&keys, false)? {
            return Err(directional_lane_unscorable_error("cache budget"));
        }
        let mut pairs = vec![
            PairEntry {
                raw_loss: 0.0,
                guided_weight: 1.0,
                normalization_scale: 1.0,
            };
            piece_count.saturating_mul(piece_count.saturating_sub(1)) / 2
        ];
        for first in 0..piece_count {
            for second in (first + 1)..piece_count {
                pairs[pair_slot(piece_count, first, second)].guided_weight =
                    self.pair_weight(first, second);
            }
        }
        let mut incident_raw_loss = vec![0.0; piece_count];
        let mut collision_pairs = Vec::with_capacity(colliding.len());
        let mut weighted_loss = boundary_loss;
        for (first, second, key, relative) in colliding {
            let penetration = self.grid_directional_pair_penetration(key, relative);
            let Some(penalty) = penetration.penetration_mm() else {
                return Err(directional_lane_unscorable_error(&format!(
                    "SAT-positive pair {} / {} has zero grid penetration at ({}, {}) for key {:?}: horizontal={} across {} intervals, vertical={} across {} intervals",
                    self.pieces[first].id,
                    self.pieces[second].id,
                    relative.x,
                    relative.y,
                    key,
                    penetration.horizontal_grid,
                    penetration.horizontal_intervals,
                    penetration.vertical_grid,
                    penetration.vertical_intervals,
                )));
            };
            let guided_weight = self.pair_weight(first, second);
            pairs[pair_slot(piece_count, first, second)] = PairEntry {
                raw_loss: penalty,
                guided_weight,
                normalization_scale: 1.0,
            };
            incident_raw_loss[first] += penalty;
            incident_raw_loss[second] += penalty;
            collision_pairs.push((first, second, penalty));
            weighted_loss += guided_weight * penalty;
            self.counters.directional_initial_pair_loss.observe(penalty);
        }
        Ok(PairTracker {
            piece_count,
            boundaries,
            pairs,
            incident_raw_loss,
            boundary_violations,
            boundary_loss,
            collision_pairs,
            weighted_loss,
        })
    }

    fn score_placement(
        &mut self,
        state: &RelaxedState,
        input_index: usize,
        candidate: &RelaxedPlacement,
        piece_index: &PieceIndex,
        upper_bound: Option<f64>,
    ) -> Result<PlacementScore, GeneralFastError> {
        if self.uses_directional_pressure() {
            return self.score_placement_directional(
                state,
                input_index,
                candidate,
                piece_index,
                upper_bound,
            );
        }
        if self.uses_dynamic_hazard() {
            return self.score_placement_dynamic(state, input_index, candidate, upper_bound);
        }
        self.counters.surrogate_evaluations += 1;
        let (boundary_violations, boundary_loss) =
            self.boundary_penalty(candidate, state.strip_depth_mm)?;
        let mut weighted_loss = boundary_loss;
        let mut collision_pairs = Vec::new();
        let shape_bounds = self
            .oriented(
                candidate.input_index,
                candidate.rotation_deg,
                candidate.mirrored,
            )?
            .bounds;
        let candidate_bounds =
            translated_bounds(shape_bounds, candidate.translate_x, candidate.translate_y);
        piece_index.query_into(candidate_bounds, &mut self.piece_query_scratch);
        let fixed_indices = std::mem::take(&mut self.piece_query_scratch.selected);
        for fixed_index in fixed_indices.iter().copied() {
            if fixed_index == input_index {
                continue;
            }
            let penalty = self.pair_penalty(candidate, &state.placements[fixed_index])?;
            if penalty > 0.0 {
                let pair = ordered_pair(input_index, fixed_index);
                collision_pairs.push((pair.0, pair.1, penalty));
                weighted_loss += self.pair_weight(pair.0, pair.1) * penalty;
                if upper_bound.is_some_and(|upper_bound| weighted_loss > upper_bound) {
                    break;
                }
            }
        }
        self.piece_query_scratch.selected = fixed_indices;
        collision_pairs.sort_by_key(|(first, second, _)| (*first, *second));
        Ok(PlacementScore {
            boundary_violations,
            boundary_loss,
            collision_pairs,
            weighted_loss,
        })
    }

    fn score_placement_directional(
        &mut self,
        state: &RelaxedState,
        input_index: usize,
        candidate: &RelaxedPlacement,
        _piece_index: &PieceIndex,
        _upper_bound: Option<f64>,
    ) -> Result<PlacementScore, GeneralFastError> {
        self.counters.surrogate_evaluations = self.counters.surrogate_evaluations.saturating_add(1);
        if !self.directional_contains(candidate, state.strip_depth_mm)? {
            self.counters.directional_containment_rejections = self
                .counters
                .directional_containment_rejections
                .saturating_add(1);
            return Ok(PlacementScore {
                boundary_violations: 1,
                boundary_loss: 0.0,
                collision_pairs: Vec::new(),
                weighted_loss: f64::INFINITY,
            });
        }
        let boundary_violations = 0;
        let boundary_loss = 0.0;
        let mut colliding = Vec::new();
        for fixed_index in 0..state.placements.len() {
            if fixed_index == input_index
                || !self.pair_collides(candidate, &state.placements[fixed_index])?
            {
                continue;
            }
            let fixed = &state.placements[fixed_index];
            let (canonical_first, canonical_second) = if input_index < fixed_index {
                (candidate, fixed)
            } else {
                (fixed, candidate)
            };
            let key = self.pair_nfp_key(canonical_first, canonical_second)?;
            let relative = self.directional_relative_point(canonical_first, canonical_second)?;
            colliding.push((fixed_index, key, relative));
        }
        colliding.sort_by_key(|(fixed, _, _)| *fixed);
        let keys = colliding.iter().map(|(_, key, _)| *key).collect::<Vec<_>>();
        if !self.preflight_directional_pair_nfps(&keys, true)? {
            return Ok(unscorable_directional_score(
                input_index,
                boundary_violations,
                boundary_loss,
                &colliding,
            ));
        }
        let mut weighted_loss = boundary_loss;
        let mut collision_pairs = Vec::with_capacity(colliding.len());
        for (fixed_index, key, relative) in colliding.iter().copied() {
            let Some(penalty) = self
                .grid_directional_pair_penetration(key, relative)
                .penetration_mm()
            else {
                return Ok(unscorable_directional_score(
                    input_index,
                    boundary_violations,
                    boundary_loss,
                    &colliding,
                ));
            };
            let pair = ordered_pair(input_index, fixed_index);
            collision_pairs.push((pair.0, pair.1, penalty));
            weighted_loss += self.pair_weight(pair.0, pair.1) * penalty;
        }
        collision_pairs.sort_by_key(|(first, second, _)| (*first, *second));
        Ok(PlacementScore {
            boundary_violations,
            boundary_loss,
            collision_pairs,
            weighted_loss,
        })
    }

    fn score_state_dynamic(
        &mut self,
        state: &RelaxedState,
    ) -> Result<PairTracker, GeneralFastError> {
        #[cfg(feature = "jagua-experimental")]
        {
            let piece_count = state.placements.len();
            let mut collision_pairs = Vec::new();
            let mut boundaries = Vec::with_capacity(piece_count);
            let mut pairs = vec![
                PairEntry {
                    raw_loss: 0.0,
                    guided_weight: 1.0,
                    normalization_scale: 1.0,
                };
                piece_count.saturating_mul(piece_count.saturating_sub(1)) / 2
            ];
            let mut incident_raw_loss = vec![0.0; piece_count];
            let mut boundary_violations = 0usize;
            let mut boundary_loss = 0.0;
            let mut weighted_loss = 0.0;
            for (index, placement) in state.placements.iter().enumerate() {
                let (violations, loss) = self.boundary_penalty(placement, state.strip_depth_mm)?;
                boundaries.push(BoundaryEntry {
                    violations,
                    raw_loss: loss,
                });
                boundary_violations = boundary_violations.saturating_add(violations);
                boundary_loss += loss;
                weighted_loss += loss;
                for second in (index + 1)..piece_count {
                    let penalty =
                        self.confirmed_pair_pressure(placement, &state.placements[second])?;
                    if penalty == 0.0 {
                        continue;
                    }
                    let guided_weight = self.pair_weight(index, second);
                    pairs[pair_slot(piece_count, index, second)] = PairEntry {
                        raw_loss: penalty,
                        guided_weight,
                        normalization_scale: 1.0,
                    };
                    incident_raw_loss[index] += penalty;
                    incident_raw_loss[second] += penalty;
                    collision_pairs.push((index, second, penalty));
                    weighted_loss += guided_weight * penalty;
                }
            }
            return Ok(PairTracker {
                piece_count,
                boundaries,
                pairs,
                incident_raw_loss,
                boundary_violations,
                boundary_loss,
                collision_pairs,
                weighted_loss,
            });
        }
        #[cfg(not(feature = "jagua-experimental"))]
        {
            let _ = state;
            Err(GeneralFastError::InvalidSettings(
                "dynamic hazard search requires the jagua-experimental feature".to_owned(),
            ))
        }
    }

    fn score_placement_dynamic(
        &mut self,
        state: &RelaxedState,
        input_index: usize,
        candidate: &RelaxedPlacement,
        upper_bound: Option<f64>,
    ) -> Result<PlacementScore, GeneralFastError> {
        #[cfg(feature = "jagua-experimental")]
        {
            self.counters.surrogate_evaluations =
                self.counters.surrogate_evaluations.saturating_add(1);
            let (boundary_violations, boundary_loss) =
                self.boundary_penalty(candidate, state.strip_depth_mm)?;
            if self.dynamic_query_budget_exhausted() {
                return Ok(PlacementScore {
                    boundary_violations,
                    boundary_loss,
                    collision_pairs: Vec::new(),
                    weighted_loss: f64::INFINITY,
                });
            }
            let mut weighted_loss = boundary_loss;
            let mut collision_pairs = Vec::new();
            let query = self
                .hazard_index
                .as_mut()
                .expect("dynamic hazard index is prepared before search")
                .query(input_index, hazard_pose(candidate), None)
                .map_err(dynamic_hazard_error)?;
            self.counters.dynamic_hazard_queries =
                self.counters.dynamic_hazard_queries.saturating_add(1);
            let GeneralHazardQuery::Complete {
                colliding_piece_ids,
                ..
            } = query
            else {
                return Err(GeneralFastError::InvalidInput(
                    "dynamic hazard placement scoring requires a complete query".to_owned(),
                ));
            };
            for fixed_index in colliding_piece_ids {
                let penalty = if self.uses_dynamic_pressure() {
                    self.hazard_index
                        .as_mut()
                        .expect("dynamic hazard index is prepared before search")
                        .collision_pressure(input_index, hazard_pose(candidate), fixed_index)
                        .map_err(dynamic_hazard_error)?
                } else {
                    self.rollback_pair_pressure(candidate, &state.placements[fixed_index])?
                };
                self.counters.dynamic_pressure_evaluations =
                    self.counters.dynamic_pressure_evaluations.saturating_add(1);
                let pair = ordered_pair(input_index, fixed_index);
                collision_pairs.push((pair.0, pair.1, penalty));
                weighted_loss += self.pair_weight(pair.0, pair.1) * penalty;
                if upper_bound.is_some_and(|upper_bound| weighted_loss > upper_bound) {
                    break;
                }
            }
            collision_pairs.sort_by_key(|(first, second, _)| (*first, *second));
            return Ok(PlacementScore {
                boundary_violations,
                boundary_loss,
                collision_pairs,
                weighted_loss,
            });
        }
        #[cfg(not(feature = "jagua-experimental"))]
        {
            let _ = (state, input_index, candidate, upper_bound);
            Err(GeneralFastError::InvalidSettings(
                "dynamic hazard search requires the jagua-experimental feature".to_owned(),
            ))
        }
    }

    fn confirm_dynamic_replacement(
        &mut self,
        state: &RelaxedState,
        input_index: usize,
        candidate: &RelaxedPlacement,
        search_score: &PlacementScore,
    ) -> Result<PlacementScore, GeneralFastError> {
        let (boundary_violations, boundary_loss) =
            self.boundary_penalty(candidate, state.strip_depth_mm)?;
        let mut collision_pairs = Vec::new();
        let mut weighted_loss = boundary_loss;
        for fixed_index in 0..state.placements.len() {
            if fixed_index == input_index {
                continue;
            }
            let penalty =
                self.confirmed_pair_pressure(candidate, &state.placements[fixed_index])?;
            if penalty == 0.0 {
                continue;
            }
            let pair = ordered_pair(input_index, fixed_index);
            collision_pairs.push((pair.0, pair.1, penalty));
            weighted_loss += self.pair_weight(pair.0, pair.1) * penalty;
        }
        collision_pairs.sort_by_key(|(first, second, _)| (*first, *second));
        let searched = search_score
            .collision_pairs
            .iter()
            .map(|(first, second, _)| (*first, *second))
            .collect::<BTreeSet<_>>();
        let confirmed = collision_pairs
            .iter()
            .map(|(first, second, _)| (*first, *second))
            .collect::<BTreeSet<_>>();
        self.counters.retained_f64_confirmations =
            self.counters.retained_f64_confirmations.saturating_add(1);
        self.counters.confirmed_pair_additions = self
            .counters
            .confirmed_pair_additions
            .saturating_add(confirmed.difference(&searched).count());
        self.counters.confirmed_pair_removals = self
            .counters
            .confirmed_pair_removals
            .saturating_add(searched.difference(&confirmed).count());
        Ok(PlacementScore {
            boundary_violations,
            boundary_loss,
            collision_pairs,
            weighted_loss,
        })
    }

    fn confirmed_pair_pressure(
        &mut self,
        first: &RelaxedPlacement,
        second: &RelaxedPlacement,
    ) -> Result<f64, GeneralFastError> {
        let first_key = (
            self.catalog.geometry_class_by_input[first.input_index],
            angle_key(0.0),
            first.mirrored,
        );
        let second_key = (
            self.catalog.geometry_class_by_input[second.input_index],
            angle_key(0.0),
            second.mirrored,
        );
        let first_shape = self.catalog.orientations.get(&first_key).ok_or_else(|| {
            GeneralPolygonError::from_message("missing zero-degree confirmation surrogate")
        })?;
        let second_shape = self.catalog.orientations.get(&second_key).ok_or_else(|| {
            GeneralPolygonError::from_message("missing zero-degree confirmation surrogate")
        })?;
        let (collides, cell_probes, sat_tests) =
            continuous_pair_collision(first_shape, first, second_shape, second);
        self.counters.piece_broad_phase_probes =
            self.counters.piece_broad_phase_probes.saturating_add(1);
        self.counters.cell_index_probes =
            self.counters.cell_index_probes.saturating_add(cell_probes);
        self.counters.sat_tests = self.counters.sat_tests.saturating_add(sat_tests);
        if !collides {
            return Ok(0.0);
        }
        let pressure = match self.relaxed_settings.pressure_model {
            GeneralRelaxedPressureModel::StructuredTrianglePoles => {
                self.rollback_pair_pressure(first, second)?
            }
            GeneralRelaxedPressureModel::DirectionalPenetration => {
                return Err(GeneralFastError::InvalidSettings(
                    "directional penetration requires rollback candidate scoring".to_owned(),
                ));
            }
            GeneralRelaxedPressureModel::ContinuousTrianglePoles => {
                continuous_pole_overlap_pressure(
                    first_shape,
                    first.rotation_deg,
                    first.translate_x,
                    first.translate_y,
                    second_shape,
                    second.rotation_deg,
                    second.translate_x,
                    second.translate_y,
                )
            }
            GeneralRelaxedPressureModel::DynamicPoles => {
                #[cfg(feature = "jagua-experimental")]
                {
                    self.hazard_index
                        .as_mut()
                        .expect("dynamic hazard index is prepared before confirmation")
                        .collision_pressure(
                            first.input_index,
                            hazard_pose(first),
                            second.input_index,
                        )
                        .map_err(dynamic_hazard_error)?
                }
                #[cfg(not(feature = "jagua-experimental"))]
                {
                    return Err(GeneralFastError::InvalidSettings(
                        "dynamic pressure requires the jagua-experimental feature".to_owned(),
                    ));
                }
            }
        };
        self.counters.dynamic_pressure_evaluations =
            self.counters.dynamic_pressure_evaluations.saturating_add(1);
        Ok(pressure)
    }

    fn build_piece_index(&mut self, state: &RelaxedState) -> Result<PieceIndex, GeneralFastError> {
        let inset = collision_sheet_inset_mm(self.fast_settings);
        let mut index = PieceIndex::new(IrregularBounds::new(
            inset,
            inset,
            self.fast_settings.sheet_short_axis_mm - inset,
            state.strip_depth_mm - inset,
        ));
        for (piece_index, placement) in state.placements.iter().enumerate() {
            index.insert(piece_index, self.placement_bounds(placement)?);
        }
        Ok(index)
    }

    fn placement_bounds(
        &mut self,
        placement: &RelaxedPlacement,
    ) -> Result<IrregularBounds, GeneralFastError> {
        let shape_bounds = self.local_shape_bounds(
            placement.input_index,
            placement.rotation_deg,
            placement.mirrored,
        )?;
        Ok(translated_bounds(
            shape_bounds,
            placement.translate_x,
            placement.translate_y,
        ))
    }

    fn boundary_penalty(
        &mut self,
        placement: &RelaxedPlacement,
        strip_depth_mm: f64,
    ) -> Result<(usize, f64), GeneralFastError> {
        let bounds = self.placement_bounds(placement)?;
        let inset = collision_sheet_inset_mm(self.fast_settings);
        let overflow = [
            (inset - bounds.min_x).max(0.0),
            (bounds.max_x - (self.fast_settings.sheet_short_axis_mm - inset)).max(0.0),
            (inset - bounds.min_y).max(0.0),
            (bounds.max_y - (strip_depth_mm - inset)).max(0.0),
        ];
        let violations = overflow.iter().filter(|value| **value > 0.0).count();
        if violations == 0 {
            return Ok((0, 0.0));
        }
        let width = (bounds.max_x - bounds.min_x).max(0.0);
        let height = (bounds.max_y - bounds.min_y).max(0.0);
        let area = width * height;
        let inside_width = (bounds
            .max_x
            .min(self.fast_settings.sheet_short_axis_mm - inset)
            - bounds.min_x.max(inset))
        .max(0.0);
        let inside_height =
            (bounds.max_y.min(strip_depth_mm - inset) - bounds.min_y.max(inset)).max(0.0);
        let outside_area = (area - inside_width * inside_height).max(0.0) + area * 0.0001;
        Ok((violations, 2.0 * outside_area.sqrt() * area.sqrt()))
    }

    fn resolve_pair_nfp(
        &mut self,
        fixed: &RelaxedPlacement,
        moving: &RelaxedPlacement,
    ) -> Result<Option<&PairNfp>, GeneralFastError> {
        let key = self.pair_nfp_key(fixed, moving)?;
        if !self.pair_nfp_cache.contains_key(&key) {
            let component_count = self.pair_nfp_component_count(key)?;
            if component_count > MAX_NFP_COMPONENTS_PER_MOVE
                || self
                    .pair_nfp_cache_components
                    .saturating_add(component_count)
                    > MAX_LANE_NFP_COMPONENTS
            {
                return Ok(None);
            }
            self.build_pair_nfp(key)?;
        }
        Ok(self.pair_nfp_cache.get(&key).map(Arc::as_ref))
    }

    fn pair_nfp_component_count(&self, key: PairNfpKey) -> Result<usize, GeneralFastError> {
        let fixed = self
            .catalog
            .orientations
            .get(&(key.0, key.1, key.2))
            .ok_or_else(|| GeneralPolygonError::from_message("missing fixed NFP surrogate"))?;
        let moving = self
            .catalog
            .orientations
            .get(&(key.3, key.4, key.5))
            .ok_or_else(|| GeneralPolygonError::from_message("missing moving NFP surrogate"))?;
        Ok(fixed.cells.len().saturating_mul(moving.cells.len()))
    }

    fn build_pair_nfp(&mut self, key: PairNfpKey) -> Result<(), GeneralFastError> {
        if self.pair_nfp_cache.contains_key(&key) {
            return Ok(());
        }
        if let Some(shared) = self.catalog.shared_pair_nfps.get(&key).cloned() {
            self.pair_nfp_cache_components = self
                .pair_nfp_cache_components
                .saturating_add(shared.components.len());
            self.counters.shared_pair_nfp_adoptions =
                self.counters.shared_pair_nfp_adoptions.saturating_add(1);
            self.pair_nfp_cache.insert(key, shared);
            return Ok(());
        }
        let pair_nfp = Arc::new(build_pair_nfp_value(&self.catalog.orientations, key)?);
        self.pair_nfp_cache_components = self
            .pair_nfp_cache_components
            .saturating_add(pair_nfp.components.len());
        self.counters.pair_nfp_builds = self.counters.pair_nfp_builds.saturating_add(1);
        self.counters.pair_nfp_components = self
            .counters
            .pair_nfp_components
            .saturating_add(pair_nfp.components.len());
        self.pair_nfp_cache.insert(key, pair_nfp);
        Ok(())
    }

    fn preflight_directional_pair_nfps(
        &mut self,
        keys: &[PairNfpKey],
        enforce_candidate_limit: bool,
    ) -> Result<bool, GeneralFastError> {
        let mut keys = keys.to_vec();
        keys.sort_unstable();
        keys.dedup();
        let mut visits = 0usize;
        let mut allocations = 0usize;
        for key in keys.iter().copied() {
            let components = self.pair_nfp_component_count(key)?;
            visits = visits.saturating_add(components);
            if self.pair_nfp_cache.contains_key(&key) {
                self.counters.directional_cache_hits =
                    self.counters.directional_cache_hits.saturating_add(1);
            } else {
                self.counters.directional_cache_misses =
                    self.counters.directional_cache_misses.saturating_add(1);
                allocations = allocations.saturating_add(components);
            }
        }
        if !directional_nfp_preflight_fits(
            self.pair_nfp_cache_components,
            allocations,
            visits,
            enforce_candidate_limit,
            MAX_NFP_COMPONENTS_PER_MOVE,
            MAX_LANE_NFP_COMPONENTS,
        ) {
            self.counters.directional_over_budget_candidates = self
                .counters
                .directional_over_budget_candidates
                .saturating_add(1);
            return Ok(false);
        }
        for key in keys {
            self.build_pair_nfp(key)?;
        }
        Ok(true)
    }

    fn pair_nfp_key(
        &self,
        fixed: &RelaxedPlacement,
        moving: &RelaxedPlacement,
    ) -> Result<PairNfpKey, GeneralFastError> {
        let fixed_key =
            self.ensure_oriented(fixed.input_index, fixed.rotation_deg, fixed.mirrored)?;
        let moving_key =
            self.ensure_oriented(moving.input_index, moving.rotation_deg, moving.mirrored)?;
        Ok((
            fixed_key.0,
            fixed_key.1,
            fixed_key.2,
            moving_key.0,
            moving_key.1,
            moving_key.2,
        ))
    }

    fn pair_penalty(
        &mut self,
        first: &RelaxedPlacement,
        second: &RelaxedPlacement,
    ) -> Result<f64, GeneralFastError> {
        if !self.pair_collides(first, second)? {
            return Ok(0.0);
        }
        if self.uses_directional_pressure() {
            return Err(GeneralFastError::InvalidSettings(
                "directional penetration requires candidate-scoped scoring".to_owned(),
            ));
        }
        let first_key =
            self.ensure_oriented(first.input_index, first.rotation_deg, first.mirrored)?;
        let second_key =
            self.ensure_oriented(second.input_index, second.rotation_deg, second.mirrored)?;
        let first_shape = self
            .catalog
            .orientations
            .get(&first_key)
            .expect("ensured first oriented surrogate");
        let second_shape = self
            .catalog
            .orientations
            .get(&second_key)
            .expect("ensured second oriented surrogate");
        Ok(pole_overlap_pressure(
            first_shape,
            first.translate_x,
            first.translate_y,
            second_shape,
            second.translate_x,
            second.translate_y,
        ))
    }

    fn pair_collides(
        &mut self,
        first: &RelaxedPlacement,
        second: &RelaxedPlacement,
    ) -> Result<bool, GeneralFastError> {
        self.counters.piece_broad_phase_probes += 1;
        let first_key =
            self.ensure_oriented(first.input_index, first.rotation_deg, first.mirrored)?;
        let second_key =
            self.ensure_oriented(second.input_index, second.rotation_deg, second.mirrored)?;
        let first_shape = self
            .catalog
            .orientations
            .get(&first_key)
            .expect("ensured first oriented surrogate");
        let second_shape = self
            .catalog
            .orientations
            .get(&second_key)
            .expect("ensured second oriented surrogate");
        if self.uses_directional_pressure() {
            let relative_x = relative_grid_coordinate(first.translate_x, second.translate_x)
                .ok_or_else(|| {
                    GeneralFastError::InvalidInput(
                        "directional horizontal translation is outside the canonical grid"
                            .to_owned(),
                    )
                })?;
            let relative_y = relative_grid_coordinate(first.translate_y, second.translate_y)
                .ok_or_else(|| {
                    GeneralFastError::InvalidInput(
                        "directional vertical translation is outside the canonical grid".to_owned(),
                    )
                })?;
            let mut sat_positive = false;
            'cells: for first_cell in &first_shape.cells {
                for second_cell in &second_shape.cells {
                    self.counters.sat_tests = self.counters.sat_tests.saturating_add(1);
                    let overlaps = triangles_overlap_on_grid(
                        *first_cell,
                        *second_cell,
                        relative_x,
                        relative_y,
                    )
                    .ok_or_else(|| {
                        GeneralFastError::InvalidInput(
                            "directional collision coordinates are outside the canonical grid"
                                .to_owned(),
                        )
                    })?;
                    if overlaps {
                        sat_positive = true;
                        break 'cells;
                    }
                }
            }
            if !sat_positive {
                return Ok(false);
            }
            let first_collision = first_shape
                .collision
                .translated(first.translate_x, first.translate_y)?;
            let second_collision = second_shape
                .collision
                .translated(second.translate_x, second.translate_y)?;
            let overlaps = polygons_overlap_exact(&first_collision, &second_collision)?;
            self.counters.directional_exact_confirmations = self
                .counters
                .directional_exact_confirmations
                .saturating_add(1);
            return Ok(overlaps);
        }
        let first_bounds =
            translated_bounds(first_shape.bounds, first.translate_x, first.translate_y);
        let second_bounds =
            translated_bounds(second_shape.bounds, second.translate_x, second.translate_y);
        if !bounds_overlap(first_bounds, second_bounds) {
            return Ok(false);
        }
        let relative_x = second.translate_x - first.translate_x;
        let relative_y = second.translate_y - first.translate_y;
        for first_cell in &first_shape.cells {
            let first_cell_bounds =
                translated_bounds(first_cell.bounds, first.translate_x, first.translate_y);
            self.counters.cell_index_probes += 1;
            let mut cell_mask = second_shape.cell_index.query_mask(
                first_cell_bounds,
                second.translate_x,
                second.translate_y,
            );
            for (word_index, word) in cell_mask.iter_mut().enumerate() {
                while *word != 0 {
                    let bit = word.trailing_zeros() as usize;
                    *word &= *word - 1;
                    let second_cell_index = word_index * 64 + bit;
                    let second_cell = second_shape.cells[second_cell_index];
                    let second_cell_bounds = translated_bounds(
                        second_cell.bounds,
                        second.translate_x,
                        second.translate_y,
                    );
                    if !bounds_overlap(first_cell_bounds, second_cell_bounds) {
                        continue;
                    }
                    self.counters.sat_tests += 1;
                    if triangle_penetration(
                        *first_cell,
                        0.0,
                        0.0,
                        second_cell,
                        relative_x,
                        relative_y,
                    )
                    .is_some()
                    {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }

    fn rollback_pair_pressure(
        &self,
        first: &RelaxedPlacement,
        second: &RelaxedPlacement,
    ) -> Result<f64, GeneralFastError> {
        if self.uses_continuous_triangle_pressure() {
            let first_shape = self.oriented(first.input_index, 0.0, first.mirrored)?;
            let second_shape = self.oriented(second.input_index, 0.0, second.mirrored)?;
            return Ok(continuous_pole_overlap_pressure(
                first_shape,
                first.rotation_deg,
                first.translate_x,
                first.translate_y,
                second_shape,
                second.rotation_deg,
                second.translate_x,
                second.translate_y,
            ));
        }
        let first_shape =
            self.pressure_oriented(first.input_index, first.rotation_deg, first.mirrored)?;
        let second_shape =
            self.pressure_oriented(second.input_index, second.rotation_deg, second.mirrored)?;
        Ok(pole_overlap_pressure(
            first_shape,
            first.translate_x,
            first.translate_y,
            second_shape,
            second.translate_x,
            second.translate_y,
        ))
    }

    fn pressure_oriented(
        &self,
        input_index: usize,
        rotation_deg: f64,
        mirrored: bool,
    ) -> Result<&OrientedSurrogate, GeneralFastError> {
        self.oriented(input_index, rotation_deg, mirrored)
    }

    fn pair_weight(&self, first: usize, second: usize) -> f64 {
        self.weights
            .get(&ordered_pair(first, second))
            .copied()
            .unwrap_or(1.0)
    }

    fn oriented(
        &self,
        input_index: usize,
        rotation_deg: f64,
        mirrored: bool,
    ) -> Result<&OrientedSurrogate, GeneralFastError> {
        let key = self.ensure_oriented(input_index, rotation_deg, mirrored)?;
        Ok(self
            .catalog
            .orientations
            .get(&key)
            .expect("surrogate catalog contains every canonical orientation"))
    }

    fn ensure_oriented(
        &self,
        input_index: usize,
        rotation_deg: f64,
        mirrored: bool,
    ) -> Result<SurrogateKey, GeneralFastError> {
        let angle = if self.uses_directional_pressure() {
            continuous_angle(rotation_deg)
        } else {
            canonical_angle(rotation_deg)
        };
        let key = (
            self.catalog.geometry_class_by_input[input_index],
            angle_key(angle),
            mirrored,
        );
        if !self.catalog.orientations.contains_key(&key) {
            return Err(GeneralPolygonError::from_message(format!(
                "relaxed surrogate catalog is missing canonical orientation {} for piece {}",
                angle_from_key(key.1),
                self.pieces[input_index].id
            ))
            .into());
        }
        Ok(key)
    }
}

fn directional_nfp_preflight_fits(
    current_components: usize,
    new_components: usize,
    candidate_visits: usize,
    enforce_candidate_limit: bool,
    candidate_limit: usize,
    lane_limit: usize,
) -> bool {
    (!enforce_candidate_limit || candidate_visits <= candidate_limit)
        && current_components.saturating_add(new_components) <= lane_limit
}

fn pole_overlap_pressure(
    first_shape: &OrientedSurrogate,
    first_translate_x: f64,
    first_translate_y: f64,
    second_shape: &OrientedSurrogate,
    second_translate_x: f64,
    second_translate_y: f64,
) -> f64 {
    let epsilon =
        first_shape.diameter.max(second_shape.diameter) * OVERLAP_PROXY_EPSILON_DIAMETER_RATIO;
    let mut overlap_proxy = epsilon * epsilon;
    for first_pole in &first_shape.poles {
        let first_center = IrregularPoint::new(
            first_pole.center.x + first_translate_x,
            first_pole.center.y + first_translate_y,
        );
        for second_pole in &second_shape.poles {
            let second_center = IrregularPoint::new(
                second_pole.center.x + second_translate_x,
                second_pole.center.y + second_translate_y,
            );
            let distance =
                (first_center.x - second_center.x).hypot(first_center.y - second_center.y);
            let penetration = first_pole.radius + second_pole.radius - distance;
            let decayed = if penetration >= epsilon {
                penetration
            } else {
                epsilon * epsilon / (-penetration + 2.0 * epsilon)
            };
            overlap_proxy +=
                std::f64::consts::PI * decayed * first_pole.radius.min(second_pole.radius);
        }
    }
    overlap_proxy.sqrt() * (first_shape.difficulty * second_shape.difficulty).sqrt()
}

fn continuous_pole_overlap_pressure(
    first_shape: &OrientedSurrogate,
    first_rotation_deg: f64,
    first_translate_x: f64,
    first_translate_y: f64,
    second_shape: &OrientedSurrogate,
    second_rotation_deg: f64,
    second_translate_x: f64,
    second_translate_y: f64,
) -> f64 {
    let first_transform =
        PoleTransform::new(first_rotation_deg, first_translate_x, first_translate_y);
    let second_transform =
        PoleTransform::new(second_rotation_deg, second_translate_x, second_translate_y);
    let first_bounds = transformed_surrogate_bounds(first_shape, first_transform);
    let second_bounds = transformed_surrogate_bounds(second_shape, second_transform);
    let first_diameter =
        (first_bounds.max_x - first_bounds.min_x).hypot(first_bounds.max_y - first_bounds.min_y);
    let second_diameter = (second_bounds.max_x - second_bounds.min_x)
        .hypot(second_bounds.max_y - second_bounds.min_y);
    let epsilon = first_diameter.max(second_diameter) * OVERLAP_PROXY_EPSILON_DIAMETER_RATIO;
    let mut overlap_proxy = epsilon * epsilon;
    for first_pole in &first_shape.poles {
        let first_center = first_transform.point(first_pole.center);
        for second_pole in &second_shape.poles {
            let second_center = second_transform.point(second_pole.center);
            let distance =
                (first_center.x - second_center.x).hypot(first_center.y - second_center.y);
            let penetration = first_pole.radius + second_pole.radius - distance;
            let decayed = if penetration >= epsilon {
                penetration
            } else {
                epsilon * epsilon / (-penetration + 2.0 * epsilon)
            };
            overlap_proxy +=
                std::f64::consts::PI * decayed * first_pole.radius.min(second_pole.radius);
        }
    }
    let first_difficulty = ((first_bounds.max_x - first_bounds.min_x)
        * (first_bounds.max_y - first_bounds.min_y))
        .sqrt()
        .max(1.0);
    let second_difficulty = ((second_bounds.max_x - second_bounds.min_x)
        * (second_bounds.max_y - second_bounds.min_y))
        .sqrt()
        .max(1.0);
    overlap_proxy.sqrt() * (first_difficulty * second_difficulty).sqrt()
}

fn continuous_pair_collision(
    first_shape: &OrientedSurrogate,
    first: &RelaxedPlacement,
    second_shape: &OrientedSurrogate,
    second: &RelaxedPlacement,
) -> (bool, usize, usize) {
    let first_transform =
        PoleTransform::new(first.rotation_deg, first.translate_x, first.translate_y);
    let second_transform =
        PoleTransform::new(second.rotation_deg, second.translate_x, second.translate_y);
    let first_bounds = transformed_surrogate_bounds(first_shape, first_transform);
    let second_bounds = transformed_surrogate_bounds(second_shape, second_transform);
    if !bounds_overlap(first_bounds, second_bounds) {
        return (false, 0, 0);
    }
    let mut cell_probes = 0usize;
    let mut sat_tests = 0usize;
    for first_cell in first_shape.cells.iter().copied() {
        let first_cell = transform_triangle(first_cell, first_transform);
        for second_cell in second_shape.cells.iter().copied() {
            cell_probes = cell_probes.saturating_add(1);
            let second_cell = transform_triangle(second_cell, second_transform);
            if !bounds_overlap(first_cell.bounds, second_cell.bounds) {
                continue;
            }
            sat_tests = sat_tests.saturating_add(1);
            if triangle_penetration(first_cell, 0.0, 0.0, second_cell, 0.0, 0.0).is_some() {
                return (true, cell_probes, sat_tests);
            }
        }
    }
    (false, cell_probes, sat_tests)
}

fn transform_triangle(triangle: Triangle, transform: PoleTransform) -> Triangle {
    Triangle::new(triangle.points.map(|point| transform.point(point)))
}

#[derive(Clone, Copy)]
struct PoleTransform {
    sin: f64,
    cos: f64,
    translate_x: f64,
    translate_y: f64,
}

impl PoleTransform {
    fn new(rotation_deg: f64, translate_x: f64, translate_y: f64) -> Self {
        let (sin, cos) = continuous_angle(rotation_deg).to_radians().sin_cos();
        Self {
            sin,
            cos,
            translate_x,
            translate_y,
        }
    }

    fn point(self, point: IrregularPoint) -> IrregularPoint {
        IrregularPoint::new(
            point.x * self.cos - point.y * self.sin + self.translate_x,
            point.x * self.sin + point.y * self.cos + self.translate_y,
        )
    }
}

fn transformed_surrogate_bounds(
    shape: &OrientedSurrogate,
    transform: PoleTransform,
) -> IrregularBounds {
    let mut bounds = IrregularBounds::new(
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for point in shape
        .cells
        .iter()
        .flat_map(|triangle| triangle.points.iter().copied())
    {
        let point = transform.point(point);
        bounds.min_x = bounds.min_x.min(point.x);
        bounds.min_y = bounds.min_y.min(point.y);
        bounds.max_x = bounds.max_x.max(point.x);
        bounds.max_y = bounds.max_y.max(point.y);
    }
    bounds
}

fn build_pair_nfp_value(
    orientations: &BTreeMap<SurrogateKey, OrientedSurrogate>,
    key: PairNfpKey,
) -> Result<PairNfp, GeneralFastError> {
    let fixed_cells = &orientations
        .get(&(key.0, key.1, key.2))
        .ok_or_else(|| GeneralPolygonError::from_message("missing fixed NFP surrogate"))?
        .cells;
    let moving_cells = &orientations
        .get(&(key.3, key.4, key.5))
        .ok_or_else(|| GeneralPolygonError::from_message("missing moving NFP surrogate"))?
        .cells;
    let mut components = Vec::with_capacity(fixed_cells.len().saturating_mul(moving_cells.len()));
    for fixed_cell in fixed_cells.iter().copied() {
        for moving_cell in moving_cells.iter().copied() {
            let boundary =
                compute_relative_nfp_boundary_reference(&fixed_cell.points, &moving_cell.points)
                    .map_err(|message| {
                        GeneralPolygonError::from_message(format!(
                            "relaxed pair NFP construction failed: {message}"
                        ))
                    })?;
            let bounds = bounds_for_points(&boundary.points).ok_or_else(|| {
                GeneralPolygonError::from_message("relaxed pair NFP component is empty")
            })?;
            components.push(ConvexNfp {
                points: boundary.points,
                bounds,
            });
        }
    }
    Ok(PairNfp { components })
}

fn build_shared_pair_nfps(
    orientations: &BTreeMap<SurrogateKey, OrientedSurrogate>,
) -> Result<(BTreeMap<PairNfpKey, Arc<PairNfp>>, WorkCounters), GeneralFastError> {
    let orientation_keys = orientations.keys().copied().collect::<Vec<_>>();
    let mut keys = Vec::with_capacity(orientation_keys.len().saturating_pow(2));
    let mut component_count = 0usize;
    for fixed in orientation_keys.iter().copied() {
        for moving in orientation_keys.iter().copied() {
            let key = (fixed.0, fixed.1, fixed.2, moving.0, moving.1, moving.2);
            let fixed_cells = orientations
                .get(&fixed)
                .expect("shared NFP fixed orientation is present")
                .cells
                .len();
            let moving_cells = orientations
                .get(&moving)
                .expect("shared NFP moving orientation is present")
                .cells
                .len();
            component_count =
                component_count.saturating_add(fixed_cells.saturating_mul(moving_cells));
            keys.push(key);
        }
    }
    keys.sort_unstable();
    keys.dedup();
    let entry_bytes = std::mem::size_of::<PairNfpKey>()
        .saturating_add(std::mem::size_of::<Arc<PairNfp>>())
        .saturating_add(std::mem::size_of::<PairNfp>())
        .saturating_add(128);
    let component_bytes = std::mem::size_of::<ConvexNfp>().saturating_add(
        MAX_TRIANGLE_NFP_POINTS.saturating_mul(std::mem::size_of::<IrregularPoint>()),
    );
    let estimated_bytes = keys
        .len()
        .saturating_mul(entry_bytes)
        .saturating_add(component_count.saturating_mul(component_bytes));
    if component_count > MAX_SHARED_NFP_COMPONENTS
        || estimated_bytes > MAX_SHARED_NFP_ESTIMATED_BYTES
    {
        return Ok((BTreeMap::new(), WorkCounters::default()));
    }
    let mut table = BTreeMap::new();
    for key in keys {
        table.insert(key, Arc::new(build_pair_nfp_value(orientations, key)?));
    }
    let counters = WorkCounters {
        shared_pair_nfp_entries: table.len(),
        shared_pair_nfp_components: component_count,
        shared_pair_nfp_estimated_bytes: estimated_bytes,
        ..WorkCounters::default()
    };
    Ok((table, counters))
}

fn build_surrogate_catalog(
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    mode: SurrogateCatalogMode,
    assignment: Option<&GeneralFastResult>,
) -> Result<(Arc<SurrogateCatalog>, WorkCounters), GeneralFastError> {
    let mut catalog = BTreeMap::new();
    let mut counters = WorkCounters::default();
    let angle_count = (360.0 / SURROGATE_ANGLE_STEP_DEG).round() as usize;
    let mut representatives = Vec::<usize>::new();
    let mut geometry_class_by_input = Vec::with_capacity(pieces.len());
    for (input_index, piece) in pieces.iter().enumerate() {
        let geometry_class = representatives
            .iter()
            .position(|representative| pieces[*representative].polygon == piece.polygon)
            .unwrap_or_else(|| {
                representatives.push(input_index);
                representatives.len() - 1
            });
        geometry_class_by_input.push(geometry_class);
    }
    for (geometry_class, input_index) in representatives.into_iter().enumerate() {
        let piece = pieces[input_index];
        let class_allows_rotation = pieces.iter().enumerate().any(|(index, piece)| {
            geometry_class_by_input[index] == geometry_class && piece.allow_rotation
        });
        let class_allows_mirror = pieces.iter().enumerate().any(|(index, piece)| {
            geometry_class_by_input[index] == geometry_class && piece.allow_mirror
        });
        let mirrors: &[bool] = if class_allows_mirror {
            &[false, true]
        } else {
            &[false]
        };
        let mut poses = match mode {
            SurrogateCatalogMode::StructuredGrid => {
                let angles = if class_allows_rotation {
                    (0..angle_count)
                        .map(|index| index as f64 * SURROGATE_ANGLE_STEP_DEG)
                        .collect::<Vec<_>>()
                } else {
                    vec![0.0]
                };
                angles
                    .into_iter()
                    .flat_map(|angle| {
                        mirrors
                            .iter()
                            .copied()
                            .map(move |mirrored| (canonical_angle(angle), mirrored))
                    })
                    .collect::<Vec<_>>()
            }
            SurrogateCatalogMode::CurrentAssignment => {
                let placements = assignment
                    .expect("current-assignment catalog requires an incumbent")
                    .placements
                    .iter()
                    .map(|placement| (placement.piece_id.as_str(), placement))
                    .collect::<BTreeMap<_, _>>();
                pieces
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| geometry_class_by_input[*index] == geometry_class)
                    .map(|(_, piece)| {
                        placements
                            .get(piece.id)
                            .map(|placement| {
                                (continuous_angle(placement.rotation_deg), placement.mirrored)
                            })
                            .unwrap_or((0.0, false))
                    })
                    .collect::<Vec<_>>()
            }
            SurrogateCatalogMode::ZeroDegreeOnly => mirrors
                .iter()
                .copied()
                .map(|mirrored| (0.0, mirrored))
                .collect::<Vec<_>>(),
        };
        poses.sort_by_key(|(angle, mirrored)| (angle_key(*angle), *mirrored));
        poses.dedup_by_key(|(angle, mirrored)| (angle_key(*angle), *mirrored));
        for (angle, mirrored) in poses {
            let key = (geometry_class, angle_key(angle), mirrored);
            let polygon = piece
                .polygon
                .transformed(angle_from_key(key.1), mirrored, 0.0, 0.0)?
                .offset(collision_expansion_mm(settings))?;
            if polygon
                .regions()
                .iter()
                .any(|region| !region.holes.is_empty())
            {
                return Err(GeneralPolygonError::from_message(
                    "relaxed surrogate does not yet support offset holes",
                )
                .into());
            }
            let mut cells = Vec::new();
            for region in polygon.regions() {
                cells.extend(triangulate_ring(region.outer.points())?);
            }
            if cells.is_empty() || cells.len() > MAX_CELLS_PER_PIECE {
                return Err(GeneralPolygonError::from_message(format!(
                    "relaxed surrogate cell count must be between 1 and {MAX_CELLS_PER_PIECE}"
                ))
                .into());
            }
            counters.oriented_surrogate_builds += 1;
            counters.generated_cells = counters.generated_cells.saturating_add(cells.len());
            if counters.generated_cells > MAX_CELLS_PER_JOB {
                return Err(GeneralPolygonError::from_message(format!(
                    "relaxed surrogate job may contain at most {MAX_CELLS_PER_JOB} generated cells"
                ))
                .into());
            }
            let bounds = polygon.bounds().ok_or_else(|| {
                GeneralPolygonError::from_message("relaxed surrogate geometry is empty")
            })?;
            let hull_area_scale = ((bounds.max_x - bounds.min_x) * (bounds.max_y - bounds.min_y))
                .sqrt()
                .max(1.0);
            let poles = cells.iter().copied().map(triangle_pole).collect();
            let cell_index = CellIndex::new(&cells, bounds);
            catalog.insert(
                key,
                OrientedSurrogate {
                    collision: polygon,
                    cells,
                    poles,
                    bounds,
                    cell_index,
                    difficulty: hull_area_scale,
                    diameter: (bounds.max_x - bounds.min_x).hypot(bounds.max_y - bounds.min_y),
                },
            );
        }
    }
    let (shared_pair_nfps, shared_work) = if mode == SurrogateCatalogMode::CurrentAssignment {
        build_shared_pair_nfps(&catalog)?
    } else {
        (BTreeMap::new(), WorkCounters::default())
    };
    counters.accumulate(shared_work);
    Ok((
        Arc::new(SurrogateCatalog {
            geometry_class_by_input,
            orientations: catalog,
            shared_pair_nfps,
        }),
        counters,
    ))
}

fn initialize_complete_state(
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    collision_backend: GeneralRelaxedCollisionBackend,
    angle_seed_policy: GeneralRelaxedAngleSeedPolicy,
    pressure_model: GeneralRelaxedPressureModel,
    incumbent: &GeneralFastResult,
) -> Result<RelaxedState, GeneralFastError> {
    let by_id = incumbent
        .placements
        .iter()
        .map(|placement| (placement.piece_id.as_str(), placement))
        .collect::<BTreeMap<_, _>>();
    let inset = collision_sheet_inset_mm(settings);
    let mut shelf_y = incumbent.used_long_axis_depth_mm.max(inset);
    let mut placements = Vec::with_capacity(pieces.len());
    for (input_index, piece) in pieces.iter().enumerate() {
        if let Some(existing) = by_id.get(piece.id) {
            placements.push(RelaxedPlacement {
                input_index,
                rotation_deg: match (pressure_model, collision_backend, angle_seed_policy) {
                    (GeneralRelaxedPressureModel::DirectionalPenetration, _, _) => {
                        continuous_angle(existing.rotation_deg)
                    }
                    (
                        _,
                        GeneralRelaxedCollisionBackend::DynamicHazard,
                        GeneralRelaxedAngleSeedPolicy::ContinuousUniform,
                    ) => continuous_angle(existing.rotation_deg),
                    (
                        _,
                        GeneralRelaxedCollisionBackend::DynamicHazard,
                        GeneralRelaxedAngleSeedPolicy::CurrentOnly,
                    ) => continuous_angle(existing.rotation_deg),
                    _ => canonical_angle(existing.rotation_deg),
                },
                mirrored: existing.mirrored,
                translate_x: existing.translate_short_axis,
                translate_y: existing.translate_long_axis,
            });
            continue;
        }
        let collision = piece.polygon.offset(collision_expansion_mm(settings))?;
        let bounds = collision.bounds().ok_or_else(|| {
            GeneralPolygonError::from_message("cannot initialize empty relaxed geometry")
        })?;
        let translate_x = snap_mm(inset - bounds.min_x);
        let translate_y = snap_mm(shelf_y - bounds.min_y);
        shelf_y += bounds.max_y - bounds.min_y + settings.total_padding_mm;
        placements.push(RelaxedPlacement {
            input_index,
            rotation_deg: 0.0,
            mirrored: false,
            translate_x,
            translate_y,
        });
    }
    Ok(RelaxedState {
        placements,
        strip_depth_mm: shelf_y.max(incumbent.used_long_axis_depth_mm),
    })
}

fn disrupt_state_legacy(
    mut state: RelaxedState,
    pieces: &[GeneralFastPiece<'_>],
    seed: u64,
) -> Result<RelaxedState, GeneralFastError> {
    if state.placements.len() < 2 {
        return Ok(state);
    }
    let mut ranked = state
        .placements
        .iter()
        .enumerate()
        .map(|(state_index, placement)| {
            let bounds = pieces[placement.input_index]
                .polygon
                .bounds()
                .ok_or_else(|| {
                    GeneralPolygonError::from_message("cannot disrupt empty geometry")
                })?;
            let width = bounds.max_x - bounds.min_x;
            let height = bounds.max_y - bounds.min_y;
            Ok::<_, GeneralFastError>((state_index, width * height, width.hypot(height)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    ranked.sort_by(|first, second| {
        second
            .1
            .total_cmp(&first.1)
            .then_with(|| second.2.total_cmp(&first.2))
            .then_with(|| first.0.cmp(&second.0))
    });
    let total_area = ranked.iter().map(|(_, area, _)| *area).sum::<f64>();
    let mut cumulative = 0.0;
    let mut large = Vec::new();
    for entry in ranked.iter().copied() {
        large.push(entry);
        cumulative += entry.1;
        if large.len() >= 2 && cumulative >= total_area * 0.75 {
            break;
        }
    }
    if large.len() < 2 {
        large = ranked;
    }

    let mut rng = SplitMix64::new(seed ^ 0x9FB2_1C65_1E98_DF25);
    let first_position = (rng.next_u64() as usize) % large.len();
    let first = large[first_position];
    let mut distinct = large
        .iter()
        .copied()
        .filter(|second| {
            second.0 != first.0
                && ((second.1 - first.1).abs() > first.1 * 0.01
                    || (second.2 - first.2).abs() > first.2 * 0.01)
        })
        .collect::<Vec<_>>();
    if distinct.is_empty() {
        distinct.extend(large.iter().copied().filter(|second| second.0 != first.0));
    }
    let second = distinct[(rng.next_u64() as usize) % distinct.len()];

    let first_old = state.placements[first.0].clone();
    let second_old = state.placements[second.0].clone();
    let first_piece = pieces[first_old.input_index];
    let second_piece = pieces[second_old.input_index];
    state.placements[first.0].translate_x = second_old.translate_x;
    state.placements[first.0].translate_y = second_old.translate_y;
    state.placements[first.0].rotation_deg = if first_piece.allow_rotation {
        second_old.rotation_deg
    } else {
        0.0
    };
    state.placements[first.0].mirrored = first_piece.allow_mirror && second_old.mirrored;
    state.placements[second.0].translate_x = first_old.translate_x;
    state.placements[second.0].translate_y = first_old.translate_y;
    state.placements[second.0].rotation_deg = if second_piece.allow_rotation {
        first_old.rotation_deg
    } else {
        0.0
    };
    state.placements[second.0].mirrored = second_piece.allow_mirror && first_old.mirrored;
    let mut moved = BTreeSet::new();
    relocate_contained_cluster(
        &mut state,
        pieces,
        first.0,
        second.0,
        first_old.translate_x - second_old.translate_x,
        first_old.translate_y - second_old.translate_y,
        &mut moved,
    )?;
    relocate_contained_cluster(
        &mut state,
        pieces,
        second.0,
        first.0,
        second_old.translate_x - first_old.translate_x,
        second_old.translate_y - first_old.translate_y,
        &mut moved,
    )?;
    Ok(state)
}

fn relocate_contained_cluster(
    state: &mut RelaxedState,
    pieces: &[GeneralFastPiece<'_>],
    container_index: usize,
    other_swapped_index: usize,
    delta_x: f64,
    delta_y: f64,
    already_moved: &mut BTreeSet<usize>,
) -> Result<(), GeneralFastError> {
    let container = &state.placements[container_index];
    let container_polygon = pieces[container.input_index].polygon.transformed(
        container.rotation_deg,
        container.mirrored,
        container.translate_x,
        container.translate_y,
    )?;
    let mut contained = Vec::new();
    for (index, placement) in state.placements.iter().enumerate() {
        if index == container_index
            || index == other_swapped_index
            || already_moved.contains(&index)
        {
            continue;
        }
        let polygon = pieces[placement.input_index].polygon.transformed(
            placement.rotation_deg,
            placement.mirrored,
            placement.translate_x,
            placement.translate_y,
        )?;
        let bounds = polygon.bounds().ok_or_else(|| {
            GeneralPolygonError::from_message("cannot relocate empty cluster geometry")
        })?;
        let point = IrregularPoint::new(
            (bounds.min_x + bounds.max_x) * 0.5,
            (bounds.min_y + bounds.max_y) * 0.5,
        );
        if container_polygon.contains_point(point) == PointInPolygonResult::IsInside {
            contained.push(index);
        }
    }
    for index in contained {
        state.placements[index].translate_x =
            snap_mm(state.placements[index].translate_x + delta_x);
        state.placements[index].translate_y =
            snap_mm(state.placements[index].translate_y + delta_y);
        already_moved.insert(index);
    }
    Ok(())
}

fn compression_split(seed: u64, strip_depth_mm: f64, settings: GeneralFastSettings) -> f64 {
    let inset = collision_sheet_inset_mm(settings);
    let mut rng = SplitMix64::new(seed ^ 0xD1B5_4A32_D192_ED03);
    rng.range(inset, (strip_depth_mm - inset).max(inset))
}

fn compress_state_at_split(
    state: &RelaxedState,
    target_depth_mm: f64,
    split_position_mm: f64,
    pieces: &[GeneralFastPiece<'_>],
) -> Result<RelaxedState, GeneralFastError> {
    let delta = (target_depth_mm - state.strip_depth_mm).min(0.0);
    let mut compressed = state.clone();
    compressed.strip_depth_mm = target_depth_mm;
    for placement in &mut compressed.placements {
        if !pieces[placement.input_index].allow_rotation {
            placement.rotation_deg = 0.0;
        }
        let bounds = pieces[placement.input_index]
            .polygon
            .transformed(placement.rotation_deg, placement.mirrored, 0.0, 0.0)?
            .bounds()
            .ok_or_else(|| {
                GeneralPolygonError::from_message(
                    "cannot compress relaxed state with empty source geometry",
                )
            })?;
        let centroid_y = placement.translate_y + (bounds.min_y + bounds.max_y) * 0.5;
        if centroid_y > split_position_mm {
            placement.translate_y = snap_mm(placement.translate_y + delta);
        }
    }
    Ok(compressed)
}

fn area_depth_lower_bound(
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
) -> Result<f64, GeneralFastError> {
    let expansion = collision_expansion_mm(settings);
    let area = pieces.iter().try_fold(0.0, |total, piece| {
        Ok::<_, GeneralFastError>(total + piece.polygon.offset(expansion)?.area_mm2())
    })?;
    Ok(area / collision_sheet_short_axis_mm(settings) + 2.0 * collision_sheet_inset_mm(settings))
}

fn to_fast_placements(
    state: &RelaxedState,
    pieces: &[GeneralFastPiece<'_>],
) -> Vec<GeneralFastPlacement> {
    state
        .placements
        .iter()
        .map(|placement| GeneralFastPlacement {
            piece_id: pieces[placement.input_index].id.to_owned(),
            rotation_deg: placement.rotation_deg,
            mirrored: placement.mirrored,
            translate_short_axis: placement.translate_x,
            translate_long_axis: placement.translate_y,
        })
        .collect()
}

fn triangulate_ring(points: &[IrregularPoint]) -> Result<Vec<Triangle>, GeneralFastError> {
    let mut indices = (0..points.len()).collect::<Vec<_>>();
    indices.retain(|index| {
        let previous = points[(*index + points.len() - 1) % points.len()];
        let current = points[*index];
        let next = points[(*index + 1) % points.len()];
        orientation(previous.x, previous.y, current.x, current.y, next.x, next.y) != 0
    });
    if indices.len() < 3 {
        return Err(GeneralPolygonError::from_message(
            "relaxed surrogate ring collapsed during triangulation",
        )
        .into());
    }
    let mut triangles = Vec::with_capacity(indices.len() - 2);
    while indices.len() > 3 {
        let mut ear = None;
        for position in 0..indices.len() {
            let previous = indices[(position + indices.len() - 1) % indices.len()];
            let current = indices[position];
            let next = indices[(position + 1) % indices.len()];
            let triangle_points = [points[previous], points[current], points[next]];
            if orientation(
                triangle_points[0].x,
                triangle_points[0].y,
                triangle_points[1].x,
                triangle_points[1].y,
                triangle_points[2].x,
                triangle_points[2].y,
            ) <= 0
            {
                continue;
            }
            if indices.iter().copied().any(|candidate| {
                candidate != previous
                    && candidate != current
                    && candidate != next
                    && point_in_triangle(points[candidate], triangle_points)
            }) {
                continue;
            }
            ear = Some((position, Triangle::new(triangle_points)));
            break;
        }
        let Some((position, triangle)) = ear else {
            return Err(GeneralPolygonError::from_message(
                "relaxed surrogate could not triangulate a canonical ring",
            )
            .into());
        };
        triangles.push(triangle);
        indices.remove(position);
    }
    triangles.push(Triangle::new([
        points[indices[0]],
        points[indices[1]],
        points[indices[2]],
    ]));
    Ok(triangles)
}

fn point_in_triangle(point: IrregularPoint, triangle: [IrregularPoint; 3]) -> bool {
    (0..3).all(|index| {
        let start = triangle[index];
        let end = triangle[(index + 1) % 3];
        orientation(start.x, start.y, end.x, end.y, point.x, point.y) >= 0
    })
}

fn triangle_penetration(
    first: Triangle,
    first_x: f64,
    first_y: f64,
    second: Triangle,
    second_x: f64,
    second_y: f64,
) -> Option<f64> {
    let first_points = first
        .points
        .map(|point| IrregularPoint::new(point.x + first_x, point.y + first_y));
    let second_points = second
        .points
        .map(|point| IrregularPoint::new(point.x + second_x, point.y + second_y));
    let mut minimum = f64::INFINITY;
    for polygon in [&first_points, &second_points] {
        for index in 0..3 {
            let edge_x = polygon[(index + 1) % 3].x - polygon[index].x;
            let edge_y = polygon[(index + 1) % 3].y - polygon[index].y;
            let length = edge_x.hypot(edge_y);
            if length == 0.0 {
                return None;
            }
            let axis_x = -edge_y / length;
            let axis_y = edge_x / length;
            let (first_min, first_max) = project_triangle(&first_points, axis_x, axis_y);
            let (second_min, second_max) = project_triangle(&second_points, axis_x, axis_y);
            let overlap = first_max.min(second_max) - first_min.max(second_min);
            if overlap <= 0.0 {
                return None;
            }
            minimum = minimum.min(overlap);
        }
    }
    Some(minimum)
}

fn triangles_overlap_on_grid(
    first: Triangle,
    second: Triangle,
    relative_x: i128,
    relative_y: i128,
) -> Option<bool> {
    let first_points = grid_triangle_points(first, 0, 0)?;
    let second_points = grid_triangle_points(second, relative_x, relative_y)?;
    for polygon in [&first_points, &second_points] {
        for index in 0..3 {
            let edge_x = polygon[(index + 1) % 3].0 - polygon[index].0;
            let edge_y = polygon[(index + 1) % 3].1 - polygon[index].1;
            if edge_x == 0 && edge_y == 0 {
                return Some(false);
            }
            let axis = (-edge_y, edge_x);
            let (first_min, first_max) = project_grid_triangle(&first_points, axis);
            let (second_min, second_max) = project_grid_triangle(&second_points, axis);
            if first_max.min(second_max) <= first_min.max(second_min) {
                return Some(false);
            }
        }
    }
    Some(true)
}

fn grid_triangle_points(
    triangle: Triangle,
    translate_x: i128,
    translate_y: i128,
) -> Option<[(i128, i128); 3]> {
    let [first, second, third] = triangle.points;
    Some([
        (
            grid_coordinate(first.x)?.checked_add(translate_x)?,
            grid_coordinate(first.y)?.checked_add(translate_y)?,
        ),
        (
            grid_coordinate(second.x)?.checked_add(translate_x)?,
            grid_coordinate(second.y)?.checked_add(translate_y)?,
        ),
        (
            grid_coordinate(third.x)?.checked_add(translate_x)?,
            grid_coordinate(third.y)?.checked_add(translate_y)?,
        ),
    ])
}

fn grid_coordinate(value: f64) -> Option<i128> {
    to_grid_mm(value).map(|value| value as i128)
}

fn relative_grid_coordinate(first: f64, second: f64) -> Option<i128> {
    grid_coordinate(second)?.checked_sub(grid_coordinate(first)?)
}

fn project_grid_triangle(points: &[(i128, i128); 3], axis: (i128, i128)) -> (i128, i128) {
    let first = points[0].0 * axis.0 + points[0].1 * axis.1;
    points[1..].iter().fold((first, first), |bounds, point| {
        let projection = point.0 * axis.0 + point.1 * axis.1;
        (bounds.0.min(projection), bounds.1.max(projection))
    })
}

fn triangle_pole(triangle: Triangle) -> Pole {
    let [first, second, third] = triangle.points;
    let opposite_first = (second.x - third.x).hypot(second.y - third.y);
    let opposite_second = (first.x - third.x).hypot(first.y - third.y);
    let opposite_third = (first.x - second.x).hypot(first.y - second.y);
    let perimeter = opposite_first + opposite_second + opposite_third;
    let center = IrregularPoint::new(
        (opposite_first * first.x + opposite_second * second.x + opposite_third * third.x)
            / perimeter,
        (opposite_first * first.y + opposite_second * second.y + opposite_third * third.y)
            / perimeter,
    );
    let doubled_area = ((second.x - first.x) * (third.y - first.y)
        - (second.y - first.y) * (third.x - first.x))
        .abs();
    Pole {
        center,
        radius: doubled_area / perimeter,
    }
}

fn project_triangle(points: &[IrregularPoint; 3], axis_x: f64, axis_y: f64) -> (f64, f64) {
    points
        .iter()
        .map(|point| point.x * axis_x + point.y * axis_y)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), value| {
            (min.min(value), max.max(value))
        })
}

fn compare_lane_outcomes(
    first_ordinal: usize,
    first: &LaneOutcome,
    second_ordinal: usize,
    second: &LaneOutcome,
) -> Ordering {
    first
        .score
        .common_loss()
        .total_cmp(&second.score.common_loss())
        .then_with(|| {
            first
                .score
                .boundary_loss
                .total_cmp(&second.score.boundary_loss)
        })
        .then_with(|| {
            first
                .score
                .boundary_violations
                .cmp(&second.score.boundary_violations)
        })
        .then_with(|| {
            first
                .score
                .collision_pairs
                .len()
                .cmp(&second.score.collision_pairs.len())
        })
        .then_with(|| canonical_state_key(&first.state).cmp(&canonical_state_key(&second.state)))
        .then_with(|| first_ordinal.cmp(&second_ordinal))
}

fn compare_chain_score(first: &PairTracker, second: &PairTracker) -> Ordering {
    first
        .common_loss()
        .total_cmp(&second.common_loss())
        .then_with(|| first.weighted_loss.total_cmp(&second.weighted_loss))
        .then_with(|| first.boundary_violations.cmp(&second.boundary_violations))
        .then_with(|| {
            first
                .collision_pairs
                .len()
                .cmp(&second.collision_pairs.len())
        })
}

fn compare_ejection_candidates(first: &EjectionCandidate, second: &EjectionCandidate) -> Ordering {
    compare_chain_score(&first.score, &second.score)
        .then_with(|| ejection_candidate_key(first).cmp(&ejection_candidate_key(second)))
}

fn ejection_candidate_key(candidate: &EjectionCandidate) -> Vec<(usize, i64, bool, i64, i64)> {
    candidate
        .replacements
        .iter()
        .map(|(_, placement)| placement_key(placement))
        .collect()
}

fn same_piece_geometry(first: GeneralFastPiece<'_>, second: GeneralFastPiece<'_>) -> bool {
    let first_bounds = first
        .polygon
        .bounds()
        .expect("general pieces are non-empty");
    let second_bounds = second
        .polygon
        .bounds()
        .expect("general pieces are non-empty");
    let first_dimensions = [
        first_bounds.max_x - first_bounds.min_x,
        first_bounds.max_y - first_bounds.min_y,
    ];
    let second_dimensions = [
        second_bounds.max_x - second_bounds.min_x,
        second_bounds.max_y - second_bounds.min_y,
    ];
    let area_scale = first
        .polygon
        .area_mm2()
        .max(second.polygon.area_mm2())
        .max(1.0);
    (first.polygon.area_mm2() - second.polygon.area_mm2()).abs() <= area_scale * 0.001
        && (first_dimensions[0] - second_dimensions[0]).abs() <= 0.001
        && (first_dimensions[1] - second_dimensions[1]).abs() <= 0.001
}

fn update_score_after_move(
    score: &mut PairTracker,
    input_index: usize,
    old_boundary: (usize, f64),
    replacement: PlacementScore,
    weights: &BTreeMap<(usize, usize), f64>,
) {
    let tracked_boundary = score.boundaries[input_index];
    debug_assert_eq!(tracked_boundary.violations, old_boundary.0);
    debug_assert!((tracked_boundary.raw_loss - old_boundary.1).abs() <= f64::EPSILON);
    score.replace_boundary(
        input_index,
        BoundaryEntry {
            violations: replacement.boundary_violations,
            raw_loss: replacement.boundary_loss,
        },
    );
    for fixed in 0..score.piece_count {
        if fixed == input_index {
            continue;
        }
        let pair = ordered_pair(input_index, fixed);
        let raw_loss = replacement
            .collision_pairs
            .iter()
            .find(|(first, second, _)| (*first, *second) == pair)
            .map(|(_, _, penalty)| *penalty)
            .unwrap_or(0.0);
        let guided_weight = weights.get(&pair).copied().unwrap_or(1.0);
        score.replace_pair(pair.0, pair.1, raw_loss, guided_weight);
    }
    score
        .collision_pairs
        .retain(|(first, second, _)| *first != input_index && *second != input_index);
    score.boundary_violations = score
        .boundary_violations
        .saturating_sub(old_boundary.0)
        .saturating_add(replacement.boundary_violations);
    score.boundary_loss =
        (score.boundary_loss - old_boundary.1 + replacement.boundary_loss).max(0.0);
    score.collision_pairs.extend(replacement.collision_pairs);
    score
        .collision_pairs
        .sort_by_key(|(first, second, _)| (*first, *second));
    score.weighted_loss = score.boundary_loss
        + score
            .collision_pairs
            .iter()
            .map(|(first, second, penalty)| {
                weights
                    .get(&ordered_pair(*first, *second))
                    .copied()
                    .unwrap_or(1.0)
                    * *penalty
            })
            .sum::<f64>();
}

fn tracked_piece_score(
    score: &PairTracker,
    input_index: usize,
    weights: &BTreeMap<(usize, usize), f64>,
) -> PlacementScore {
    let boundary = score.boundaries[input_index];
    let collision_pairs = score
        .collision_pairs
        .iter()
        .filter(|(first, second, _)| *first == input_index || *second == input_index)
        .copied()
        .collect::<Vec<_>>();
    let weighted_loss = boundary.raw_loss
        + collision_pairs
            .iter()
            .map(|(first, second, penalty)| {
                weights
                    .get(&ordered_pair(*first, *second))
                    .copied()
                    .unwrap_or(1.0)
                    * *penalty
            })
            .sum::<f64>();
    PlacementScore {
        boundary_violations: boundary.violations,
        boundary_loss: boundary.raw_loss,
        collision_pairs,
        weighted_loss,
    }
}

fn refresh_weighted_loss(score: &mut PairTracker, weights: &BTreeMap<(usize, usize), f64>) {
    for first in 0..score.piece_count {
        for second in (first + 1)..score.piece_count {
            let slot = pair_slot(score.piece_count, first, second);
            score.pairs[slot].guided_weight = weights.get(&(first, second)).copied().unwrap_or(1.0);
        }
    }
    score.weighted_loss = score.boundary_loss
        + score
            .collision_pairs
            .iter()
            .map(|(first, second, penalty)| {
                weights
                    .get(&ordered_pair(*first, *second))
                    .copied()
                    .unwrap_or(1.0)
                    * *penalty
            })
            .sum::<f64>();
}

fn compare_move_score(
    first_score: &PlacementScore,
    first: &RelaxedPlacement,
    second_score: &PlacementScore,
    second: &RelaxedPlacement,
) -> Ordering {
    first_score
        .weighted_loss
        .total_cmp(&second_score.weighted_loss)
        .then_with(|| {
            first_score
                .boundary_violations
                .cmp(&second_score.boundary_violations)
        })
        .then_with(|| {
            first_score
                .collision_pairs
                .len()
                .cmp(&second_score.collision_pairs.len())
        })
        .then_with(|| move_tie_key(first).cmp(&move_tie_key(second)))
}

fn compare_score_objective(first: &PlacementScore, second: &PlacementScore) -> Ordering {
    first
        .weighted_loss
        .total_cmp(&second.weighted_loss)
        .then_with(|| first.boundary_violations.cmp(&second.boundary_violations))
        .then_with(|| {
            first
                .collision_pairs
                .len()
                .cmp(&second.collision_pairs.len())
        })
}

fn unscorable_directional_score(
    input_index: usize,
    boundary_violations: usize,
    boundary_loss: f64,
    colliding: &[(usize, PairNfpKey, IrregularPoint)],
) -> PlacementScore {
    let mut collision_pairs = colliding
        .iter()
        .map(|(fixed_index, _, _)| {
            let pair = ordered_pair(input_index, *fixed_index);
            (pair.0, pair.1, 1.0)
        })
        .collect::<Vec<_>>();
    collision_pairs.sort_by_key(|(first, second, _)| (*first, *second));
    PlacementScore {
        boundary_violations,
        boundary_loss,
        collision_pairs,
        weighted_loss: f64::INFINITY,
    }
}

fn coordinate_offsets(
    axis: CoordinateAxis,
    step_x: f64,
    step_y: f64,
    rotation_step_deg: f64,
) -> [(f64, f64, f64); 2] {
    match axis {
        CoordinateAxis::Horizontal => [(step_x, 0.0, 0.0), (-step_x, 0.0, 0.0)],
        CoordinateAxis::Vertical => [(0.0, step_y, 0.0), (0.0, -step_y, 0.0)],
        CoordinateAxis::ForwardDiagonal => [(step_x, step_y, 0.0), (-step_x, -step_y, 0.0)],
        CoordinateAxis::BackwardDiagonal => [(-step_x, step_y, 0.0), (step_x, -step_y, 0.0)],
        CoordinateAxis::Rotation => [
            (0.0, 0.0, rotation_step_deg),
            (0.0, 0.0, -rotation_step_deg),
        ],
    }
}

fn convex_line_interval(
    points: &[IrregularPoint],
    origin: IrregularPoint,
    direction: (f64, f64),
) -> Option<(f64, f64)> {
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for index in 0..points.len() {
        let start = points[index];
        let end = points[(index + 1) % points.len()];
        let edge = (end.x - start.x, end.y - start.y);
        let start_from_origin = (start.x - origin.x, start.y - origin.y);
        let denominator = cross_vectors(direction, edge);
        if denominator == 0.0 {
            if cross_vectors(start_from_origin, direction) == 0.0 {
                let first = dot_vectors(start_from_origin, direction);
                let second = dot_vectors((end.x - origin.x, end.y - origin.y), direction);
                minimum = minimum.min(first.min(second));
                maximum = maximum.max(first.max(second));
            }
            continue;
        }
        let segment_position = cross_vectors(start_from_origin, direction) / denominator;
        if !(0.0..=1.0).contains(&segment_position) {
            continue;
        }
        let line_position = cross_vectors(start_from_origin, edge) / denominator;
        minimum = minimum.min(line_position);
        maximum = maximum.max(line_position);
    }
    (minimum.is_finite() && maximum.is_finite()).then_some((minimum, maximum))
}

fn cross_vectors(first: (f64, f64), second: (f64, f64)) -> f64 {
    first.0 * second.1 - first.1 * second.0
}

fn dot_vectors(first: (f64, f64), second: (f64, f64)) -> f64 {
    first.0 * second.0 + first.1 * second.1
}

fn merge_intervals(intervals: &mut Vec<(f64, f64)>) {
    intervals.sort_by(|first, second| {
        first
            .0
            .total_cmp(&second.0)
            .then_with(|| first.1.total_cmp(&second.1))
    });
    let mut write = 0usize;
    for read in 0..intervals.len() {
        let current = intervals[read];
        if write > 0 && current.0 <= intervals[write - 1].1 {
            intervals[write - 1].1 = intervals[write - 1].1.max(current.1);
        } else {
            intervals[write] = current;
            write += 1;
        }
    }
    intervals.truncate(write);
}

fn interval_penetration(value: f64, intervals: &[(f64, f64)]) -> f64 {
    for (start, end) in intervals.iter().copied() {
        if value < start {
            break;
        }
        if value <= end {
            return (value - start).min(end - value).max(0.0);
        }
    }
    0.0
}

fn compare_axis_candidate(
    first: &(f64, f64),
    second: &(f64, f64),
    current: f64,
    prefer_distance: bool,
) -> Ordering {
    let by_loss = first.1.total_cmp(&second.1);
    let by_distance = (first.0 - current)
        .abs()
        .total_cmp(&(second.0 - current).abs());
    let by_coordinate = first.0.total_cmp(&second.0);
    if prefer_distance {
        by_loss.then(by_distance).then(by_coordinate)
    } else {
        by_loss.then(by_coordinate).then(by_distance)
    }
}

fn merge_grid_intervals(intervals: &mut Vec<(i64, i64)>) {
    for interval in intervals.iter_mut() {
        if interval.0 > interval.1 {
            *interval = (interval.1, interval.0);
        }
    }
    intervals.sort_unstable();
    let mut write = 0usize;
    for read in 0..intervals.len() {
        let current = intervals[read];
        if write > 0 && current.0 <= intervals[write - 1].1 {
            intervals[write - 1].1 = intervals[write - 1].1.max(current.1);
        } else {
            intervals[write] = current;
            write += 1;
        }
    }
    intervals.truncate(write);
}

fn grid_interval_penetration(value: i64, intervals: &[(i64, i64)]) -> i64 {
    for (start, end) in intervals.iter().copied() {
        if value < start {
            break;
        }
        if value <= end {
            return (value - start).min(end - value).max(0);
        }
    }
    0
}

fn grid_neighbors_clamped(value: f64, minimum: f64, maximum: f64) -> Vec<f64> {
    let value = value.clamp(minimum, maximum);
    let scaled = value * 1_000.0;
    [scaled.floor(), scaled.ceil()]
        .into_iter()
        .map(from_grid)
        .map(|value| value.clamp(minimum, maximum))
        .collect()
}

fn grid_predecessor_clamped(value: f64, minimum: f64, maximum: f64) -> f64 {
    from_grid((grid_lower_bound_key(value) - 1) as f64).clamp(minimum, maximum)
}

fn grid_successor_clamped(value: f64, minimum: f64, maximum: f64) -> f64 {
    from_grid((grid_upper_bound_key(value) + 1) as f64).clamp(minimum, maximum)
}

fn apply_coordinate_multiplier(
    axis: CoordinateAxis,
    step_x: &mut f64,
    step_y: &mut f64,
    rotation_step_deg: &mut f64,
    multiplier: f64,
) {
    match axis {
        CoordinateAxis::Horizontal => *step_x *= multiplier,
        CoordinateAxis::Vertical => *step_y *= multiplier,
        CoordinateAxis::ForwardDiagonal | CoordinateAxis::BackwardDiagonal => {
            let diagonal_multiplier = multiplier.sqrt();
            *step_x *= diagonal_multiplier;
            *step_y *= diagonal_multiplier;
        }
        CoordinateAxis::Rotation => *rotation_step_deg *= multiplier,
    }
}

fn even_floor(value: usize) -> usize {
    value - value % 2
}

fn move_tie_key(placement: &RelaxedPlacement) -> (i64, bool, i64, i64) {
    (
        angle_key(placement.rotation_deg),
        placement.mirrored,
        grid_key(placement.translate_x),
        grid_key(placement.translate_y),
    )
}

fn blocking_pair_diagnostics(
    pieces: &[GeneralFastPiece<'_>],
    score: &PairTracker,
    weights: &BTreeMap<(usize, usize), f64>,
) -> Vec<GeneralRelaxedPairDiagnostics> {
    let mut pairs = score
        .collision_pairs
        .iter()
        .map(|(first, second, penalty)| {
            let guided_weight = weights
                .get(&ordered_pair(*first, *second))
                .copied()
                .unwrap_or(1.0);
            GeneralRelaxedPairDiagnostics {
                first_piece_id: pieces[*first].id.to_owned(),
                second_piece_id: pieces[*second].id.to_owned(),
                raw_penalty: *penalty,
                guided_weight,
                weighted_pressure: *penalty * guided_weight,
            }
        })
        .collect::<Vec<_>>();
    pairs.sort_by(|first, second| {
        second
            .weighted_pressure
            .total_cmp(&first.weighted_pressure)
            .then_with(|| first.first_piece_id.cmp(&second.first_piece_id))
            .then_with(|| first.second_piece_id.cmp(&second.second_piece_id))
    });
    pairs.truncate(8);
    pairs
}

fn canonical_state_key(state: &RelaxedState) -> Vec<(usize, i64, bool, i64, i64)> {
    state.placements.iter().map(placement_key).collect()
}

fn report_diverse_sample(
    samples: &mut Vec<(RelaxedPlacement, PlacementScore)>,
    candidate: RelaxedPlacement,
    score: PlacementScore,
    position_threshold: f64,
) {
    let mut similar = [0usize; LOCAL_DESCENT_STARTS];
    let mut similar_count = 0usize;
    for (index, (placement, _)) in samples.iter().enumerate() {
        if placements_are_similar(placement, &candidate, position_threshold) {
            similar[similar_count] = index;
            similar_count += 1;
        }
    }
    if similar_count > 0
        && similar[..similar_count].iter().any(|index| {
            compare_move_score(&samples[*index].1, &samples[*index].0, &score, &candidate)
                != Ordering::Greater
        })
    {
        return;
    }
    for index in similar[..similar_count].iter().copied().rev() {
        samples.remove(index);
    }
    samples.push((candidate, score));
    samples.sort_by(|(first, first_score), (second, second_score)| {
        compare_move_score(first_score, first, second_score, second)
    });
    samples.truncate(LOCAL_DESCENT_STARTS);
}

fn sample_upper_bound(samples: &[(RelaxedPlacement, PlacementScore)]) -> Option<f64> {
    (samples.len() >= LOCAL_DESCENT_STARTS).then(|| {
        samples
            .last()
            .expect("sample capacity is non-empty")
            .1
            .weighted_loss
    })
}

fn placements_are_similar(
    first: &RelaxedPlacement,
    second: &RelaxedPlacement,
    position_threshold: f64,
) -> bool {
    (first.translate_x - second.translate_x).abs() < position_threshold
        && (first.translate_y - second.translate_y).abs() < position_threshold
        && angle_distance_deg(first.rotation_deg, second.rotation_deg) < UNIQUE_SAMPLE_ANGLE_DEG
        && first.mirrored == second.mirrored
}

fn angle_distance_deg(first: f64, second: f64) -> f64 {
    let difference = (first - second).rem_euclid(360.0);
    difference.min(360.0 - difference)
}

fn sample_or_center(rng: &mut SplitMix64, minimum: f64, maximum: f64) -> f64 {
    if minimum <= maximum {
        rng.range(minimum, maximum)
    } else {
        (minimum + maximum) * 0.5
    }
}

fn clamp_or_center(value: f64, minimum: f64, maximum: f64) -> f64 {
    if minimum <= maximum {
        value.clamp(minimum, maximum)
    } else {
        (minimum + maximum) * 0.5
    }
}

fn placement_key(placement: &RelaxedPlacement) -> (usize, i64, bool, i64, i64) {
    (
        placement.input_index,
        angle_key(placement.rotation_deg),
        placement.mirrored,
        grid_key(placement.translate_x),
        grid_key(placement.translate_y),
    )
}

fn update_weights(weights: &mut BTreeMap<(usize, usize), f64>, collisions: &[(usize, usize, f64)]) {
    let maximum = collisions
        .iter()
        .map(|(_, _, penalty)| *penalty)
        .max_by(f64::total_cmp)
        .unwrap_or(0.0);
    let active = collisions
        .iter()
        .map(|(first, second, _)| ordered_pair(*first, *second))
        .collect::<BTreeSet<_>>();
    for (pair, weight) in weights.iter_mut() {
        if !active.contains(pair) {
            *weight = (*weight * 0.95).max(1.0);
        }
    }
    if maximum > 0.0 {
        for (first, second, penalty) in collisions {
            let multiplier = 1.02 + 0.08 * (*penalty / maximum);
            *weights.entry(ordered_pair(*first, *second)).or_insert(1.0) *= multiplier;
        }
    }
}

fn repair_active_indices(
    state: &RelaxedState,
    score: &PairTracker,
    pieces: &[GeneralFastPiece<'_>],
    weights: &BTreeMap<(usize, usize), f64>,
    neighborhood_size: usize,
) -> Vec<usize> {
    let neighborhood_size = neighborhood_size.clamp(2, pieces.len());
    let mut selected = Vec::with_capacity(neighborhood_size);
    if let Some((first, second, _)) = score.collision_pairs.iter().max_by(|first, second| {
        let first_weighted = first.2
            * weights
                .get(&ordered_pair(first.0, first.1))
                .copied()
                .unwrap_or(1.0);
        let second_weighted = second.2
            * weights
                .get(&ordered_pair(second.0, second.1))
                .copied()
                .unwrap_or(1.0);
        first_weighted
            .total_cmp(&second_weighted)
            .then_with(|| ordered_pair(second.0, second.1).cmp(&ordered_pair(first.0, first.1)))
    }) {
        selected.push(*first);
        selected.push(*second);
    }
    for blocker in high_frontier_blockers(state, pieces, neighborhood_size) {
        if selected.len() >= neighborhood_size {
            break;
        }
        if !selected.contains(&blocker) {
            selected.push(blocker);
        }
    }
    selected.sort_by(|first, second| {
        pieces[*second]
            .polygon
            .area_mm2()
            .total_cmp(&pieces[*first].polygon.area_mm2())
            .then_with(|| pieces[*first].id.cmp(pieces[*second].id))
    });
    selected
}

fn repair_angles(piece: GeneralFastPiece<'_>, current: &RelaxedPlacement) -> Vec<f64> {
    if !piece.allow_rotation {
        return vec![current.rotation_deg];
    }
    let mut edges = piece
        .polygon
        .regions()
        .iter()
        .flat_map(|region| {
            let points = region.outer.points();
            (0..points.len()).map(move |index| {
                let mut start = points[index];
                let mut end = points[(index + 1) % points.len()];
                if current.mirrored {
                    start.x = -start.x;
                    end.x = -end.x;
                }
                let delta_x = end.x - start.x;
                let delta_y = end.y - start.y;
                (delta_x.hypot(delta_y), delta_y.atan2(delta_x).to_degrees())
            })
        })
        .filter(|(length, _)| *length > 0.001)
        .collect::<Vec<_>>();
    edges.sort_by(|first, second| {
        second
            .0
            .total_cmp(&first.0)
            .then_with(|| first.1.total_cmp(&second.1))
    });
    let mut directions = Vec::with_capacity(2);
    for (_, direction) in edges {
        if directions.iter().all(|selected: &f64| {
            let difference = (direction - *selected).to_radians().sin().abs();
            difference > 0.1
        }) {
            directions.push(direction);
        }
        if directions.len() == 2 {
            break;
        }
    }
    let mut angles = vec![continuous_angle(current.rotation_deg)];
    for direction in directions {
        angles.push(continuous_angle(-direction));
        angles.push(continuous_angle(90.0 - direction));
    }
    angles.sort_by_key(|angle| angle_key(*angle));
    angles.dedup_by_key(|angle| angle_key(*angle));
    if let Some(position) = angles
        .iter()
        .position(|angle| angle_key(*angle) == angle_key(current.rotation_deg))
    {
        angles.swap(0, position);
    }
    angles
}

fn high_frontier_blockers(
    state: &RelaxedState,
    pieces: &[GeneralFastPiece<'_>],
    count: usize,
) -> Vec<usize> {
    let mut ranked = state
        .placements
        .iter()
        .map(|placement| {
            (
                placement.input_index,
                transformed_source_max_y(pieces[placement.input_index], placement),
                pieces[placement.input_index].id,
            )
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|first, second| {
        second
            .1
            .total_cmp(&first.1)
            .then_with(|| first.2.cmp(second.2))
    });
    ranked
        .into_iter()
        .take(count)
        .map(|(index, _, _)| index)
        .collect()
}

fn legacy_forced_blockers(
    state: &RelaxedState,
    pieces: &[GeneralFastPiece<'_>],
    count: usize,
) -> Vec<usize> {
    let mut ranked = state
        .placements
        .iter()
        .map(|placement| {
            let bounds = pieces[placement.input_index]
                .polygon
                .bounds()
                .expect("general pieces are non-empty");
            (
                placement.input_index,
                placement.translate_y + bounds.max_y,
                pieces[placement.input_index].id,
            )
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|first, second| {
        second
            .1
            .total_cmp(&first.1)
            .then_with(|| first.2.cmp(second.2))
    });
    ranked
        .into_iter()
        .take(count)
        .map(|(index, _, _)| index)
        .collect()
}

fn transformed_source_max_y(piece: GeneralFastPiece<'_>, placement: &RelaxedPlacement) -> f64 {
    let radians = placement.rotation_deg.to_radians();
    let sine = radians.sin();
    let cosine = radians.cos();
    piece
        .polygon
        .regions()
        .iter()
        .flat_map(|region| std::iter::once(&region.outer).chain(region.holes.iter()))
        .flat_map(|ring| ring.points())
        .map(|point| {
            let local_x = if placement.mirrored {
                -point.x
            } else {
                point.x
            };
            placement.translate_y + local_x * sine + point.y * cosine
        })
        .max_by(f64::total_cmp)
        .expect("general pieces are non-empty")
}

fn validate_relaxed_settings(settings: GeneralRelaxedSettings) -> Result<(), GeneralFastError> {
    if settings.epochs == 0
        || settings.lanes == 0
        || settings.sweeps_per_epoch == 0
        || settings.global_samples_per_move == 0
        || settings.focused_samples_per_move == 0
        || settings.refinement_rounds == 0
        || !settings.initial_shrink_ratio.is_finite()
        || !settings.minimum_shrink_ratio.is_finite()
        || settings.initial_shrink_ratio <= 0.0
        || settings.initial_shrink_ratio >= 1.0
        || settings.minimum_shrink_ratio <= 0.0
        || settings.minimum_shrink_ratio > settings.initial_shrink_ratio
    {
        return Err(GeneralFastError::InvalidSettings(
            "relaxed-search quotas and shrink ratios must be positive and bounded".to_owned(),
        ));
    }
    if settings.collision_backend == GeneralRelaxedCollisionBackend::RollbackTriangle
        && !matches!(
            settings.pressure_model,
            GeneralRelaxedPressureModel::StructuredTrianglePoles
                | GeneralRelaxedPressureModel::DirectionalPenetration
        )
    {
        return Err(GeneralFastError::InvalidSettings(
            "the rollback triangle backend requires structured or directional pressure".to_owned(),
        ));
    }
    if settings.pressure_model == GeneralRelaxedPressureModel::DirectionalPenetration
        && settings.synchronize_lanes
    {
        return Err(GeneralFastError::InvalidSettings(
            "directional penetration does not support synchronized lanes".to_owned(),
        ));
    }
    if settings.collision_backend == GeneralRelaxedCollisionBackend::DynamicHazard
        && settings.pressure_model == GeneralRelaxedPressureModel::DirectionalPenetration
    {
        return Err(GeneralFastError::InvalidSettings(
            "directional penetration requires the rollback triangle backend".to_owned(),
        ));
    }
    #[cfg(not(feature = "jagua-experimental"))]
    if settings.collision_backend == GeneralRelaxedCollisionBackend::DynamicHazard {
        return Err(GeneralFastError::InvalidSettings(
            "dynamic hazard search requires the jagua-experimental feature".to_owned(),
        ));
    }
    Ok(())
}

fn cell_bin_range(
    cell_bounds: IrregularBounds,
    shape_bounds: IrregularBounds,
) -> (usize, usize, usize, usize) {
    bin_range(cell_bounds, shape_bounds, CELL_INDEX_SIDE)
}

fn bin_range(
    cell_bounds: IrregularBounds,
    shape_bounds: IrregularBounds,
    side: usize,
) -> (usize, usize, usize, usize) {
    let width = (shape_bounds.max_x - shape_bounds.min_x).max(0.001);
    let height = (shape_bounds.max_y - shape_bounds.min_y).max(0.001);
    let bin = |value: f64, min: f64, span: f64| {
        (((value - min) / span) * side as f64)
            .floor()
            .clamp(0.0, (side - 1) as f64) as usize
    };
    (
        bin(cell_bounds.min_x, shape_bounds.min_x, width),
        bin(cell_bounds.max_x, shape_bounds.min_x, width),
        bin(cell_bounds.min_y, shape_bounds.min_y, height),
        bin(cell_bounds.max_y, shape_bounds.min_y, height),
    )
}

fn translated_bounds(bounds: IrregularBounds, x: f64, y: f64) -> IrregularBounds {
    IrregularBounds::new(
        bounds.min_x + x,
        bounds.min_y + y,
        bounds.max_x + x,
        bounds.max_y + y,
    )
}

fn bounds_overlap(first: IrregularBounds, second: IrregularBounds) -> bool {
    first.min_x < second.max_x
        && first.max_x > second.min_x
        && first.min_y < second.max_y
        && first.max_y > second.min_y
}

fn point_angle_key(angle_deg: f64) -> i64 {
    (angle_deg.rem_euclid(360.0) * ANGLE_KEY_SCALE).round() as i64
}

fn angle_key(angle_deg: f64) -> i64 {
    point_angle_key(angle_deg)
}

fn angle_from_key(key: i64) -> f64 {
    key as f64 / ANGLE_KEY_SCALE
}

fn canonical_angle(angle_deg: f64) -> f64 {
    let normalized = angle_deg.rem_euclid(360.0);
    angle_from_key(angle_key(
        (normalized / SURROGATE_ANGLE_STEP_DEG).round() * SURROGATE_ANGLE_STEP_DEG,
    ))
}

fn continuous_angle(angle_deg: f64) -> f64 {
    angle_from_key(angle_key(angle_deg.rem_euclid(360.0)))
}

#[cfg(feature = "jagua-experimental")]
fn hazard_pose(placement: &RelaxedPlacement) -> GeneralHazardPose {
    GeneralHazardPose {
        rotation_deg: continuous_angle(placement.rotation_deg),
        mirrored: placement.mirrored,
        translate_short_axis: placement.translate_x,
        translate_long_axis: placement.translate_y,
    }
}

#[cfg(feature = "jagua-experimental")]
fn dynamic_hazard_error(
    error: crate::search::general_hazard::GeneralHazardError,
) -> GeneralFastError {
    GeneralFastError::InvalidInput(format!("dynamic hazard backend: {error}"))
}

fn directional_lane_unscorable_error(reason: &str) -> GeneralFastError {
    GeneralFastError::InvalidSettings(format!("{DIRECTIONAL_LANE_UNSCORABLE}: {reason}"))
}

fn is_directional_lane_unscorable(error: &GeneralFastError) -> bool {
    error.to_string().contains(DIRECTIONAL_LANE_UNSCORABLE)
}

fn snap_mm(value: f64) -> f64 {
    to_grid_mm(value).map(from_grid).unwrap_or(value)
}

fn grid_key(value: f64) -> i64 {
    to_grid_mm(value)
        .map(|value| value as i64)
        .unwrap_or(i64::MAX)
}

fn grid_lower_bound_key(value: f64) -> i64 {
    (value * 1_000.0).floor() as i64
}

fn grid_upper_bound_key(value: f64) -> i64 {
    (value * 1_000.0).ceil() as i64
}

fn grid_interval_bounds(interval: (f64, f64)) -> (i64, i64) {
    let start = interval.0.min(interval.1);
    let end = interval.0.max(interval.1);
    (grid_lower_bound_key(start), grid_upper_bound_key(end))
}

fn ordered_pair(first: usize, second: usize) -> (usize, usize) {
    if first < second {
        (first, second)
    } else {
        (second, first)
    }
}

fn pair_slot(piece_count: usize, first: usize, second: usize) -> usize {
    let (first, second) = ordered_pair(first, second);
    debug_assert!(first < second);
    first * (2 * piece_count - first - 1) / 2 + second - first - 1
}

fn derive_seed(seed: u64, epoch: usize, lane: usize) -> u64 {
    seed ^ (epoch as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (lane as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
}

fn sample_grid_coordinate_with_rng(
    rng: &mut SplitMix64,
    minimum: i128,
    maximum: i128,
) -> Result<i128, GeneralFastError> {
    let span = maximum
        .checked_sub(minimum)
        .and_then(|value| value.checked_add(1))
        .and_then(|value| u64::try_from(value).ok())
        .ok_or_else(|| {
            GeneralFastError::InvalidInput(
                "directional inner-fit interval is outside the sampling domain".to_owned(),
            )
        })?;
    Ok(minimum + i128::from(rng.next_u64() % span))
}

fn shuffle(values: &mut [usize], rng: &mut SplitMix64) {
    for index in (1..values.len()).rev() {
        let swap = (rng.next_u64() as usize) % (index + 1);
        values.swap(index, swap);
    }
}

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / ((1_u64 << 53) as f64))
    }

    fn range(&mut self, min: f64, max: f64) -> f64 {
        if min >= max {
            return min;
        }
        min + (max - min) * self.unit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "jagua-experimental")]
    use crate::geometry::general_polygon::PolygonRegion;
    use crate::geometry::general_polygon::PolygonSet;
    #[cfg(feature = "jagua-experimental")]
    use crate::parallel::JobPool;
    #[cfg(feature = "jagua-experimental")]
    use crate::search::general_fast::construct_short_side_first;

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

    fn square(size: f64) -> PolygonSet {
        PolygonSet::from_outer(vec![
            point(0.0, 0.0),
            point(size, 0.0),
            point(size, size),
            point(0.0, size),
        ])
        .unwrap()
    }

    #[cfg(feature = "jagua-experimental")]
    fn holed_square() -> PolygonSet {
        PolygonSet::new(vec![PolygonRegion::new(
            vec![
                point(0.0, 0.0),
                point(40.0, 0.0),
                point(40.0, 40.0),
                point(0.0, 40.0),
            ],
            vec![vec![
                point(10.0, 10.0),
                point(10.0, 30.0),
                point(30.0, 30.0),
                point(30.0, 10.0),
            ]],
        )
        .unwrap()])
        .unwrap()
    }

    fn feasible_tracker(piece_count: usize) -> PairTracker {
        PairTracker {
            piece_count,
            boundaries: vec![
                BoundaryEntry {
                    violations: 0,
                    raw_loss: 0.0,
                };
                piece_count
            ],
            pairs: vec![
                PairEntry {
                    raw_loss: 0.0,
                    guided_weight: 1.0,
                    normalization_scale: 1.0,
                };
                piece_count.saturating_mul(piece_count.saturating_sub(1)) / 2
            ],
            incident_raw_loss: vec![0.0; piece_count],
            boundary_violations: 0,
            boundary_loss: 0.0,
            collision_pairs: Vec::new(),
            weighted_loss: 0.0,
        }
    }

    fn feasible_lane(lane: usize, translate_x: f64, translate_y: f64) -> LaneOutcome {
        LaneOutcome {
            state: RelaxedState {
                placements: vec![RelaxedPlacement {
                    input_index: 0,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x,
                    translate_y,
                }],
                strip_depth_mm: translate_y + 10.0,
            },
            score: feasible_tracker(1),
            weights: BTreeMap::new(),
            counters: WorkCounters::default(),
            selected_lane: lane,
            restart_disruptions: lane_disruption_count(lane),
        }
    }

    #[cfg(feature = "jagua-experimental")]
    fn coupled_test_settings(seed: u64) -> GeneralRelaxedSettings {
        let mut settings =
            GeneralRelaxedSettings::mixed_61_dynamic_hazard_probe(seed, COUPLED_SEPARATOR_WORKERS);
        settings.sweeps_per_epoch = COUPLED_SEPARATOR_ROUNDS;
        settings.global_samples_per_move = 10;
        settings.focused_samples_per_move = 10;
        settings.refinement_rounds = 5;
        settings.angle_seed_policy = GeneralRelaxedAngleSeedPolicy::ContinuousUniform;
        settings.pressure_model = GeneralRelaxedPressureModel::DynamicPoles;
        settings.synchronize_lanes = true;
        settings
    }

    #[cfg(feature = "jagua-experimental")]
    fn coupled_experiment_test_settings(seed: u64) -> GeneralRelaxedSettings {
        let mut settings = GeneralRelaxedSettings::mixed_61_probe(seed, COUPLED_SEPARATOR_WORKERS);
        settings.sweeps_per_epoch = COUPLED_SEPARATOR_ROUNDS;
        settings.global_samples_per_move = 10;
        settings.focused_samples_per_move = 10;
        settings.refinement_rounds = 5;
        settings.coupled_dynamic_separator = true;
        settings
    }

    #[test]
    fn rollback_backend_rejects_non_rollback_pressure_models() {
        for pressure_model in [
            GeneralRelaxedPressureModel::ContinuousTrianglePoles,
            GeneralRelaxedPressureModel::DynamicPoles,
        ] {
            let settings = GeneralRelaxedSettings {
                pressure_model,
                ..GeneralRelaxedSettings::mixed_61_probe(0, 1)
            };
            let error = validate_relaxed_settings(settings).unwrap_err();
            assert!(error
                .to_string()
                .contains("requires structured or directional pressure"));
        }
        let settings = GeneralRelaxedSettings {
            pressure_model: GeneralRelaxedPressureModel::DirectionalPenetration,
            ..GeneralRelaxedSettings::mixed_61_probe(0, 1)
        };
        assert!(validate_relaxed_settings(settings).is_ok());
    }

    #[test]
    fn ear_clipping_preserves_concave_area() {
        let polygon = l_shape();
        let cells = triangulate_ring(polygon.regions()[0].outer.points()).unwrap();
        let cell_area = cells
            .iter()
            .map(|cell| {
                let [a, b, c] = cell.points;
                ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).abs() / 2.0
            })
            .sum::<f64>();
        assert_eq!(cells.len(), 4);
        assert!((cell_area - polygon.area_mm2()).abs() < 1e-9);
    }

    #[test]
    fn triangle_penetration_has_a_boolean_zero_boundary() {
        let first = Triangle::new([point(0.0, 0.0), point(2.0, 0.0), point(0.0, 2.0)]);
        let second = Triangle::new([point(0.0, 0.0), point(2.0, 0.0), point(0.0, 2.0)]);
        assert!(triangle_penetration(first, 0.0, 0.0, second, 1.0, 0.0).is_some());
        assert!(triangle_penetration(first, 0.0, 0.0, second, 2.0, 0.0).is_none());
    }

    #[test]
    fn canonical_grid_triangle_overlap_has_an_exact_contact_boundary() {
        let first = Triangle::new([point(0.0, 0.0), point(2.0, 0.0), point(0.0, 2.0)]);
        let second = Triangle::new([point(0.0, 0.0), point(2.0, 0.0), point(0.0, 2.0)]);
        assert_eq!(
            triangles_overlap_on_grid(first, second, 1_999, 0),
            Some(true)
        );
        assert_eq!(
            triangles_overlap_on_grid(first, second, 2_000, 0),
            Some(false)
        );
        assert_eq!(
            triangles_overlap_on_grid(first, second, 2_001, 0),
            Some(false)
        );
        let origin_relative = relative_grid_coordinate(0.0, 1.999);
        let shifted_relative = relative_grid_coordinate(1_000_000.0, 1_000_001.999);
        assert_eq!(shifted_relative, origin_relative);
        assert_eq!(
            triangles_overlap_on_grid(first, second, shifted_relative.unwrap(), 0),
            Some(true)
        );
        assert_eq!(relative_grid_coordinate(f64::INFINITY, 0.0), None);
    }

    #[test]
    fn convex_line_interval_returns_contact_coordinates() {
        let square = [
            point(0.0, 0.0),
            point(4.0, 0.0),
            point(4.0, 3.0),
            point(0.0, 3.0),
        ];
        assert_eq!(
            convex_line_interval(&square, point(-2.0, 1.0), (1.0, 0.0)),
            Some((2.0, 6.0))
        );
        assert_eq!(
            convex_line_interval(&square, point(2.0, -5.0), (0.0, 1.0)),
            Some((5.0, 8.0))
        );
        assert_eq!(
            convex_line_interval(&square, point(-2.0, 4.0), (1.0, 0.0)),
            None
        );
    }

    #[test]
    fn interval_union_drives_directional_penetration() {
        let mut intervals = vec![(5.0, 8.0), (0.0, 3.0), (2.0, 6.0), (10.0, 11.0)];
        merge_intervals(&mut intervals);
        assert_eq!(intervals, vec![(0.0, 8.0), (10.0, 11.0)]);
        assert_eq!(interval_penetration(4.0, &intervals), 4.0);
        assert_eq!(interval_penetration(9.0, &intervals), 0.0);
        assert_eq!(interval_penetration(10.25, &intervals), 0.25);
    }

    #[test]
    fn lane_seed_derivation_is_stable_and_distinct() {
        assert_eq!(derive_seed(7, 2, 3), derive_seed(7, 2, 3));
        assert_ne!(derive_seed(7, 2, 3), derive_seed(7, 2, 4));
    }

    #[test]
    fn shared_pair_nfps_preserve_tracker_and_lane_budget_semantics() {
        let polygon = square(10.0);
        let pieces = [
            GeneralFastPiece {
                id: "first",
                polygon: &polygon,
                allow_rotation: false,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "second",
                polygon: &polygon,
                allow_rotation: false,
                allow_mirror: false,
            },
        ];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let incumbent =
            crate::search::general_fast::construct_short_side_first(&pieces, fast_settings)
                .unwrap();
        let (shared_catalog, shared_work) = build_surrogate_catalog(
            &pieces,
            fast_settings,
            SurrogateCatalogMode::CurrentAssignment,
            Some(&incumbent),
        )
        .unwrap();
        assert_eq!(shared_work.shared_pair_nfp_entries, 1);
        assert_eq!(shared_work.shared_pair_nfp_components, 4);
        let cold_catalog = Arc::new(SurrogateCatalog {
            geometry_class_by_input: shared_catalog.geometry_class_by_input.clone(),
            orientations: shared_catalog.orientations.clone(),
            shared_pair_nfps: BTreeMap::new(),
        });
        let mut relaxed_settings = GeneralRelaxedSettings::mixed_61_probe(0, 1);
        relaxed_settings.angle_seed_policy = GeneralRelaxedAngleSeedPolicy::CurrentOnly;
        relaxed_settings.pressure_model = GeneralRelaxedPressureModel::DirectionalPenetration;
        let state = RelaxedState {
            placements: vec![
                RelaxedPlacement {
                    input_index: 0,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 20.0,
                    translate_y: 20.0,
                },
                RelaxedPlacement {
                    input_index: 1,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 20.0,
                    translate_y: 20.0,
                },
            ],
            strip_depth_mm: 100.0,
        };
        let mut cold = LaneSearch::new(&pieces, fast_settings, relaxed_settings, 0, cold_catalog);
        let mut shared = LaneSearch::new(
            &pieces,
            fast_settings,
            relaxed_settings,
            0,
            shared_catalog.clone(),
        );
        let cold_tracker = cold.score_state(&state).unwrap();
        let shared_tracker = shared.score_state(&state).unwrap();
        assert_eq!(shared_tracker, cold_tracker);
        assert_eq!(
            shared.pair_nfp_cache_components,
            cold.pair_nfp_cache_components
        );
        assert_eq!(cold.counters.pair_nfp_builds, 1);
        assert_eq!(shared.counters.pair_nfp_builds, 0);
        assert_eq!(shared.counters.shared_pair_nfp_adoptions, 1);
        let key = shared
            .pair_nfp_key(&state.placements[0], &state.placements[1])
            .unwrap();
        assert!(Arc::ptr_eq(
            shared.pair_nfp_cache.get(&key).unwrap(),
            shared_catalog.shared_pair_nfps.get(&key).unwrap(),
        ));
        let components = shared.pair_nfp_cache_components;
        assert!(!directional_nfp_preflight_fits(
            0,
            components,
            components,
            false,
            MAX_NFP_COMPONENTS_PER_MOVE,
            components - 1,
        ));
        assert!(directional_nfp_preflight_fits(
            0, components, components, true, components, components,
        ));
        assert_ne!(
            (key.0, key.1, false, key.3, key.4, true),
            (key.3, key.4, true, key.0, key.1, false),
        );
        assert_ne!(
            (key.0, key.1, key.2, key.3, key.4, key.5),
            (key.0, key.1 + 1, key.2, key.3, key.4, key.5),
        );
    }

    #[test]
    fn lane_disruptions_preserve_control_and_cycle_restart_depth() {
        assert_eq!(
            (0..8).map(lane_disruption_count).collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 1, 2, 3, 1]
        );
    }

    #[test]
    fn frontier_blockers_use_transformed_source_bounds() {
        let wide = PolygonSet::from_outer(vec![
            point(0.0, 0.0),
            point(100.0, 0.0),
            point(100.0, 10.0),
            point(0.0, 10.0),
        ])
        .unwrap();
        let square = square(20.0);
        let pieces = [
            GeneralFastPiece {
                id: "rotated-wide",
                polygon: &wide,
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "translated-square",
                polygon: &square,
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let state = RelaxedState {
            placements: vec![
                RelaxedPlacement {
                    input_index: 0,
                    rotation_deg: 90.0,
                    mirrored: false,
                    translate_x: 0.0,
                    translate_y: 0.0,
                },
                RelaxedPlacement {
                    input_index: 1,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 0.0,
                    translate_y: 50.0,
                },
            ],
            strip_depth_mm: 120.0,
        };

        assert_eq!(high_frontier_blockers(&state, &pieces, 2), vec![0, 1]);
    }

    #[test]
    fn publication_reducer_selects_the_shallower_exact_lane() {
        let polygon = square(10.0);
        let pieces = [GeneralFastPiece {
            id: "square",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let mut diagnostics = GeneralRelaxedDiagnostics::default();
        let selected = select_lane_for_publication(
            &pieces,
            GeneralFastSettings::deterministic_test(100.0, 100.0),
            vec![feasible_lane(0, 10.0, 30.0), feasible_lane(1, 10.0, 10.0)],
            &mut diagnostics,
        );
        assert_eq!(selected.outcome.selected_lane, 1);
        assert!(matches!(
            selected.validation,
            ExactLaneValidation::Accepted { .. }
        ));
        assert_eq!(diagnostics.surrogate_feasible_states, 2);
        assert_eq!(diagnostics.exact_rejected_states, 0);
    }

    #[test]
    fn publication_reducer_uses_the_canonical_key_for_equal_metrics() {
        let polygon = square(10.0);
        let pieces = [GeneralFastPiece {
            id: "square",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let mut diagnostics = GeneralRelaxedDiagnostics::default();
        let selected = select_lane_for_publication(
            &pieces,
            GeneralFastSettings::deterministic_test(100.0, 100.0),
            vec![feasible_lane(0, 10.0, 10.0), feasible_lane(1, 20.0, 10.0)],
            &mut diagnostics,
        );
        assert_eq!(selected.outcome.selected_lane, 0);
    }

    #[test]
    fn publication_reducer_skips_an_exact_rejection() {
        let polygon = square(10.0);
        let pieces = [GeneralFastPiece {
            id: "square",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let mut diagnostics = GeneralRelaxedDiagnostics::default();
        let selected = select_lane_for_publication(
            &pieces,
            GeneralFastSettings::deterministic_test(100.0, 100.0),
            vec![feasible_lane(0, -1.0, 1.0), feasible_lane(1, 20.0, 20.0)],
            &mut diagnostics,
        );
        assert_eq!(selected.outcome.selected_lane, 1);
        assert_eq!(diagnostics.exact_rejected_states, 1);
        assert!(matches!(
            selected.validation,
            ExactLaneValidation::Accepted { .. }
        ));
    }

    #[test]
    fn atomic_replacement_delta_matches_full_rescore() {
        let polygons = [square(10.0), square(8.0), square(6.0)];
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "medium",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[2],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let relaxed_settings = GeneralRelaxedSettings::mixed_61_probe(0, 1);
        let (catalog, _) = build_surrogate_catalog(
            &pieces,
            fast_settings,
            SurrogateCatalogMode::StructuredGrid,
            None,
        )
        .unwrap();
        let mut search = LaneSearch::new(&pieces, fast_settings, relaxed_settings, 0, catalog);
        let state = RelaxedState {
            placements: vec![
                RelaxedPlacement {
                    input_index: 0,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 0.0,
                    translate_y: 0.0,
                },
                RelaxedPlacement {
                    input_index: 1,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 5.0,
                    translate_y: 0.0,
                },
                RelaxedPlacement {
                    input_index: 2,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 30.0,
                    translate_y: 0.0,
                },
            ],
            strip_depth_mm: 100.0,
        };
        let base = search.score_state(&state).unwrap();
        let replacements = vec![
            (
                0,
                RelaxedPlacement {
                    translate_x: 30.0,
                    ..state.placements[0].clone()
                },
            ),
            (
                2,
                RelaxedPlacement {
                    translate_x: 0.0,
                    ..state.placements[2].clone()
                },
            ),
        ];
        let incremental = search
            .score_after_replacements(&state, &base, &replacements)
            .unwrap();
        let mut replaced = state;
        for (index, placement) in replacements {
            replaced.placements[index] = placement;
        }
        let full = search.score_state(&replaced).unwrap();
        assert_eq!(incremental.boundary_violations, full.boundary_violations);
        assert_eq!(
            incremental
                .collision_pairs
                .iter()
                .map(|(first, second, _)| (*first, *second))
                .collect::<Vec<_>>(),
            full.collision_pairs
                .iter()
                .map(|(first, second, _)| (*first, *second))
                .collect::<Vec<_>>()
        );
        assert!((incremental.boundary_loss - full.boundary_loss).abs() < 1e-9);
        assert!((incremental.weighted_loss - full.weighted_loss).abs() < 1e-9);
        assert!((incremental.common_loss() - full.common_loss()).abs() < 1e-9);
        assert_eq!(incremental.boundaries.len(), full.boundaries.len());
        assert_eq!(incremental.pairs.len(), full.pairs.len());
        for index in 0..incremental.piece_count {
            assert_eq!(
                incremental.boundaries[index].violations,
                full.boundaries[index].violations
            );
            assert!(
                (incremental.boundaries[index].raw_loss - full.boundaries[index].raw_loss).abs()
                    < 1e-9
            );
            assert!(
                (incremental.incident_raw_loss[index] - full.incident_raw_loss[index]).abs() < 1e-9
            );
        }
        for first in 0..incremental.piece_count {
            for second in (first + 1)..incremental.piece_count {
                assert!(
                    (incremental.pair(first, second).raw_loss - full.pair(first, second).raw_loss)
                        .abs()
                        < 1e-9
                );
            }
        }
    }

    #[test]
    fn move_update_recomputes_weighted_loss_without_cancellation() {
        let mut score = PairTracker {
            piece_count: 3,
            boundaries: vec![
                BoundaryEntry {
                    violations: 0,
                    raw_loss: 0.0,
                };
                3
            ],
            pairs: vec![
                PairEntry {
                    raw_loss: 1.0e16,
                    guided_weight: 1.0,
                    normalization_scale: 1.0,
                },
                PairEntry {
                    raw_loss: 0.0,
                    guided_weight: 1.0,
                    normalization_scale: 1.0,
                },
                PairEntry {
                    raw_loss: 1.0,
                    guided_weight: 1.0,
                    normalization_scale: 1.0,
                },
            ],
            incident_raw_loss: vec![1.0e16, 1.0e16, 1.0],
            boundary_violations: 0,
            boundary_loss: 0.0,
            collision_pairs: vec![(0, 1, 1.0e16), (1, 2, 1.0)],
            weighted_loss: 1.0e16,
        };
        update_score_after_move(
            &mut score,
            0,
            (0, 0.0),
            PlacementScore {
                boundary_violations: 0,
                boundary_loss: 0.0,
                collision_pairs: Vec::new(),
                weighted_loss: 0.0,
            },
            &BTreeMap::new(),
        );
        assert_eq!(score.collision_pairs, vec![(1, 2, 1.0)]);
        assert_eq!(score.weighted_loss, 1.0);
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_raw_minimum_transition_matches_strict_thresholds() {
        assert_eq!(
            raw_minimum_transition(100.0, 100.0),
            RawMinimumTransition::NoImprovement
        );
        assert_eq!(
            raw_minimum_transition(99.0, 100.0),
            RawMinimumTransition::MinorImprovement
        );
        assert_eq!(
            raw_minimum_transition(98.0, 100.0),
            RawMinimumTransition::MinorImprovement
        );
        assert_eq!(
            raw_minimum_transition(97.999, 100.0),
            RawMinimumTransition::SubstantialImprovement
        );
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_canonical_tracker_agreement_uses_authoritative_rows() {
        let mut canonical = feasible_tracker(2);
        canonical.boundaries[0] = BoundaryEntry {
            violations: 1,
            raw_loss: 4.0,
        };
        canonical.boundary_violations = 1;
        canonical.boundary_loss = 4.0;
        canonical.pairs[0].raw_loss = 3.0;
        canonical.incident_raw_loss = vec![3.0, 3.0];
        canonical.collision_pairs = vec![(0, 1, 3.0)];
        canonical.weighted_loss = 7.0;

        let mut derived_drift = canonical.clone();
        derived_drift.boundary_loss = f64::from_bits(canonical.boundary_loss.to_bits() + 8);
        derived_drift.incident_raw_loss[0] =
            f64::from_bits(canonical.incident_raw_loss[0].to_bits() + 8);
        derived_drift.weighted_loss =
            f64::from_bits(canonical.weighted_loss.to_bits().saturating_add(8));
        assert!(authoritative_raw_tracker_disagreement(&canonical, &derived_drift).is_none());
        assert!(raw_tracker_disagreement(&canonical, &derived_drift).is_some());

        let mut changed = canonical.clone();
        changed.boundaries[0].raw_loss = 4.5;
        assert_eq!(
            authoritative_raw_tracker_disagreement(&canonical, &changed).as_deref(),
            Some("boundary rows differ")
        );
        changed = canonical.clone();
        changed.pairs[0].raw_loss = 3.5;
        assert_eq!(
            authoritative_raw_tracker_disagreement(&canonical, &changed).as_deref(),
            Some("pair rows differ")
        );
        changed = canonical.clone();
        changed.pairs[0].normalization_scale = 2.0;
        assert_eq!(
            authoritative_raw_tracker_disagreement(&canonical, &changed).as_deref(),
            Some("pair rows differ")
        );
        changed = canonical.clone();
        changed.collision_pairs[0].2 = 3.5;
        assert_eq!(
            authoritative_raw_tracker_disagreement(&canonical, &changed).as_deref(),
            Some("collision rows differ")
        );
        changed = canonical.clone();
        changed.boundary_violations = 2;
        assert!(authoritative_raw_tracker_disagreement(&canonical, &changed)
            .as_deref()
            .is_some_and(|reason| reason.starts_with("boundary violation count")));
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_canonical_rollback_installs_rescore_before_strike_state() {
        let mut restored = feasible_tracker(2);
        restored.boundaries[0] = BoundaryEntry {
            violations: 1,
            raw_loss: 4.0,
        };
        restored.boundary_violations = 1;
        restored.boundary_loss = 4.0;
        restored.pairs[0].raw_loss = 3.0;
        restored.incident_raw_loss = vec![3.0, 3.0];
        restored.collision_pairs = vec![(0, 1, 3.0)];
        restored.weighted_loss = 7.0;
        let mut minimum = restored.clone();
        minimum.boundary_loss = 99.0;
        minimum.incident_raw_loss = vec![99.0, 99.0];
        minimum.weighted_loss = 102.0;
        let minimum_state = RelaxedState {
            placements: vec![
                RelaxedPlacement {
                    input_index: 0,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 1.0,
                    translate_y: 2.0,
                },
                RelaxedPlacement {
                    input_index: 1,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 3.0,
                    translate_y: 4.0,
                },
            ],
            strip_depth_mm: 10.0,
        };
        let mut master = LaneOutcome {
            state: RelaxedState {
                placements: Vec::new(),
                strip_depth_mm: 999.0,
            },
            score: feasible_tracker(0),
            weights: BTreeMap::new(),
            counters: WorkCounters::default(),
            selected_lane: 7,
            restart_disruptions: 3,
        };
        let weights = BTreeMap::new();
        let mut strikes = 2;
        let mut strike_start_raw_loss = 100.0;

        let reached_limit = install_canonical_coupled_rollback(
            restored.clone(),
            &minimum_state,
            &mut minimum,
            &mut master,
            &weights,
            &mut strikes,
            &mut strike_start_raw_loss,
        );

        assert!(!reached_limit);
        assert_eq!(strikes, 0);
        assert_eq!(strike_start_raw_loss, 7.0);
        assert_eq!(minimum, restored);
        assert_eq!(master.score, restored);
        assert_eq!(
            canonical_state_key(&master.state),
            canonical_state_key(&minimum_state)
        );
        assert_eq!(master.selected_lane, 0);
        assert_eq!(master.restart_disruptions, 0);
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn exact_boundary_projection_clamps_rotated_concave_geometry_on_every_side() {
        let polygon = l_shape();
        let pieces = [GeneralFastPiece {
            id: "concave",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let mut settings = GeneralFastSettings::deterministic_test(20.0, 20.0);
        settings.total_padding_mm = 2.0;
        let inset = collision_sheet_inset_mm(settings);

        for (translate_x, translate_y) in [(-100.0, 5.0), (100.0, 5.0), (5.0, -100.0), (5.0, 100.0)]
        {
            let mut state = RelaxedState {
                placements: vec![RelaxedPlacement {
                    input_index: 0,
                    rotation_deg: 33.125,
                    mirrored: true,
                    translate_x,
                    translate_y,
                }],
                strip_depth_mm: 20.0,
            };
            let projected = project_piece_into_exact_boundary(&state, &pieces, settings, 0)
                .expect("rotated concave contour fits the exact sheet");
            state.placements[0] = projected.clone();
            let collision = polygon
                .transformed(
                    projected.rotation_deg,
                    projected.mirrored,
                    projected.translate_x,
                    projected.translate_y,
                )
                .and_then(|geometry| geometry.offset(collision_expansion_mm(settings)))
                .expect("projected collision geometry");
            assert!(collision.fits_rect(inset, inset, 20.0 - inset, 20.0 - inset));
            let repeated = project_piece_into_exact_boundary(&state, &pieces, settings, 0)
                .expect("projection is idempotent");
            assert_eq!(projected.rotation_deg, repeated.rotation_deg);
            assert_eq!(projected.mirrored, repeated.mirrored);
            assert_eq!(projected.translate_x, repeated.translate_x);
            assert_eq!(projected.translate_y, repeated.translate_y);
        }
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn exact_boundary_projection_rejects_an_empty_inner_fit() {
        let polygon = square(30.0);
        let pieces = [GeneralFastPiece {
            id: "oversized",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let state = RelaxedState {
            placements: vec![RelaxedPlacement {
                input_index: 0,
                rotation_deg: 17.0,
                mirrored: false,
                translate_x: 0.0,
                translate_y: 0.0,
            }],
            strip_depth_mm: 20.0,
        };
        let result = project_piece_into_exact_boundary(
            &state,
            &pieces,
            GeneralFastSettings::deterministic_test(20.0, 20.0),
            0,
        );
        let Err(error) = result else {
            panic!("an oversized contour has no exact inner fit");
        };
        assert!(error.contains("empty canonical inner-fit rectangle"));
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_feasibility_precedes_a_single_gls_update() {
        let feasible = feasible_tracker(2);
        assert_eq!(
            coupled_round_disposition(&feasible, 0.0),
            CoupledRoundDisposition::AcceptFeasible
        );

        let mut colliding = feasible_lane(0, 0.0, 0.0);
        colliding.state.placements.push(RelaxedPlacement {
            input_index: 1,
            rotation_deg: 0.0,
            mirrored: false,
            translate_x: 0.0,
            translate_y: 0.0,
        });
        colliding.score = PairTracker {
            piece_count: 2,
            boundaries: vec![
                BoundaryEntry {
                    violations: 0,
                    raw_loss: 0.0,
                };
                2
            ],
            pairs: vec![PairEntry {
                raw_loss: 10.0,
                guided_weight: 1.0,
                normalization_scale: 1.0,
            }],
            incident_raw_loss: vec![10.0, 10.0],
            boundary_violations: 0,
            boundary_loss: 0.0,
            collision_pairs: vec![(0, 1, 10.0)],
            weighted_loss: 10.0,
        };
        assert_eq!(
            coupled_round_disposition(&colliding.score, 12.0),
            CoupledRoundDisposition::ContinueInfeasible(
                RawMinimumTransition::SubstantialImprovement
            )
        );
        let mut weights = BTreeMap::new();
        apply_coupled_gls_update(&mut weights, &mut colliding);
        assert_eq!(weights.get(&(0, 1)).copied(), Some(1.1));
        assert_eq!(colliding.score.weighted_loss, 11.0);
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_auditor_reuses_variants_and_restores_worker_accounting() {
        let polygons = [square(10.0), square(8.0)];
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let relaxed_settings = coupled_test_settings(7);
        let (catalog, _) = build_surrogate_catalog(
            &pieces,
            fast_settings,
            SurrogateCatalogMode::ZeroDegreeOnly,
            None,
        )
        .unwrap();
        let hazard_catalog = Arc::new(JaguaHazardCatalog::new(&pieces, fast_settings).unwrap());
        let mut search = LaneSearch::new(&pieces, fast_settings, relaxed_settings, 7, catalog);
        search.hazard_catalog = Some(hazard_catalog);
        let worker = Mutex::new(search);
        let state = RelaxedState {
            placements: vec![
                RelaxedPlacement {
                    input_index: 0,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 10.0,
                    translate_y: 10.0,
                },
                RelaxedPlacement {
                    input_index: 1,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 40.0,
                    translate_y: 10.0,
                },
            ],
            strip_depth_mm: 100.0,
        };
        let (initial_score, initial_work) =
            coupled_auditor_score(&worker, &state, &BTreeMap::new(), 1);
        assert!(initial_score.unwrap().feasible());
        assert_eq!(initial_work.auditor_layout_loads, 1);
        assert_eq!(initial_work.auditor_index_builds, 1);
        {
            let worker = worker.lock().unwrap();
            assert_eq!(worker.counters.dynamic_layout_loads, 0);
            assert_eq!(worker.counters.dynamic_index_builds, 0);
            assert!(worker.hazard_index.is_some());
        }

        let outcome = worker.lock().unwrap().run_sweep(state.clone(), 0).unwrap();
        assert!(outcome.score.feasible());
        {
            let worker = worker.lock().unwrap();
            assert_eq!(worker.counters.dynamic_layout_loads, 1);
            assert_eq!(worker.counters.dynamic_index_builds, 0);
        }
        let (restored_score, restored_work) =
            coupled_auditor_score(&worker, &state, &BTreeMap::new(), 1);
        assert!(restored_score.unwrap().feasible());
        assert_eq!(restored_work.auditor_layout_loads, 1);
        assert_eq!(restored_work.auditor_index_builds, 0);
        let worker = worker.lock().unwrap();
        assert_eq!(worker.counters.dynamic_layout_loads, 1);
        assert_eq!(worker.counters.dynamic_index_builds, 0);
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_target_failure_is_structured_and_keeps_work() {
        let polygon = square(10.0);
        let pieces = [GeneralFastPiece {
            id: "square",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let incumbent = construct_short_side_first(&pieces, fast_settings).unwrap();
        let relaxed_settings = coupled_test_settings(11);
        let (catalog, _) = build_surrogate_catalog(
            &pieces,
            fast_settings,
            SurrogateCatalogMode::ZeroDegreeOnly,
            Some(&incumbent),
        )
        .unwrap();
        let target_seed = 19;
        let worker_seeds = (0..COUPLED_SEPARATOR_WORKERS)
            .map(|worker| derive_seed(target_seed, 0, worker))
            .collect::<Vec<_>>();
        let invalid = RelaxedState {
            placements: vec![RelaxedPlacement {
                input_index: 0,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: f64::NAN,
                translate_y: 0.0,
            }],
            strip_depth_mm: 99.0,
        };
        let hazard_catalog = Arc::new(JaguaHazardCatalog::new(&pieces, fast_settings).unwrap());
        let outcome = run_coupled_separator_target(
            &pieces,
            fast_settings,
            relaxed_settings,
            &incumbent,
            invalid,
            0,
            99.0,
            50.0,
            target_seed,
            23,
            worker_seeds,
            CoupledSeparatorArm::Control,
            CoupledRollbackRescorePolicy::StrictDerivedAgreement,
            false,
            catalog,
            hazard_catalog,
        )
        .unwrap();
        assert!(outcome.accepted.is_none());
        assert!(outcome
            .diagnostics
            .failure_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("initial full score")));
        assert_eq!(outcome.diagnostics.rounds, 0);
        assert_eq!(outcome.work.layout_loads, 1);
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_pair_visit_cap_falls_back_atomically() {
        let reason = coupled_separator_cap_reason(
            &[],
            COUPLED_SEPARATOR_WORKER_FULL_SCORE_PAIR_VISIT_CAP + 1,
            1,
        )
        .unwrap();
        assert_eq!(reason.as_deref(), Some("worker full-score pair-visit cap"));
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn disabled_coupled_separator_replays_without_diagnostics() {
        let polygon = square(10.0);
        let pieces = [GeneralFastPiece {
            id: "square",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let incumbent = construct_short_side_first(&pieces, fast_settings).unwrap();
        let mut settings = GeneralRelaxedSettings::mixed_61_probe(29, 1);
        settings.epochs = 1;
        settings.sweeps_per_epoch = 1;
        settings.global_samples_per_move = 1;
        settings.focused_samples_per_move = 1;
        settings.refinement_rounds = 1;
        settings.coupled_dynamic_separator = false;
        let first = improve_complete_layout(&pieces, fast_settings, settings, &incumbent).unwrap();
        let second = improve_complete_layout(&pieces, fast_settings, settings, &incumbent).unwrap();
        assert_eq!(first.result, second.result);
        assert_eq!(first.diagnostics, second.diagnostics);
        assert!(first.diagnostics.coupled_dynamic_separator.is_none());
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_separator_comparator_applies_weighted_then_raw_order() {
        let mut weighted = feasible_lane(0, 0.0, 0.0);
        weighted.score.weighted_loss = 2.0;
        weighted.score.boundary_loss = 1.0;
        weighted.score.collision_pairs = vec![(0, 1, 1.0)];
        let mut raw = feasible_lane(1, 1.0, 0.0);
        raw.score.weighted_loss = 3.0;
        raw.score.boundary_loss = 0.1;
        raw.score.collision_pairs = vec![(0, 1, 0.1)];
        assert_eq!(
            compare_coupled_separator_outcomes(0, &weighted, 1, &raw),
            Ordering::Less
        );

        raw.score.weighted_loss = weighted.score.weighted_loss;
        assert_eq!(
            compare_coupled_separator_outcomes(0, &weighted, 1, &raw),
            Ordering::Greater
        );
        assert_eq!(
            compare_coupled_separator_outcomes(1, &raw, 0, &weighted),
            Ordering::Less
        );
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_separator_seed_table_is_shared_between_arms() {
        let seed = 17_u64 ^ COUPLED_SEPARATOR_SEED_DOMAIN;
        let first = (0..COUPLED_SEPARATOR_TARGETS)
            .map(|target| {
                let target_seed = derive_seed(seed, target, usize::MAX - 64);
                (0..COUPLED_SEPARATOR_WORKERS)
                    .map(|worker| derive_seed(target_seed, 0, worker))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let second = (0..COUPLED_SEPARATOR_TARGETS)
            .map(|target| {
                let target_seed = derive_seed(seed, target, usize::MAX - 64);
                (0..COUPLED_SEPARATOR_WORKERS)
                    .map(|worker| derive_seed(target_seed, 0, worker))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(first, second);
        assert_ne!(first[0], first[1]);
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_separator_is_cross_schedule_deterministic_and_resets_targets() {
        let polygons = [square(10.0), square(8.0)];
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let protected = construct_short_side_first(&pieces, fast_settings).unwrap();
        let mut settings = GeneralRelaxedSettings::mixed_61_probe(31, COUPLED_SEPARATOR_WORKERS);
        settings.sweeps_per_epoch = COUPLED_SEPARATOR_ROUNDS;
        settings.global_samples_per_move = 10;
        settings.focused_samples_per_move = 10;
        settings.refinement_rounds = 5;
        let single = JobPool::new(Some(1)).run_scoped(|| {
            run_coupled_dynamic_separator_experiment(
                &pieces,
                fast_settings,
                settings,
                &protected,
                None,
            )
        });
        let parallel = JobPool::new(Some(4)).run_scoped(|| {
            run_coupled_dynamic_separator_experiment(
                &pieces,
                fast_settings,
                settings,
                &protected,
                None,
            )
        });
        assert_eq!(single, parallel);
        for arm in [&single.control, &single.treatment] {
            assert_eq!(arm.targets_attempted, arm.targets.len());
            assert_eq!(arm.catalog_builds, 1);
            assert_eq!(
                arm.index_builds,
                arm.targets_attempted * COUPLED_SEPARATOR_WORKERS
            );
            assert_eq!(arm.immutable_variant_builds, 4);
            for (ordinal, target) in arm.targets.iter().enumerate() {
                assert_eq!(target.ordinal, ordinal);
                assert_eq!(target.worker_seeds.len(), COUPLED_SEPARATOR_WORKERS);
                if ordinal > 0 {
                    assert_ne!(target.target_seed, arm.targets[ordinal - 1].target_seed);
                }
            }
        }
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_catalog_runs_supported_concave_shapes() {
        let polygons = [l_shape(), square(8.0)];
        let pieces = [
            GeneralFastPiece {
                id: "concave",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "square",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let protected = construct_short_side_first(&pieces, fast_settings).unwrap();
        let settings = coupled_experiment_test_settings(37);
        let result = run_coupled_dynamic_separator_experiment(
            &pieces,
            fast_settings,
            settings,
            &protected,
            None,
        );

        for arm in [&result.control, &result.treatment] {
            assert_eq!(arm.catalog_builds, 1);
            assert_eq!(arm.immutable_variant_builds, 4);
            assert!(!arm
                .skipped_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("dynamic hazard catalog")));
        }
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn coupled_catalog_reports_unsupported_holes_without_mutating_protected_result() {
        let polygon = holed_square();
        let pieces = [GeneralFastPiece {
            id: "holed",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let protected = construct_short_side_first(&pieces, fast_settings).unwrap();
        let result = run_coupled_dynamic_separator_experiment(
            &pieces,
            fast_settings,
            coupled_experiment_test_settings(41),
            &protected,
            None,
        );
        let protected_fingerprint = coupled_fast_placement_fingerprint(&protected.placements);

        for arm in [&result.control, &result.treatment] {
            assert!(!arm.attempted);
            assert_eq!(arm.catalog_builds, 0);
            assert_eq!(
                arm.final_placement_fingerprint.as_deref(),
                Some(protected_fingerprint.as_str())
            );
            assert_eq!(
                arm.final_placements,
                coupled_placement_diagnostics(&protected.placements)
            );
            assert!(arm
                .skipped_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("confirmation catalog")));
        }
    }

    #[test]
    #[cfg(feature = "jagua-experimental")]
    fn public_relaxed_path_reports_coupled_hole_fallback() {
        let polygon = holed_square();
        let pieces = [GeneralFastPiece {
            id: "holed",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let fast_settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let protected = construct_short_side_first(&pieces, fast_settings).unwrap();
        let outcome = improve_complete_layout(
            &pieces,
            fast_settings,
            coupled_experiment_test_settings(43),
            &protected,
        )
        .unwrap();

        assert_eq!(outcome.result, protected);
        assert_eq!(
            outcome.diagnostics.skipped_reason.as_deref(),
            Some("relaxed search does not yet flatten hole topology")
        );
        let coupled = outcome
            .diagnostics
            .coupled_dynamic_separator
            .expect("coupled fallback diagnostics");
        let protected_fingerprint = coupled_fast_placement_fingerprint(&protected.placements);
        for arm in [&coupled.control, &coupled.treatment] {
            assert!(!arm.attempted);
            assert_eq!(
                arm.final_placement_fingerprint.as_deref(),
                Some(protected_fingerprint.as_str())
            );
            assert!(arm
                .skipped_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("confirmation catalog")));
        }
    }
}
