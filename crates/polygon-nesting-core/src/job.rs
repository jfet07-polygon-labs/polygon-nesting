//! Typed core job execution.
//!
//! This module validates and prepares protocol requests, owns job-local caches
//! and worker pools, dispatches irregular nesting, sequences progress events,
//! projects typed results and failures, and reports execution diagnostics.

use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;

use polygon_nesting_protocol::{
    DiagnosticTraceMode, EllipseSource, EngineError, EngineErrorCode, EngineEvent, EngineOutcome,
    EngineProfile, EngineRequest, EngineSettings, ExecutionDiagnostics, GeometrySettings,
    HistoryMode as ProtocolHistoryMode, OptimizerSettings, PlacementPolicy,
    PreparedPiece as ProtocolPreparedPiece, SourceEntityHandle, SourceGeometry,
    SourceGeometryEntityType, SourceGeometrySegment, SourcePiece, SourceWarning,
};

use crate::caches::GeometryCacheStore;
use crate::control::CancellationControl;
use crate::domain::{
    DxfArcSegment, DxfEllipseSource, DxfEllipseSourceKind, DxfEntityHandle, DxfGeometryEntityType,
    DxfGeometrySegment, DxfGeometrySummary, DxfLineSegment, ImportWarning, ImportedPiece,
    IntrinsicObjectiveProfileId, IrregularGeometrySettings, IrregularNestingSettings,
    IrregularOptimizerSettings, IrregularPlacementPolicyId, PieceId, Rect, SheetSpec, SourceFileId,
};
use crate::events::{EngineEventSink, EventSequencer};
use crate::nfp_ifp::NfpIfpAbortReason;
use crate::parallel::JobPool;
use crate::result::coordinator::{compute_irregular_nesting, ComputeIrregularNestingOptions};
use crate::result::progress::IrregularComputeEventSink;
use crate::result::{
    HistoryMode, IrregularComputeErrorType, IrregularPortfolioPhase, IrregularPortfolioProgress,
    NestingOptions, NestingRequest,
};
use crate::search::layout_scorer::FreeMaterialCache;
use crate::search::sort_pieces::{CutRowRef, PreparedPiece, RectWith};

fn prepare_nesting_request(request: &EngineRequest) -> NestingRequest {
    NestingRequest {
        sheet: sheet_spec_from_engine(&request.sheet),
        padding: request.settings.padding,
        pieces: request
            .pieces
            .iter()
            .map(prepared_piece_from_engine)
            .collect(),
        source_pieces: request
            .source_pieces
            .iter()
            .map(source_piece_from_engine)
            .collect(),
        options: NestingOptions {
            allow_global_rotation: request.settings.allow_global_rotation,
            allow_global_mirror: Some(request.settings.allow_global_mirror),
            history_mode: history_mode_from_engine(request.history_mode),
            diagnostic_trace_mode: request.diagnostic_trace_mode,
            irregular_settings: Some(nesting_settings_from_engine(
                &request.settings,
                request.profile,
            )),
        },
    }
}

fn sheet_spec_from_engine(sheet: &polygon_nesting_protocol::SheetSpec) -> SheetSpec {
    SheetSpec {
        width: sheet.width,
        height: sheet.height,
        label: sheet.label.clone(),
    }
}

fn nesting_settings_from_engine(
    settings: &EngineSettings,
    profile: EngineProfile,
) -> IrregularNestingSettings {
    IrregularNestingSettings {
        geometry: geometry_settings_from_engine(&settings.geometry),
        optimizer: optimizer_settings_from_engine(&settings.optimizer, profile),
    }
}

fn geometry_settings_from_engine(settings: &GeometrySettings) -> IrregularGeometrySettings {
    IrregularGeometrySettings {
        flattening_sag_tolerance_mm: settings.flattening_sag_tolerance_mm,
        clearance_safety_margin_mm: settings.clearance_safety_margin_mm,
        geometry_backend_id: settings.geometry_backend_id.clone(),
        geometry_backend_version: settings.geometry_backend_version.clone(),
    }
}

fn optimizer_settings_from_engine(
    settings: &OptimizerSettings,
    profile: EngineProfile,
) -> IrregularOptimizerSettings {
    IrregularOptimizerSettings {
        order_window: settings.order_window,
        beam_width: settings.beam_width,
        local_candidate_fanout: settings.local_candidate_fanout,
        local_repair_budget: settings.local_repair_budget,
        intrinsic_shared_archive_enabled: settings.intrinsic_shared_archive_enabled,
        intrinsic_objective_profile_id: intrinsic_objective_profile_id_from_engine(profile),
        transform_cap: settings.transform_cap,
        transform_minimum_edge_length_mm: settings.transform_minimum_edge_length_mm,
        transform_angle_deduplication_tolerance_deg: settings
            .transform_angle_deduplication_tolerance_deg,
        configured_rotation_enabled: settings.configured_rotation_enabled,
        edge_alignment_enabled: settings.edge_alignment_enabled,
        configured_rotation_deg: settings.configured_rotation_deg.clone(),
        ga_enabled: settings.ga_enabled,
        baseline_only: settings.baseline_only,
        ga_population: settings.ga_population,
        ga_generation_budget: settings.ga_generation_budget,
        ga_evaluation_budget: settings.ga_evaluation_budget,
        ga_time_budget_ms: settings.ga_time_budget_ms,
        ga_seed: settings.ga_seed.clone(),
        priority_order_mutation_enabled: settings.priority_order_mutation_enabled,
        transform_preference_mutation_enabled: settings.transform_preference_mutation_enabled,
        placement_policy_mutation_enabled: settings.placement_policy_mutation_enabled,
        placement_policy_id: placement_policy_id_from_engine(settings.placement_policy_id),
        placement_policy_ids: settings
            .placement_policy_ids
            .iter()
            .copied()
            .map(placement_policy_id_from_engine)
            .collect(),
    }
}

fn prepared_piece_from_engine(piece: &ProtocolPreparedPiece) -> PreparedPiece {
    PreparedPiece {
        id: PieceId::new(piece.id.clone()),
        source_piece_id: PieceId::new(piece.source_piece_id.clone()),
        interchangeability_key: piece.interchangeability_key.clone(),
        real_bounds: rect_from_engine(&piece.real_bounds),
        padded_bounds: RectWith {
            rect: Rect {
                x: piece.padded_bounds.x,
                y: piece.padded_bounds.y,
                width: piece.padded_bounds.width,
                height: piece.padded_bounds.height,
            },
            longest_edge: piece.padded_bounds.longest_edge,
            area: piece.padded_bounds.area,
            imbalance: piece.padded_bounds.imbalance,
        },
        padding: piece.padding,
        allow_rotation: piece.allow_rotation,
        allow_mirror: piece.allow_mirror,
        cut_row_ref: piece.cut_row_ref.as_ref().map(|reference| CutRowRef {
            reference: reference.reference.clone(),
            customer_name: reference.customer_name.clone(),
            csv_row_id: reference.csv_row_id.clone(),
        }),
    }
}

fn source_piece_from_engine(piece: &SourcePiece) -> ImportedPiece {
    ImportedPiece {
        id: PieceId::new(piece.id.clone()),
        source_file_id: SourceFileId::new(piece.source_file_id.clone()),
        source_layer: piece.source_layer.clone(),
        label: piece.label.clone(),
        real_bounds: rect_from_engine(&piece.real_bounds),
        geometry: geometry_summary_from_engine(&piece.geometry),
        warnings: piece.warnings.iter().map(warning_from_engine).collect(),
    }
}

fn rect_from_engine(rect: &polygon_nesting_protocol::Rect) -> Rect {
    Rect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

fn geometry_summary_from_engine(geometry: &SourceGeometry) -> DxfGeometrySummary {
    DxfGeometrySummary {
        entity_type: geometry_entity_type_from_engine(geometry.entity_type),
        closed: geometry.closed,
        segments: geometry
            .segments
            .iter()
            .map(geometry_segment_from_engine)
            .collect(),
    }
}

fn geometry_entity_type_from_engine(
    entity_type: SourceGeometryEntityType,
) -> DxfGeometryEntityType {
    match entity_type {
        SourceGeometryEntityType::Line => DxfGeometryEntityType::Line,
        SourceGeometryEntityType::Lwpolyline => DxfGeometryEntityType::Lwpolyline,
        SourceGeometryEntityType::Polyline => DxfGeometryEntityType::Polyline,
        SourceGeometryEntityType::Circle => DxfGeometryEntityType::Circle,
        SourceGeometryEntityType::Arc => DxfGeometryEntityType::Arc,
        SourceGeometryEntityType::Ellipse => DxfGeometryEntityType::Ellipse,
        SourceGeometryEntityType::DxfShape => DxfGeometryEntityType::DxfShape,
        SourceGeometryEntityType::PresetShape => DxfGeometryEntityType::PresetShape,
    }
}

fn geometry_segment_from_engine(segment: &SourceGeometrySegment) -> DxfGeometrySegment {
    match segment {
        SourceGeometrySegment::Line(line) => DxfGeometrySegment::Line(DxfLineSegment {
            x1: line.x1,
            y1: line.y1,
            x2: line.x2,
            y2: line.y2,
            bulge: line.bulge,
            source_curve: line.source_curve.as_ref().map(ellipse_source_from_engine),
        }),
        SourceGeometrySegment::Arc(arc) => DxfGeometrySegment::Arc(DxfArcSegment {
            x1: arc.x1,
            y1: arc.y1,
            x2: arc.x2,
            y2: arc.y2,
            cx: arc.cx,
            cy: arc.cy,
            radius: arc.radius,
            start_angle: arc.start_angle,
            end_angle: arc.end_angle,
        }),
    }
}

fn ellipse_source_from_engine(source: &EllipseSource) -> DxfEllipseSource {
    DxfEllipseSource {
        kind: match source.kind {
            polygon_nesting_protocol::EllipseSourceKind::Ellipse => DxfEllipseSourceKind::Ellipse,
        },
        source_id: source.source_id.clone(),
        cx: source.cx,
        cy: source.cy,
        major_axis_x: source.major_axis_x,
        major_axis_y: source.major_axis_y,
        axis_ratio: source.axis_ratio,
        start_angle: source.start_angle,
        end_angle: source.end_angle,
    }
}

fn warning_from_engine(warning: &SourceWarning) -> ImportWarning {
    ImportWarning {
        code: warning.code.clone(),
        message: warning.message.clone(),
        entity_type: warning.entity_type.clone(),
        entity_handle: warning
            .entity_handle
            .as_ref()
            .map(entity_handle_from_engine),
    }
}

fn entity_handle_from_engine(handle: &SourceEntityHandle) -> DxfEntityHandle {
    match handle {
        SourceEntityHandle::Text(value) => DxfEntityHandle::Text(value.clone()),
        SourceEntityHandle::Number(value) => DxfEntityHandle::Number(*value),
    }
}

fn placement_policy_id_from_engine(policy: PlacementPolicy) -> IrregularPlacementPolicyId {
    match policy {
        PlacementPolicy::BalancedCompactness => IrregularPlacementPolicyId::BalancedCompactness,
        PlacementPolicy::ShortSideFill => IrregularPlacementPolicyId::ShortSideFill,
        PlacementPolicy::EdgeContactThenBalancedCompactness => {
            IrregularPlacementPolicyId::EdgeContactThenBalancedCompactness
        }
    }
}

fn intrinsic_objective_profile_id_from_engine(
    profile: EngineProfile,
) -> IntrinsicObjectiveProfileId {
    match profile {
        EngineProfile::Compact => IntrinsicObjectiveProfileId::Compact,
        EngineProfile::CompactShortSide => IntrinsicObjectiveProfileId::ShortSide,
    }
}

fn history_mode_from_engine(history_mode: ProtocolHistoryMode) -> HistoryMode {
    match history_mode {
        ProtocolHistoryMode::Stream => HistoryMode::Stream,
        ProtocolHistoryMode::Final => HistoryMode::Final,
        ProtocolHistoryMode::Off => HistoryMode::Off,
    }
}

struct ProtocolEventSink<'a> {
    sequencer: EventSequencer<'a>,
}

impl<'a> ProtocolEventSink<'a> {
    fn new(sink: &'a mut dyn EngineEventSink) -> Self {
        Self {
            sequencer: EventSequencer::new(sink),
        }
    }
}

impl IrregularComputeEventSink for ProtocolEventSink<'_> {
    fn emit_state_snapshot(
        &mut self,
        snapshot: &crate::result::IrregularStateSnapshot,
        beam_width: f64,
    ) {
        self.sequencer.emit(EngineEvent::StateSnapshot {
            snapshot: project_state_snapshot(snapshot),
            beam_width,
        });
    }

    fn emit_portfolio_progress(&mut self, progress: &IrregularPortfolioProgress) {
        self.sequencer.emit(EngineEvent::PortfolioProgress {
            progress: polygon_nesting_protocol::PortfolioProgress {
                phase: match progress.phase {
                    IrregularPortfolioPhase::SharedArchive => {
                        polygon_nesting_protocol::PortfolioPhase::SharedArchive
                    }
                    IrregularPortfolioPhase::ShortSideProfile => {
                        polygon_nesting_protocol::PortfolioPhase::ShortSideProfile
                    }
                    IrregularPortfolioPhase::Completed => {
                        polygon_nesting_protocol::PortfolioPhase::Completed
                    }
                },
                best_score: progress.best_score.as_ref().map(project_score_summary),
                elapsed_ms: progress.elapsed_ms,
            },
        });
    }
}

fn validation_error(error: polygon_nesting_protocol::ProtocolError) -> EngineError {
    let category = match error {
        polygon_nesting_protocol::ProtocolError::UnsupportedVersion { .. } => {
            EngineErrorCode::ProtocolVersionMismatch
        }
        _ => EngineErrorCode::MalformedInput,
    };
    EngineError::new(category, "validate-request", error.to_string())
}

fn internal_failure(operation: &str) -> EngineError {
    EngineError::new(
        EngineErrorCode::InternalFailure,
        operation,
        "polygon nesting execution failed internally",
    )
}

fn execution_diagnostics(
    thread_counts: crate::parallel::JobThreadCounts,
    elapsed_ms: f64,
    geometry: &crate::caches::CacheTelemetrySnapshot,
    free_material: &crate::search::layout_scorer::FreeMaterialCacheTelemetry,
) -> ExecutionDiagnostics {
    let mut counters = BTreeMap::new();
    for (key, value) in [
        ("geometry_cache.cap_bytes", geometry.cap_bytes),
        ("geometry_cache.current_bytes", geometry.current_bytes),
        ("geometry_cache.peak_bytes", geometry.peak_bytes),
        ("geometry_cache.admissions", geometry.admissions),
        ("geometry_cache.replacements", geometry.replacements),
        ("geometry_cache.evictions", geometry.evictions),
        ("geometry_cache.evicted_bytes", geometry.evicted_bytes),
        (
            "geometry_cache.oversized_rejections",
            geometry.oversized_rejections,
        ),
        ("geometry_cache.instances", geometry.cache_instances),
        ("free_material_cache.cap_bytes", free_material.cap_bytes),
        (
            "free_material_cache.current_bytes",
            free_material.current_bytes,
        ),
        ("free_material_cache.peak_bytes", free_material.peak_bytes),
        ("free_material_cache.entries", free_material.entries),
        ("free_material_cache.admissions", free_material.admissions),
        (
            "free_material_cache.replacements",
            free_material.replacements,
        ),
        ("free_material_cache.evictions", free_material.evictions),
        (
            "free_material_cache.evicted_bytes",
            free_material.evicted_bytes,
        ),
        (
            "free_material_cache.oversized_rejections",
            free_material.oversized_rejections,
        ),
        ("free_material_cache.hits", free_material.hits),
        ("free_material_cache.misses", free_material.misses),
    ] {
        counters.insert(key.to_string(), value);
    }
    for (namespace, telemetry) in &geometry.namespaces {
        let prefix = format!("geometry_cache.namespace.{namespace}");
        for (suffix, value) in [
            ("lookups", telemetry.lookups),
            ("hits", telemetry.hits),
            ("misses", telemetry.misses),
            ("stores", telemetry.stores),
            ("stale_detections", telemetry.stale_detections),
            ("stale_removals", telemetry.stale_removals),
            ("duplicate_computations", telemetry.duplicate_computations),
            ("single_flight_waits", telemetry.single_flight_waits),
            ("shard_lock_wait_nanos", telemetry.shard_lock_wait_nanos),
            (
                "shard_lock_contended_acquisitions",
                telemetry.shard_lock_contended_acquisitions,
            ),
            ("front_cache_hits", telemetry.front_cache_hits),
            ("backing_cache_hits", telemetry.backing_cache_hits),
            ("cloning_hits", telemetry.cloning_hits),
            ("cap_bytes", telemetry.cap_bytes),
            ("admissions", telemetry.admissions),
            ("replacements", telemetry.replacements),
            ("evictions", telemetry.evictions),
            ("evicted_bytes", telemetry.evicted_bytes),
            ("oversized_rejections", telemetry.oversized_rejections),
            ("entries", telemetry.entries),
            ("approx_bytes", telemetry.approx_bytes),
            ("peak_bytes", telemetry.peak_bytes),
            ("computation_time_nanos", telemetry.computation_time_nanos),
        ] {
            counters.insert(format!("{prefix}.{suffix}"), value);
        }
    }
    ExecutionDiagnostics {
        requested_workers: u32::try_from(thread_counts.requested).ok(),
        actual_workers: u32::try_from(thread_counts.actual).ok(),
        elapsed_ms: Some(elapsed_ms.max(0.0)),
        counters,
    }
}

fn project_result(
    result: crate::result::IrregularComputeResult,
    diagnostic_trace_mode: DiagnosticTraceMode,
) -> polygon_nesting_protocol::EngineResult {
    let diagnostic_traces = match diagnostic_trace_mode {
        DiagnosticTraceMode::Full => (
            result.capacity_trace.as_ref().map(project_capacity_trace),
            result
                .intrinsic_anytime_scheduler_trace
                .as_ref()
                .map(project_scheduler_trace),
            result
                .focused_complete_reconstruction_trace
                .as_ref()
                .map(project_focused_reconstruction_trace),
            result
                .intrinsic_short_side_observer_trace
                .as_ref()
                .map(project_short_side_observer_trace),
            result
                .intrinsic_short_side_pair_fold_trace
                .as_ref()
                .map(project_short_side_pair_fold_trace),
        ),
        DiagnosticTraceMode::Off => (None, None, None, None, None),
    };

    polygon_nesting_protocol::EngineResult {
        placed_collision_geometries: result
            .placed_collision_geometries
            .iter()
            .map(|placed| project_placed_collision_geometry(placed))
            .collect(),
        score: project_layout_score(&result.score),
        unplaced_piece_ids: result
            .unplaced_piece_ids
            .iter()
            .map(|id| id.as_str().to_string())
            .collect(),
        diagnostics: result.diagnostics.iter().map(project_diagnostic).collect(),
        sorted_piece_ids: result
            .sorted_piece_ids
            .iter()
            .map(|id| id.as_str().to_string())
            .collect(),
        state_snapshots: result
            .state_snapshots
            .iter()
            .map(project_state_snapshot)
            .collect(),
        beam_width: result.beam_width,
        portfolio: polygon_nesting_protocol::PortfolioResult {
            status: match result.portfolio.status {
                crate::result::IrregularPortfolioStatus::Completed => {
                    polygon_nesting_protocol::PortfolioStatus::Completed
                }
                crate::result::IrregularPortfolioStatus::Partial
                | crate::result::IrregularPortfolioStatus::Failed => {
                    polygon_nesting_protocol::PortfolioStatus::Completed
                }
            },
            termination_reason: match result.portfolio.termination_reason {
                crate::result::IrregularPortfolioTerminationReason::CapacitySubsetSettled => {
                    polygon_nesting_protocol::PortfolioTerminationReason::CapacitySubsetSettled
                }
                crate::result::IrregularPortfolioTerminationReason::SharedArchiveCompleted => {
                    polygon_nesting_protocol::PortfolioTerminationReason::SharedArchiveCompleted
                }
            },
            source: polygon_nesting_protocol::SearchSource::SharedArchive,
            placements: result
                .portfolio
                .placements
                .iter()
                .map(project_placement)
                .collect(),
            unplaced_piece_ids: result
                .portfolio
                .unplaced_piece_ids
                .iter()
                .map(|id| id.as_str().to_string())
                .collect(),
            score: project_score_summary(&result.portfolio.score),
            diagnostics: result
                .portfolio
                .diagnostics
                .iter()
                .map(project_diagnostic)
                .collect(),
        },
        capacity_trace: diagnostic_traces.0,
        intrinsic_anytime_scheduler_trace: diagnostic_traces.1,
        focused_complete_reconstruction_trace: diagnostic_traces.2,
        intrinsic_short_side_observer_trace: diagnostic_traces.3,
        intrinsic_short_side_pair_fold_trace: diagnostic_traces.4,
    }
}

fn project_layout_score(
    score: &crate::search::layout_scorer::IrregularLayoutScore,
) -> polygon_nesting_protocol::LayoutScore {
    polygon_nesting_protocol::LayoutScore {
        unplaced_count: score.unplaced_count,
        shared_collision_boundary_length_mm: score.shared_collision_boundary_length_mm,
        shared_collision_boundary_contact_units: score.shared_collision_boundary_contact_units,
        shared_collision_boundary_contact_band: score.shared_collision_boundary_contact_band,
        near_complete_structural_contact_count: score.near_complete_structural_contact_count,
        dominant_near_complete_structural_contact_count: score
            .dominant_near_complete_structural_contact_count,
        largest_net_free_material_region_area_mm2: score.largest_net_free_material_region_area_mm2,
        free_material_region_count: score.free_material_region_count,
        free_material_hole_count: score.free_material_hole_count,
        free_material_sliver_metric: score.free_material_sliver_metric,
        collision_bounds_worst_normalized_sheet_consumption: score
            .collision_bounds_worst_normalized_sheet_consumption,
        collision_bounds_normalized_span_sum: score.collision_bounds_normalized_span_sum,
        collision_bounds_area_mm2: score.collision_bounds_area_mm2,
        collision_bounds_span_mm: score.collision_bounds_span_mm,
        occupied_hull_waste_ratio: score.occupied_hull_waste_ratio,
        collision_bounds_bottom_mm: score.collision_bounds_bottom_mm,
        collision_bounds_left_mm: score.collision_bounds_left_mm,
        free_material_snapshot: project_free_material_snapshot(&score.free_material_snapshot),
        placement_order: score
            .placement_order
            .iter()
            .map(|id| id.as_str().to_string())
            .collect(),
        unplaced_source_piece_ids: score
            .unplaced_source_piece_ids
            .iter()
            .map(|id| id.as_str().to_string())
            .collect(),
    }
}

fn project_free_material_snapshot(
    snapshot: &crate::domain::FreeMaterialSnapshot,
) -> polygon_nesting_protocol::FreeMaterialSnapshot {
    polygon_nesting_protocol::FreeMaterialSnapshot {
        sheet: polygon_nesting_protocol::result::FreeMaterialSheet {
            width: snapshot.sheet.width,
            height: snapshot.sheet.height,
            label: snapshot.sheet.label.clone(),
        },
        regions: snapshot
            .regions
            .iter()
            .map(
                |region| polygon_nesting_protocol::result::FreeMaterialRegion {
                    boundary: project_polygon(&region.boundary),
                    holes: region.holes.iter().map(project_polygon).collect(),
                },
            )
            .collect(),
        diagnostics: snapshot
            .diagnostics
            .iter()
            .map(project_diagnostic)
            .collect(),
    }
}

fn project_capacity_trace(
    trace: &crate::capacity::mode::IntrinsicCapacityTrace,
) -> polygon_nesting_protocol::CapacityTrace {
    polygon_nesting_protocol::CapacityTrace {
        routing: match trace.routing {
            crate::capacity::mode::IntrinsicCapacityRouting::PreflightProvenImpossible => {
                polygon_nesting_protocol::result::CapacityRouting::PreflightProvenImpossible
            }
            crate::capacity::mode::IntrinsicCapacityRouting::BoundedCompleteArchiveMiss => {
                polygon_nesting_protocol::result::CapacityRouting::BoundedCompleteArchiveMiss
            }
        },
        preflight: project_capacity_preflight(&trace.preflight),
        prefixes: polygon_nesting_protocol::result::CapacityPrefixTrace {
            captured_count: trace.prefixes.captured_count,
            fitting_count: trace.prefixes.fitting_count,
            rejected_count: trace.prefixes.rejected_count,
            terminalized_count: trace.prefixes.terminalized_count,
            descriptors: trace
                .prefixes
                .descriptors
                .iter()
                .map(
                    |descriptor| polygon_nesting_protocol::result::CapacityPrefixDescriptor {
                        role: descriptor.role.clone(),
                        depth: descriptor.depth,
                    },
                )
                .collect(),
        },
        prefix_incumbent: trace.prefix_incumbent.as_ref().map(|incumbent| {
            polygon_nesting_protocol::result::CapacityIncumbentTrace {
                source_role: incumbent.source_role.clone(),
                prefix_depth: incumbent.prefix_depth,
                placed_count: incumbent.placed_count,
                placed_material_area_mm2: incumbent.placed_material_area_mm2,
                selected_rotation_deg: project_orthogonal_rotation(incumbent.selected_rotation_deg),
                canonical_geometry_hash: incumbent.canonical_geometry_hash.clone(),
            }
        }),
        cold_search: project_capacity_search_trace(&trace.cold_search),
        warm_prefix_lanes: trace.warm_prefix_lanes.as_ref().map(|lanes| {
            lanes
                .iter()
                .map(project_capacity_warm_prefix_lane)
                .collect()
        }),
        warm_prefix_endpoints_admitted: trace.warm_prefix_endpoints_admitted,
        cohesion_shadow: trace
            .cohesion_shadow
            .as_ref()
            .map(project_capacity_cohesion_shadow),
        quality_warm_prefix: trace
            .quality_warm_prefix
            .as_ref()
            .map(project_capacity_quality_warm_prefix),
        lane_coordinator: trace
            .lane_coordinator
            .as_ref()
            .map(project_capacity_lane_coordinator),
        selected: polygon_nesting_protocol::result::CapacitySelectionTrace {
            objective: project_capacity_objective(&trace.selected.objective),
            unplaced_count: trace.selected.unplaced_count,
            placed_material_area_mm2: trace.selected.placed_material_area_mm2,
            selected_rotation_deg: project_orthogonal_rotation(
                trace.selected.selected_rotation_deg,
            ),
        },
        preflight_runtime_ms: trace.preflight_runtime_ms,
        complete_archive_runtime_ms: trace.complete_archive_runtime_ms,
        prefix_terminalization_ms: trace.prefix_terminalization_ms,
        cold_search_ms: trace.cold_search_ms,
        runtime_ms: trace.runtime_ms,
    }
}

fn project_capacity_preflight(
    outcome: &crate::capacity::preflight::IntrinsicCapacityPreflightOutcome,
) -> polygon_nesting_protocol::result::CapacityPreflightOutcome {
    use crate::capacity::preflight::{
        IntrinsicCapacityPreflightOutcome, IntrinsicCapacityProvenImpossibleReason,
    };
    use polygon_nesting_protocol::result::{
        CapacityPreflightOutcome, InconclusiveKind, InconclusivePreflight,
        MinimumCollisionAreaExceedsSheetAreaPreflight, MinimumCollisionAreaExceedsSheetAreaReason,
        ProvenImpossibleKind, SingletonTransformSetDoesNotFitPreflight,
        SingletonTransformSetDoesNotFitReason,
    };

    match outcome {
        IntrinsicCapacityPreflightOutcome::ProvenImpossible {
            reason:
                IntrinsicCapacityProvenImpossibleReason::SingletonTransformSetDoesNotFit { piece_id },
            measurements,
        } => CapacityPreflightOutcome::SingletonTransformSetDoesNotFit(
            SingletonTransformSetDoesNotFitPreflight {
                kind: ProvenImpossibleKind::ProvenImpossible,
                reason: SingletonTransformSetDoesNotFitReason::SingletonTransformSetDoesNotFit,
                piece_id: piece_id.as_str().to_string(),
                measurements: project_capacity_preflight_measurements(measurements),
            },
        ),
        IntrinsicCapacityPreflightOutcome::ProvenImpossible {
            reason: IntrinsicCapacityProvenImpossibleReason::MinimumCollisionAreaExceedsSheetArea,
            measurements,
        } => CapacityPreflightOutcome::MinimumCollisionAreaExceedsSheetArea(
            MinimumCollisionAreaExceedsSheetAreaPreflight {
                kind: ProvenImpossibleKind::ProvenImpossible,
                reason:
                    MinimumCollisionAreaExceedsSheetAreaReason::MinimumCollisionAreaExceedsSheetArea,
                measurements: project_capacity_preflight_measurements(measurements),
            },
        ),
        IntrinsicCapacityPreflightOutcome::Inconclusive { measurements } => {
            CapacityPreflightOutcome::Inconclusive(InconclusivePreflight {
                kind: InconclusiveKind::Inconclusive,
                measurements: project_capacity_preflight_measurements(measurements),
            })
        }
    }
}

fn project_capacity_preflight_measurements(
    measurements: &crate::capacity::preflight::IntrinsicCapacityPreflightMeasurements,
) -> polygon_nesting_protocol::result::CapacityPreflightMeasurements {
    polygon_nesting_protocol::result::CapacityPreflightMeasurements {
        piece_count: measurements.piece_count,
        sheet_width_grid: measurements.sheet_width_grid,
        sheet_height_grid: measurements.sheet_height_grid,
        sheet_doubled_area_grid2: exact_decimal(measurements.sheet_doubled_area_grid2.to_string()),
        minimum_doubled_collision_area_sum_grid2: exact_decimal(
            measurements
                .minimum_doubled_collision_area_sum_grid2
                .to_string(),
        ),
        minimum_collision_area_pressure_ppm: exact_decimal(
            measurements.minimum_collision_area_pressure_ppm.to_string(),
        ),
        maximum_singleton_span_pressure_ppm: exact_decimal(
            measurements.maximum_singleton_span_pressure_ppm.to_string(),
        ),
        singleton_infeasible_piece_ids: measurements
            .singleton_infeasible_piece_ids
            .iter()
            .map(|id| id.as_str().to_string())
            .collect(),
    }
}

fn project_capacity_search_trace(
    trace: &crate::capacity::search::IntrinsicCapacitySearchTrace,
) -> polygon_nesting_protocol::result::CapacitySearchTrace {
    polygon_nesting_protocol::result::CapacitySearchTrace {
        beam_width: trace.beam_width,
        local_legal_placement_fanout: trace.local_legal_placement_fanout,
        placement_evaluation_cap: trace.placement_evaluation_cap,
        placement_evaluation_quota_per_depth: trace.placement_evaluation_quota_per_depth,
        consumed_placement_evaluations: trace.consumed_placement_evaluations,
        auxiliary_placement_evaluations: trace.auxiliary_placement_evaluations,
        pruned_by_attainable_count: trace.pruned_by_attainable_count,
        pruned_by_attainable_material: trace.pruned_by_attainable_material,
        deduplicated_successors: trace.deduplicated_successors,
        fit_rejected_candidates: trace.fit_rejected_candidates,
        invalid_candidates: trace.invalid_candidates,
        endpoint_fit_rejections: trace.endpoint_fit_rejections,
        completed_depths: trace.completed_depths,
        depth_quota_exhaustions: trace.depth_quota_exhaustions,
        piece_count: trace.piece_count,
        settlement: match trace.settlement {
            crate::capacity::search::IntrinsicCapacitySettlement::Exhausted => {
                polygon_nesting_protocol::result::CapacitySearchSettlement::Exhausted
            }
            crate::capacity::search::IntrinsicCapacitySettlement::EvaluationCap => {
                polygon_nesting_protocol::result::CapacitySearchSettlement::EvaluationCap
            }
            crate::capacity::search::IntrinsicCapacitySettlement::Paused => {
                polygon_nesting_protocol::result::CapacitySearchSettlement::Paused
            }
        },
        topology_retention_depths: trace.topology_retention_depths.as_ref().map(|depths| {
            depths
                .iter()
                .map(project_capacity_topology_retention_depth)
                .collect()
        }),
    }
}

fn project_capacity_topology_retention_depth(
    trace: &crate::capacity::search::IntrinsicCapacityTopologyRetentionDepthTrace,
) -> polygon_nesting_protocol::result::CapacityTopologyRetentionDepthTrace {
    polygon_nesting_protocol::result::CapacityTopologyRetentionDepthTrace {
        depth: trace.depth,
        piece_id: trace.piece_id.as_str().to_string(),
        measured_survivor_count: trace.measured_survivor_count,
        retained_count: trace.retained_count,
        best_accounting_stratum_count: trace.best_accounting_stratum_count,
        topology_measurement_count: trace.topology_measurement_count,
        topology_measurement_ms: trace.topology_measurement_ms,
        legal_candidate_count: trace.legal_candidate_count,
        contact_measured_candidate_count: trace.contact_measured_candidate_count,
        positive_contact_candidate_count: trace.positive_contact_candidate_count,
        contact_measurement_ms: trace.contact_measurement_ms,
        contact_selected_successor_count: trace.contact_selected_successor_count,
        contact_deduplicated_successor_count: trace.contact_deduplicated_successor_count,
        contact_retained_successor_count: trace.contact_retained_successor_count,
        representatives: trace
            .representatives
            .iter()
            .map(project_capacity_topology_representative)
            .collect(),
    }
}

fn project_capacity_topology_representative(
    representative: &crate::capacity::search::IntrinsicCapacityTopologyRepresentative,
) -> polygon_nesting_protocol::result::CapacityTopologyRepresentative {
    use crate::capacity::search::{
        IntrinsicCapacityDecision, IntrinsicCapacityProposalRole,
        IntrinsicCapacityTopologyRepresentativeRole,
    };
    use polygon_nesting_protocol::result::{
        CapacityDecision, CapacityProposalRole, CapacityTopologyRepresentativeRole,
    };

    polygon_nesting_protocol::result::CapacityTopologyRepresentative {
        role: match representative.role {
            IntrinsicCapacityTopologyRepresentativeRole::TerminalObjective => {
                CapacityTopologyRepresentativeRole::TerminalObjective
            }
            IntrinsicCapacityTopologyRepresentativeRole::MinimumComponents => {
                CapacityTopologyRepresentativeRole::MinimumComponents
            }
            IntrinsicCapacityTopologyRepresentativeRole::MinimumIsolated => {
                CapacityTopologyRepresentativeRole::MinimumIsolated
            }
            IntrinsicCapacityTopologyRepresentativeRole::MaximumLargestComponent => {
                CapacityTopologyRepresentativeRole::MaximumLargestComponent
            }
            IntrinsicCapacityTopologyRepresentativeRole::MinimumHullWaste => {
                CapacityTopologyRepresentativeRole::MinimumHullWaste
            }
        },
        decision_identity: representative.decision_identity.clone(),
        parent_decision_identity: representative.parent_decision_identity.clone(),
        decision: match representative.decision {
            IntrinsicCapacityDecision::Place => CapacityDecision::Place,
            IntrinsicCapacityDecision::Skip => CapacityDecision::Skip,
        },
        proposal_role: match representative.proposal_role {
            IntrinsicCapacityProposalRole::Compactness => CapacityProposalRole::Compactness,
            IntrinsicCapacityProposalRole::Contact => CapacityProposalRole::Contact,
            IntrinsicCapacityProposalRole::Skip => CapacityProposalRole::Skip,
        },
        piece_id: representative.piece_id.as_str().to_string(),
        anchored_occupied_key: representative.anchored_occupied_key.clone(),
        placed_count: representative.placed_count,
        placed_doubled_material_area_grid2: exact_decimal(
            representative
                .placed_doubled_material_area_grid2
                .to_string(),
        ),
        cavities: project_capacity_cavity_metrics(&representative.cavities),
        grid_span: polygon_nesting_protocol::result::CapacityGridSpan {
            width_grid: representative.grid_span.width_grid,
            height_grid: representative.grid_span.height_grid,
        },
        topology: representative
            .topology
            .as_ref()
            .map(project_canonical_layout_topology_exact),
        retained: representative.retained,
    }
}

fn project_capacity_cavity_metrics(
    metrics: &crate::capacity::endpoint::IntrinsicCapacityCavityMetrics,
) -> polygon_nesting_protocol::result::CapacityCavityMetrics {
    polygon_nesting_protocol::result::CapacityCavityMetrics {
        count: metrics.count,
        total_area_mm2: metrics.total_area_mm2,
        total_doubled_area_grid2: exact_decimal(metrics.total_doubled_area_grid2.clone()),
    }
}

fn project_canonical_layout_topology_exact(
    exact: &crate::canonical_grid::layout::CanonicalLayoutTopologyExact,
) -> polygon_nesting_protocol::result::CanonicalLayoutTopologyExact {
    polygon_nesting_protocol::result::CanonicalLayoutTopologyExact {
        topology: polygon_nesting_protocol::result::CanonicalLayoutTopology {
            enclosed_cavity_count: exact.topology.enclosed_cavity_count,
            largest_occupied_hull_gap_ratio: exact.topology.largest_occupied_hull_gap_ratio,
            occupied_envelope_aspect_ratio: exact.topology.occupied_envelope_aspect_ratio,
            positive_contact_component_count: exact.topology.positive_contact_component_count,
            isolated_piece_count: exact.topology.isolated_piece_count,
            largest_positive_contact_component_size: exact
                .topology
                .largest_positive_contact_component_size,
            largest_positive_contact_component_ratio: exact
                .topology
                .largest_positive_contact_component_ratio,
        },
        hull_gap_doubled_area_grid2: exact.hull_gap_doubled_area_grid2,
        hull_doubled_area_grid2: exact.hull_doubled_area_grid2,
        exact_hull_gap_doubled_area_grid2: exact_decimal(
            exact.exact_hull_gap_doubled_area_grid2.clone(),
        ),
        exact_hull_doubled_area_grid2: exact_decimal(exact.exact_hull_doubled_area_grid2.clone()),
    }
}

fn project_capacity_objective(
    objective: &crate::capacity::endpoint::IntrinsicCapacityObjective,
) -> polygon_nesting_protocol::result::CapacityObjective {
    polygon_nesting_protocol::result::CapacityObjective {
        placed_count: objective.placed_count,
        placed_doubled_material_area_grid2: exact_decimal(
            objective.placed_doubled_material_area_grid2.to_string(),
        ),
        enclosed_cavity_count: objective.enclosed_cavity_count,
        total_enclosed_cavity_area_mm2: objective.total_enclosed_cavity_area_mm2,
        total_enclosed_cavity_doubled_area_grid2: exact_decimal(
            objective.total_enclosed_cavity_doubled_area_grid2.clone(),
        ),
        envelope_maximum_side_mm: objective.envelope_maximum_side_mm,
        envelope_area_mm2: objective.envelope_area_mm2,
        envelope_span_mm: objective.envelope_span_mm,
        envelope_maximum_side_grid: objective.envelope_maximum_side_grid,
        envelope_area_grid2: exact_decimal(objective.envelope_area_grid2.clone()),
        envelope_span_grid: objective.envelope_span_grid,
        canonical_geometry_hash: objective.canonical_geometry_hash.clone(),
        origin: match objective.origin {
            crate::capacity::endpoint::IntrinsicCapacityEndpointOrigin::ColdSearch => {
                polygon_nesting_protocol::result::CapacityEndpointOrigin::ColdSearch
            }
            crate::capacity::endpoint::IntrinsicCapacityEndpointOrigin::PrefixIncumbent => {
                polygon_nesting_protocol::result::CapacityEndpointOrigin::PrefixIncumbent
            }
            crate::capacity::endpoint::IntrinsicCapacityEndpointOrigin::WarmPrefixContinuation => {
                polygon_nesting_protocol::result::CapacityEndpointOrigin::WarmPrefixContinuation
            }
        },
        prefix_depth: objective.prefix_depth,
        source_role: objective.source_role.clone(),
    }
}

fn project_capacity_warm_prefix_lane(
    lane: &crate::capacity::mode::IntrinsicCapacityWarmPrefixLaneTrace,
) -> polygon_nesting_protocol::result::CapacityWarmPrefixLaneTrace {
    polygon_nesting_protocol::result::CapacityWarmPrefixLaneTrace {
        source_role: lane.source_role.clone(),
        prefix_depth: lane.prefix_depth,
        reused_placed_count: lane.reused_placed_count,
        status: match lane.status {
            crate::capacity::mode::WarmPrefixLaneStatus::Settled => {
                polygon_nesting_protocol::result::CapacityWarmPrefixStatus::Settled
            }
            crate::capacity::mode::WarmPrefixLaneStatus::CheckpointedCensored => {
                polygon_nesting_protocol::result::CapacityWarmPrefixStatus::CheckpointedCensored
            }
        },
        selected_for_continuation: lane.selected_for_continuation,
        checkpoint_retained: lane.checkpoint_retained,
        consumed_placement_evaluations: lane.consumed_placement_evaluations,
        completed_depths: lane.completed_depths,
        elapsed_ms: lane.elapsed_ms,
        endpoint: lane.endpoint.as_ref().map(project_capacity_objective),
    }
}

fn project_capacity_cohesion_shadow(
    trace: &crate::capacity::mode::IntrinsicCapacityCohesionShadowTrace,
) -> polygon_nesting_protocol::result::CapacityCohesionShadowTrace {
    project_capacity_cohesion_shadow_literals(
        trace.producer_role,
        trace.status,
        trace.output_influence,
    );
    polygon_nesting_protocol::result::CapacityCohesionShadowTrace {
        producer_role:
            polygon_nesting_protocol::result::CapacityCohesionShadowProducerRole::CapacityCohesionShadow,
        status: polygon_nesting_protocol::result::SettledStatus::Settled,
        output_influence: polygon_nesting_protocol::result::NoOutputInfluence::None,
        consumed_placement_evaluations: trace.consumed_placement_evaluations,
        completed_depths: trace.completed_depths,
        elapsed_ms: trace.elapsed_ms,
        endpoint: trace.endpoint.as_ref().map(project_capacity_objective),
        retention_depths: trace.retention_depths.as_ref().map(|depths| {
            depths
                .iter()
                .map(project_capacity_topology_retention_depth)
                .collect()
        }),
    }
}

fn project_capacity_cohesion_shadow_literals(
    producer_role: &str,
    status: &str,
    output_influence: &str,
) {
    match (producer_role, status, output_influence) {
        ("capacity-cohesion-shadow", "settled", "none") => {}
        _ => unreachable!("invalid intrinsic capacity cohesion-shadow literals"),
    }
}

fn project_capacity_quality_warm_prefix(
    trace: &crate::capacity::mode::IntrinsicCapacityQualityWarmPrefixTrace,
) -> polygon_nesting_protocol::result::CapacityQualityWarmPrefixTrace {
    match (trace.version, trace.producer_role, trace.policy) {
        (
            "intrinsic-capacity-quality-warm-prefix-v1",
            "capacity-quality-warm-prefix",
            "quality-frontier",
        ) => {}
        _ => unreachable!("invalid intrinsic capacity quality warm-prefix literals"),
    }
    polygon_nesting_protocol::result::CapacityQualityWarmPrefixTrace {
        version: polygon_nesting_protocol::result::CapacityQualityWarmPrefixVersion::V1,
        producer_role:
            polygon_nesting_protocol::result::CapacityQualityWarmPrefixProducerRole::CapacityQualityWarmPrefix,
        policy:
            polygon_nesting_protocol::result::CapacityQualityWarmPrefixPolicy::QualityFrontier,
        status: match trace.status {
            crate::capacity::mode::QualityWarmPrefixStatus::SkippedBelowMinimumPieceCount => {
                polygon_nesting_protocol::result::CapacityQualityWarmPrefixStatus::SkippedBelowMinimumPieceCount
            }
            crate::capacity::mode::QualityWarmPrefixStatus::SkippedNoFittingCanonicalPrefix => {
                polygon_nesting_protocol::result::CapacityQualityWarmPrefixStatus::SkippedNoFittingCanonicalPrefix
            }
            crate::capacity::mode::QualityWarmPrefixStatus::Settled => {
                polygon_nesting_protocol::result::CapacityQualityWarmPrefixStatus::Settled
            }
            crate::capacity::mode::QualityWarmPrefixStatus::EvaluationCap => {
                polygon_nesting_protocol::result::CapacityQualityWarmPrefixStatus::EvaluationCap
            }
            crate::capacity::mode::QualityWarmPrefixStatus::CheckpointedCensored => {
                polygon_nesting_protocol::result::CapacityQualityWarmPrefixStatus::CheckpointedCensored
            }
        },
        output_influence: match trace.output_influence {
            crate::capacity::mode::QualityWarmPrefixOutputInfluence::None => {
                polygon_nesting_protocol::result::CapacityQualityWarmPrefixOutputInfluence::None
            }
            crate::capacity::mode::QualityWarmPrefixOutputInfluence::StrictCountImprovement => {
                polygon_nesting_protocol::result::CapacityQualityWarmPrefixOutputInfluence::StrictCountImprovement
            }
        },
        source_role: trace
            .source_role
            .as_deref()
            .map(project_canonical_grid_source_role),
        prefix_depth: trace.prefix_depth,
        reused_placed_count: trace.reused_placed_count,
        request_piece_count: trace.request_piece_count,
        minimum_piece_count: trace.minimum_piece_count,
        placement_evaluation_cap: trace.placement_evaluation_cap,
        consumed_placement_evaluations: trace.consumed_placement_evaluations,
        completed_depths: trace.completed_depths,
        checkpoint_retained: trace.checkpoint_retained,
        elapsed_ms: trace.elapsed_ms,
        endpoint: trace.endpoint.as_ref().map(project_capacity_objective),
    }
}

fn project_capacity_lane_coordinator(
    trace: &crate::capacity::mode::IntrinsicCapacityLaneCoordinatorTrace,
) -> polygon_nesting_protocol::result::CapacityLaneCoordinatorTrace {
    match trace.version {
        "intrinsic-capacity-lane-coordinator-v3" => {}
        _ => unreachable!("invalid intrinsic capacity lane-coordinator version"),
    }
    polygon_nesting_protocol::result::CapacityLaneCoordinatorTrace {
        version: polygon_nesting_protocol::result::CapacityLaneCoordinatorVersion::V3,
        aggregate_placement_evaluation_cap: trace.aggregate_placement_evaluation_cap,
        aggregate_consumed_placement_evaluations: trace
            .aggregate_consumed_placement_evaluations,
        warm_pilot_depth_boundaries: trace.warm_pilot_depth_boundaries,
        continued_producers: trace
            .continued_producers
            .iter()
            .map(|producer| match producer {
                crate::capacity::mode::LaneCoordinatorContinuedProducer::CapacityCold => {
                    polygon_nesting_protocol::result::CapacityContinuedProducer::CapacityCold
                }
                crate::capacity::mode::LaneCoordinatorContinuedProducer::CapacityWarmPrefix {
                    source_role,
                    prefix_depth,
                } => polygon_nesting_protocol::result::CapacityContinuedProducer::CapacityWarmPrefix {
                    source_role: source_role.clone(),
                    prefix_depth: *prefix_depth,
                },
                crate::capacity::mode::LaneCoordinatorContinuedProducer::CapacityQualityWarmPrefix {
                    source_role,
                    prefix_depth,
                } => polygon_nesting_protocol::result::CapacityContinuedProducer::CapacityQualityWarmPrefix {
                    source_role: project_canonical_grid_source_role(source_role),
                    prefix_depth: *prefix_depth,
                },
            })
            .collect(),
        retained_checkpoint_count: trace.retained_checkpoint_count,
        censored_lane_count: trace.censored_lane_count,
        quanta: trace
            .quanta
            .iter()
            .map(|quantum| polygon_nesting_protocol::result::CapacityLaneCoordinatorQuantum {
                ordinal: quantum.ordinal,
                producer_role: match quantum.producer_role {
                    crate::capacity::mode::LaneCoordinatorQuantumProducerRole::CapacityCold => polygon_nesting_protocol::result::CapacityCoordinatorProducerRole::CapacityCold,
                    crate::capacity::mode::LaneCoordinatorQuantumProducerRole::CapacityQualityWarmPrefix => polygon_nesting_protocol::result::CapacityCoordinatorProducerRole::CapacityQualityWarmPrefix,
                    crate::capacity::mode::LaneCoordinatorQuantumProducerRole::CapacityWarmPrefix => polygon_nesting_protocol::result::CapacityCoordinatorProducerRole::CapacityWarmPrefix,
                },
                source_role: quantum.source_role.clone(),
                prefix_depth: quantum.prefix_depth,
                phase: match quantum.phase {
                    crate::capacity::mode::LaneCoordinatorQuantumPhase::Initial => polygon_nesting_protocol::result::CapacityCoordinatorPhase::Initial,
                    crate::capacity::mode::LaneCoordinatorQuantumPhase::Resume => polygon_nesting_protocol::result::CapacityCoordinatorPhase::Resume,
                    crate::capacity::mode::LaneCoordinatorQuantumPhase::Censor => polygon_nesting_protocol::result::CapacityCoordinatorPhase::Censor,
                },
                from_depth: quantum.from_depth,
                to_depth: quantum.to_depth,
                placement_evaluation_delta: quantum.placement_evaluation_delta,
                outcome: match quantum.outcome {
                    crate::capacity::mode::LaneCoordinatorQuantumOutcome::Checkpointed => polygon_nesting_protocol::result::CapacityCoordinatorOutcome::Checkpointed,
                    crate::capacity::mode::LaneCoordinatorQuantumOutcome::Settled => polygon_nesting_protocol::result::CapacityCoordinatorOutcome::Settled,
                    crate::capacity::mode::LaneCoordinatorQuantumOutcome::Censored => polygon_nesting_protocol::result::CapacityCoordinatorOutcome::Censored,
                },
            })
            .collect(),
    }
}

fn project_canonical_grid_source_role(
    source_role: &str,
) -> polygon_nesting_protocol::result::CanonicalGridSourceRole {
    match source_role {
        "canonical-grid" => {
            polygon_nesting_protocol::result::CanonicalGridSourceRole::CanonicalGrid
        }
        _ => unreachable!("invalid canonical-grid source role"),
    }
}

fn project_orthogonal_rotation(
    rotation_deg: f64,
) -> polygon_nesting_protocol::result::OrthogonalRotation {
    match rotation_deg {
        0.0 => polygon_nesting_protocol::result::OrthogonalRotation::Deg0,
        90.0 => polygon_nesting_protocol::result::OrthogonalRotation::Deg90,
        _ => unreachable!("intrinsic capacity rotation must be 0 or 90 degrees"),
    }
}

fn exact_decimal(value: String) -> polygon_nesting_protocol::result::ExactDecimalString {
    polygon_nesting_protocol::result::ExactDecimalString::new(value)
        .expect("intrinsic capacity exact decimals must be canonical")
}

fn project_scheduler_trace(
    trace: &crate::result::IntrinsicAnytimeSchedulerTrace,
) -> polygon_nesting_protocol::IntrinsicAnytimeSchedulerTrace {
    polygon_nesting_protocol::IntrinsicAnytimeSchedulerTrace {
        version: polygon_nesting_protocol::result::IntrinsicAnytimeSchedulerVersion::V1,
        cold_quantum_depths: trace.cold_quantum_depths,
        cold_start_status: match trace.cold_start_status {
            crate::result::IntrinsicAnytimeSchedulerColdStartStatus::Paused => {
                polygon_nesting_protocol::result::IntrinsicAnytimeSchedulerColdStartStatus::Paused
            }
            crate::result::IntrinsicAnytimeSchedulerColdStartStatus::Settled => {
                polygon_nesting_protocol::result::IntrinsicAnytimeSchedulerColdStartStatus::Settled
            }
        },
        cold_start_completed_depths: trace.cold_start_completed_depths,
        cold_start_consumed_placement_evaluations: trace
            .cold_start_consumed_placement_evaluations,
        cold_checkpoint_reused: trace.cold_checkpoint_reused,
        warm_prefix_endpoints_admitted: trace.warm_prefix_endpoints_admitted,
        cancellation_reason: trace.cancellation_reason.map(|reason| match reason {
            crate::result::IntrinsicAnytimeSchedulerCancellationReason::CompleteEndpointFitted => {
                polygon_nesting_protocol::result::IntrinsicAnytimeSchedulerCancellationReason::CompleteEndpointFitted
            }
            crate::result::IntrinsicAnytimeSchedulerCancellationReason::CompleteCohortMiss => {
                polygon_nesting_protocol::result::IntrinsicAnytimeSchedulerCancellationReason::CompleteCohortMiss
            }
        }),
        quanta: trace
            .quanta
            .iter()
            .map(|quantum| polygon_nesting_protocol::result::IntrinsicAnytimeSchedulerQuantum {
                ordinal: quantum.ordinal,
                cohort: match quantum.cohort {
                    crate::result::IntrinsicAnytimeSchedulerCohort::Partial => polygon_nesting_protocol::result::IntrinsicAnytimeSchedulerCohort::Partial,
                    crate::result::IntrinsicAnytimeSchedulerCohort::Complete => polygon_nesting_protocol::result::IntrinsicAnytimeSchedulerCohort::Complete,
                    crate::result::IntrinsicAnytimeSchedulerCohort::ExperimentalComplete => polygon_nesting_protocol::result::IntrinsicAnytimeSchedulerCohort::ExperimentalComplete,
                },
                producer_role: match quantum.producer_role {
                    crate::result::IntrinsicAnytimeSchedulerProducerRole::CapacityCold => polygon_nesting_protocol::result::IntrinsicAnytimeSchedulerProducerRole::CapacityCold,
                    crate::result::IntrinsicAnytimeSchedulerProducerRole::CapacityQualityWarmPrefix => polygon_nesting_protocol::result::IntrinsicAnytimeSchedulerProducerRole::CapacityQualityWarmPrefix,
                    crate::result::IntrinsicAnytimeSchedulerProducerRole::LegacyComplete => polygon_nesting_protocol::result::IntrinsicAnytimeSchedulerProducerRole::LegacyComplete,
                    crate::result::IntrinsicAnytimeSchedulerProducerRole::CapacityWarmPrefix => polygon_nesting_protocol::result::IntrinsicAnytimeSchedulerProducerRole::CapacityWarmPrefix,
                    crate::result::IntrinsicAnytimeSchedulerProducerRole::ExperimentalPlaceDeferComplete => polygon_nesting_protocol::result::IntrinsicAnytimeSchedulerProducerRole::ExperimentalPlaceDeferComplete,
                },
                outcome: match quantum.outcome {
                    crate::result::IntrinsicAnytimeSchedulerOutcome::Checkpointed => polygon_nesting_protocol::result::IntrinsicAnytimeSchedulerOutcome::Checkpointed,
                    crate::result::IntrinsicAnytimeSchedulerOutcome::Settled => polygon_nesting_protocol::result::IntrinsicAnytimeSchedulerOutcome::Settled,
                    crate::result::IntrinsicAnytimeSchedulerOutcome::Cancelled => polygon_nesting_protocol::result::IntrinsicAnytimeSchedulerOutcome::Cancelled,
                    crate::result::IntrinsicAnytimeSchedulerOutcome::Censored => polygon_nesting_protocol::result::IntrinsicAnytimeSchedulerOutcome::Censored,
                },
            })
            .collect(),
    }
}

fn project_focused_reconstruction_trace(
    trace: &crate::result::IntrinsicFocusedCompleteReconstructionTrace,
) -> polygon_nesting_protocol::FocusedCompleteReconstructionTrace {
    polygon_nesting_protocol::FocusedCompleteReconstructionTrace {
        version: polygon_nesting_protocol::result::FocusedCompleteReconstructionVersion::V1,
        status: match trace.status {
            crate::result::IntrinsicFocusedCompleteReconstructionStatus::Completed => polygon_nesting_protocol::result::FocusedCompleteReconstructionStatus::Completed,
            crate::result::IntrinsicFocusedCompleteReconstructionStatus::DuplicateOrder => polygon_nesting_protocol::result::FocusedCompleteReconstructionStatus::DuplicateOrder,
            crate::result::IntrinsicFocusedCompleteReconstructionStatus::EvaluationCap => polygon_nesting_protocol::result::FocusedCompleteReconstructionStatus::EvaluationCap,
            crate::result::IntrinsicFocusedCompleteReconstructionStatus::Deadline => polygon_nesting_protocol::result::FocusedCompleteReconstructionStatus::Deadline,
            crate::result::IntrinsicFocusedCompleteReconstructionStatus::Incomplete => polygon_nesting_protocol::result::FocusedCompleteReconstructionStatus::Incomplete,
            crate::result::IntrinsicFocusedCompleteReconstructionStatus::FailedProtectedFallback => polygon_nesting_protocol::result::FocusedCompleteReconstructionStatus::FailedProtectedFallback,
            crate::result::IntrinsicFocusedCompleteReconstructionStatus::SkippedPreflightProvenImpossible => polygon_nesting_protocol::result::FocusedCompleteReconstructionStatus::SkippedPreflightProvenImpossible,
            crate::result::IntrinsicFocusedCompleteReconstructionStatus::SkippedNoFittingProtectedEndpoint => polygon_nesting_protocol::result::FocusedCompleteReconstructionStatus::SkippedNoFittingProtectedEndpoint,
        },
        source_canonical_geometry_hash: trace.source_canonical_geometry_hash.clone(),
        candidate_canonical_geometry_hash: trace.candidate_canonical_geometry_hash.clone(),
        selected_canonical_geometry_hash: trace.selected_canonical_geometry_hash.clone(),
        consumed_candidate_evaluations: trace.consumed_candidate_evaluations,
        candidate_evaluation_accounting_complete: trace.candidate_evaluation_accounting_complete,
        runtime_ms: trace.runtime_ms,
        output_influence: match trace.output_influence {
            crate::result::IntrinsicFocusedCompleteReconstructionOutputInfluence::Selected => polygon_nesting_protocol::result::FocusedCompleteReconstructionOutputInfluence::Selected,
            crate::result::IntrinsicFocusedCompleteReconstructionOutputInfluence::ProtectedFallback => polygon_nesting_protocol::result::FocusedCompleteReconstructionOutputInfluence::ProtectedFallback,
            crate::result::IntrinsicFocusedCompleteReconstructionOutputInfluence::None => polygon_nesting_protocol::result::FocusedCompleteReconstructionOutputInfluence::None,
        },
        failure_reason: trace.failure_reason.clone(),
    }
}

fn project_short_side_observer_trace(
    trace: &crate::short_side::observer::IntrinsicShortSideObserverTrace,
) -> polygon_nesting_protocol::IntrinsicShortSideObserverTrace {
    use crate::short_side::observer::{IntrinsicShortSideObserverStatus, ShortSideOutputInfluence};
    use polygon_nesting_protocol::result::{
        IntrinsicShortSideObserverVersion, SheetAxis, ShortSideObserverStatus,
        ShortSideOutputInfluence as ProtocolOutputInfluence,
    };

    match trace.version {
        "intrinsic-short-side-observer-v6" => {}
        _ => unreachable!("invalid intrinsic short-side observer version"),
    }
    polygon_nesting_protocol::IntrinsicShortSideObserverTrace {
        version: IntrinsicShortSideObserverVersion::V6,
        status: match trace.status {
            IntrinsicShortSideObserverStatus::Observed => ShortSideObserverStatus::Observed,
            IntrinsicShortSideObserverStatus::ObservedNoLegalOrientation => {
                ShortSideObserverStatus::ObservedNoLegalOrientation
            }
            IntrinsicShortSideObserverStatus::ObservedNoGuardEligibleEndpoint => {
                ShortSideObserverStatus::ObservedNoGuardEligibleEndpoint
            }
            IntrinsicShortSideObserverStatus::ObservedNoDirectionalImprovement => {
                ShortSideObserverStatus::ObservedNoDirectionalImprovement
            }
            IntrinsicShortSideObserverStatus::SkippedNoSettledCompleteEndpoints => {
                ShortSideObserverStatus::SkippedNoSettledCompleteEndpoints
            }
            IntrinsicShortSideObserverStatus::RuntimeBudgetExceeded => {
                ShortSideObserverStatus::RuntimeBudgetExceeded
            }
            IntrinsicShortSideObserverStatus::TraceBudgetExceeded => {
                ShortSideObserverStatus::TraceBudgetExceeded
            }
        },
        output_influence: match trace.output_influence {
            ShortSideOutputInfluence::None => ProtocolOutputInfluence::None,
            ShortSideOutputInfluence::Selected => ProtocolOutputInfluence::Selected,
        },
        requested_sheet_width_mm: trace.requested_sheet_width_mm,
        requested_sheet_height_mm: trace.requested_sheet_height_mm,
        requested_long_axis_mm: trace.requested_long_axis_mm,
        requested_short_axis_mm: trace.requested_short_axis_mm,
        requested_long_axis: match trace.requested_long_axis {
            crate::short_side::axes::ShortSideAxisDimension::Width => SheetAxis::Width,
            crate::short_side::axes::ShortSideAxisDimension::Height => SheetAxis::Height,
        },
        production_short_axis_span_mm: trace.production_short_axis_span_mm,
        production_maximum_side_mm: trace.production_maximum_side_mm,
        production_envelope_area_mm2: trace.production_envelope_area_mm2,
        production_short_axis_span_grid: trace.production_short_axis_span_grid,
        production_maximum_side_grid: trace.production_maximum_side_grid,
        production_envelope_area_grid2: trace
            .production_envelope_area_grid2
            .as_ref()
            .map(|value| exact_decimal(value.clone())),
        settled_endpoint_count: trace.settled_endpoint_count,
        evaluated_orientation_count: trace.evaluated_orientation_count,
        cavity_hull_guard_eligible_endpoint_count: trace.cavity_hull_guard_eligible_endpoint_count,
        geometric_pareto_eligible_endpoint_count: trace.geometric_pareto_eligible_endpoint_count,
        placement_evaluations: trace.placement_evaluations,
        candidate_evaluations: trace.candidate_evaluations,
        runtime_ms: trace.runtime_ms,
        runtime_budget_exceeded: trace.runtime_budget_exceeded,
        serialized_trace_bytes: trace.serialized_trace_bytes,
        endpoints: trace
            .endpoints
            .iter()
            .map(project_short_side_endpoint_observation)
            .collect(),
        ranked_canonical_geometry_hashes: trace.ranked_canonical_geometry_hashes.clone(),
        directional_admission_terms: trace.directional_admission_terms.as_ref().map(|terms| {
            polygon_nesting_protocol::result::ShortSideDirectionalAdmissionTerms {
                short_edge_fill_admitted: terms.short_edge_fill_admitted,
                shortfall_halved: terms.shortfall_halved,
                depth_within_production_maximum_side: terms.depth_within_production_maximum_side,
                envelope_area_cost_within_production_bound: terms
                    .envelope_area_cost_within_production_bound,
            }
        }),
        observer_winner_canonical_geometry_hash: trace
            .observer_winner_canonical_geometry_hash
            .clone(),
        observer_winner_rotation_deg: trace
            .observer_winner_rotation_deg
            .map(project_short_side_rotation),
    }
}

fn project_short_side_endpoint_observation(
    observation: &crate::short_side::observer::IntrinsicShortSideEndpointObservation,
) -> polygon_nesting_protocol::result::ShortSideEndpointObservation {
    polygon_nesting_protocol::result::ShortSideEndpointObservation {
        archive_index: observation.archive_index as f64,
        role: observation.role.clone(),
        source_id: observation.source_id.clone(),
        canonical_geometry_hash: observation.canonical_geometry_hash.clone(),
        q0: project_short_side_orientation_observation(&observation.q0),
        q90: project_short_side_orientation_observation(&observation.q90),
        selected_rotation_deg: project_short_side_rotation(observation.selected_rotation_deg),
        selected: project_short_side_orientation_observation(&observation.selected),
        cavity_hull_guard_eligible: observation.cavity_hull_guard_eligible,
        geometric_pareto_eligible: observation.geometric_pareto_eligible,
    }
}

fn project_short_side_orientation_observation(
    observation: &crate::short_side::observer::IntrinsicShortSideOrientationObservation,
) -> polygon_nesting_protocol::result::ShortSideOrientationObservation {
    polygon_nesting_protocol::result::ShortSideOrientationObservation {
        rotation_deg: project_short_side_rotation(observation.rotation_deg),
        exact_legal: observation.exact_legal,
        canonical_geometry_hash: observation.canonical_geometry_hash.clone(),
        used_width_mm: observation.used_width_mm,
        used_height_mm: observation.used_height_mm,
        requested_long_axis_used_span_mm: observation.requested_long_axis_used_span_mm,
        requested_short_axis_shortfall_mm: observation.requested_short_axis_shortfall_mm,
        requested_long_axis_used_span_grid: observation.requested_long_axis_used_span_grid,
        requested_short_axis_shortfall_grid: observation.requested_short_axis_shortfall_grid,
        cavity_count: observation.cavity_count,
        hull_gap_ratio: observation.hull_gap_ratio,
        hull_gap_doubled_area_grid2: observation
            .hull_gap_doubled_area_grid2
            .as_ref()
            .map(|value| exact_decimal(value.clone())),
        occupied_hull_doubled_area_grid2: observation
            .occupied_hull_doubled_area_grid2
            .as_ref()
            .map(|value| exact_decimal(value.clone())),
        cohesion_passes: observation.cohesion_passes,
        cohesion_deficit: observation.cohesion_deficit,
        cohesion_deficit_numerator: observation
            .cohesion_deficit_numerator
            .as_ref()
            .map(|value| exact_decimal(value.clone())),
        cohesion_deficit_denominator: observation
            .cohesion_deficit_denominator
            .as_ref()
            .map(|value| exact_decimal(value.clone())),
        intrinsic_envelope_area_mm2: observation.intrinsic_envelope_area_mm2,
        intrinsic_envelope_maximum_side_mm: observation.intrinsic_envelope_maximum_side_mm,
        intrinsic_envelope_span_mm: observation.intrinsic_envelope_span_mm,
        intrinsic_envelope_area_grid2: observation
            .intrinsic_envelope_area_grid2
            .as_ref()
            .map(|value| exact_decimal(value.clone())),
        intrinsic_envelope_maximum_side_grid: observation.intrinsic_envelope_maximum_side_grid,
        intrinsic_envelope_span_grid: observation.intrinsic_envelope_span_grid,
        dominant_structural_contacts: observation.dominant_structural_contacts,
        total_structural_contacts: observation.total_structural_contacts,
        contact_units: observation.contact_units,
        shared_boundary_length_mm: observation.shared_boundary_length_mm,
        comparison_tuple: observation
            .comparison_tuple
            .iter()
            .map(|entry| match entry {
                crate::short_side::observer::ComparisonTupleEntry::Num(value) => {
                    polygon_nesting_protocol::result::ShortSideComparisonValue::Number(*value)
                }
                crate::short_side::observer::ComparisonTupleEntry::Str(value) => {
                    polygon_nesting_protocol::result::ShortSideComparisonValue::Text(value.clone())
                }
            })
            .collect(),
    }
}

fn project_short_side_rotation(
    rotation: crate::short_side::axes::ShortSideRotationDeg,
) -> polygon_nesting_protocol::result::OrthogonalRotation {
    match rotation {
        crate::short_side::axes::ShortSideRotationDeg::Zero => {
            polygon_nesting_protocol::result::OrthogonalRotation::Deg0
        }
        crate::short_side::axes::ShortSideRotationDeg::Ninety => {
            polygon_nesting_protocol::result::OrthogonalRotation::Deg90
        }
    }
}

fn project_short_side_pair_fold_trace(
    trace: &crate::short_side::pair_fold::IntrinsicShortSidePairFoldTrace,
) -> polygon_nesting_protocol::IntrinsicShortSidePairFoldTrace {
    use polygon_nesting_protocol::result::{
        IntrinsicShortSidePairFoldVersion, ShortSideExecutionModel,
    };

    match trace.version {
        "intrinsic-short-side-terminal-observer-v6" => {}
        _ => unreachable!("invalid intrinsic short-side pair-fold version"),
    }
    match trace.execution_model {
        "single-process-sequential" => {}
        _ => unreachable!("invalid intrinsic short-side pair-fold execution model"),
    }
    polygon_nesting_protocol::IntrinsicShortSidePairFoldTrace {
        version: IntrinsicShortSidePairFoldVersion::V6,
        status: project_short_side_pair_fold_status(trace.status),
        output_influence: match trace.output_influence {
            crate::short_side::observer::ShortSideOutputInfluence::None => {
                polygon_nesting_protocol::result::ShortSideOutputInfluence::None
            }
            crate::short_side::observer::ShortSideOutputInfluence::Selected => {
                polygon_nesting_protocol::result::ShortSideOutputInfluence::Selected
            }
        },
        execution_model: ShortSideExecutionModel::SingleProcessSequential,
        requested_short_axis_mm: trace.requested_short_axis_mm,
        requested_long_axis_mm: trace.requested_long_axis_mm,
        prescribed_rotation_deg: trace
            .prescribed_rotation_deg
            .map(project_short_side_rotation),
        production_short_axis_span_mm: trace.production_short_axis_span_mm,
        production_maximum_side_mm: trace.production_maximum_side_mm,
        production_envelope_area_mm2: trace.production_envelope_area_mm2,
        production_short_axis_span_grid: trace.production_short_axis_span_grid,
        production_maximum_side_grid: trace.production_maximum_side_grid,
        production_envelope_area_grid2: exact_decimal(trace.production_envelope_area_grid2.clone()),
        transform_evaluations: trace.transform_evaluations,
        expected_pair_count: trace.expected_pair_count,
        evaluated_pair_count: trace.evaluated_pair_count,
        construction_kind: trace
            .construction_kind
            .map(project_short_side_construction_kind),
        row_count: trace.row_count,
        selected_bottom_piece_id: trace
            .selected_bottom_piece_id
            .as_ref()
            .map(|id| id.as_str().to_string()),
        selected_upper_piece_id: trace
            .selected_upper_piece_id
            .as_ref()
            .map(|id| id.as_str().to_string()),
        placed_count: trace.placed_count,
        used_short_axis_span_mm: trace.used_short_axis_span_mm,
        used_long_axis_depth_mm: trace.used_long_axis_depth_mm,
        envelope_area_mm2: trace.envelope_area_mm2,
        used_short_axis_span_grid: trace.used_short_axis_span_grid,
        used_long_axis_depth_grid: trace.used_long_axis_depth_grid,
        envelope_area_grid2: trace
            .envelope_area_grid2
            .as_ref()
            .map(|value| exact_decimal(value.clone())),
        collision_material_doubled_area_grid2: trace
            .collision_material_doubled_area_grid2
            .as_ref()
            .map(|value| exact_decimal(value.clone())),
        canonical_geometry_hash: trace.canonical_geometry_hash.clone(),
        admission: trace.admission.as_ref().map(project_short_side_admission),
        interlocking: trace
            .interlocking
            .as_ref()
            .map(project_short_side_interlocking),
        envelope_area_cost_veto_observed: trace.envelope_area_cost_veto_observed,
        envelope_area_cost_vetoes: trace
            .envelope_area_cost_vetoes
            .iter()
            .map(
                |veto| polygon_nesting_protocol::result::ShortSideEnvelopeAreaCostVeto {
                    construction_kind: project_short_side_construction_kind(veto.construction_kind),
                    admission: project_short_side_admission(&veto.admission),
                },
            )
            .collect(),
        contact_strip: trace
            .contact_strip
            .as_ref()
            .map(project_short_side_contact_strip_trace),
        contact_strip_lanes: trace
            .contact_strip_lanes
            .iter()
            .map(project_short_side_contact_strip_trace)
            .collect(),
        contact_strip_promotion: trace
            .contact_strip_promotion
            .as_ref()
            .map(project_short_side_contact_strip_promotion),
        runtime_ms: trace.runtime_ms,
        peak_rss_delta_bytes: trace.peak_rss_delta_bytes,
        serialized_trace_bytes: trace.serialized_trace_bytes,
        failure_reason: trace.failure_reason.clone(),
    }
}

fn project_short_side_pair_fold_status(
    status: crate::short_side::pair_fold::IntrinsicShortSidePairFoldStatus,
) -> polygon_nesting_protocol::result::ShortSidePairFoldStatus {
    use crate::short_side::pair_fold::IntrinsicShortSidePairFoldStatus as Core;
    use polygon_nesting_protocol::result::ShortSidePairFoldStatus as Protocol;

    match status {
        Core::Accepted => Protocol::Accepted,
        Core::SkippedSquareSheet => Protocol::SkippedSquareSheet,
        Core::NoPair => Protocol::NoPair,
        Core::NoFittingPair => Protocol::NoFittingPair,
        Core::RejectedAdmission => Protocol::RejectedAdmission,
        Core::Deadline => Protocol::Deadline,
        Core::MemoryCap => Protocol::MemoryCap,
        Core::TraceCap => Protocol::TraceCap,
        Core::FailedProtectedFallback => Protocol::FailedProtectedFallback,
    }
}

fn project_short_side_construction_kind(
    kind: crate::short_side::pair_fold::IntrinsicShortSideConstructionKind,
) -> polygon_nesting_protocol::result::ShortSideConstructionKind {
    use crate::short_side::pair_fold::IntrinsicShortSideConstructionKind as Core;
    use polygon_nesting_protocol::result::ShortSideConstructionKind as Protocol;

    match kind {
        Core::PairFold => Protocol::PairFold,
        Core::MultiRowShelf => Protocol::MultiRowShelf,
        Core::ContactStrip => Protocol::ContactStrip,
    }
}

fn project_short_side_admission(
    admission: &crate::short_side::pair_fold::IntrinsicShortSidePairFoldAdmission,
) -> polygon_nesting_protocol::result::ShortSidePairFoldAdmission {
    polygon_nesting_protocol::result::ShortSidePairFoldAdmission {
        exact_legal: admission.exact_legal,
        all_pieces_placed: admission.all_pieces_placed,
        fill_ratio: admission.fill_ratio,
        depth_within_production_maximum_side: admission.depth_within_production_maximum_side,
        projection_coverage_ratio: admission.projection_coverage_ratio,
        projection_component_count: admission.projection_component_count,
        enclosed_cavity_count: admission.enclosed_cavity_count,
        collision_envelope_density: admission.collision_envelope_density,
        short_axis_span_gain_factor: admission.short_axis_span_gain_factor,
        envelope_area_cost_factor: admission.envelope_area_cost_factor,
        directionally_efficient: admission.directionally_efficient,
        envelope_area_cost_within_production_bound: admission
            .envelope_area_cost_within_production_bound,
        accepted: admission.accepted,
    }
}

fn project_short_side_interlocking(
    metrics: &crate::short_side::pair_fold::IntrinsicShortSideInterlockingMetrics,
) -> polygon_nesting_protocol::result::ShortSideInterlockingMetrics {
    polygon_nesting_protocol::result::ShortSideInterlockingMetrics {
        largest_occupied_hull_gap_ratio: metrics.largest_occupied_hull_gap_ratio,
        largest_occupied_hull_gap_doubled_area_grid2: exact_decimal(
            metrics.largest_occupied_hull_gap_doubled_area_grid2.clone(),
        ),
        occupied_hull_doubled_area_grid2: exact_decimal(
            metrics.occupied_hull_doubled_area_grid2.clone(),
        ),
        isolated_piece_count: metrics.isolated_piece_count,
        positive_contact_component_count: metrics.positive_contact_component_count,
        largest_positive_contact_component_size: metrics.largest_positive_contact_component_size,
        shared_boundary_length_mm: metrics.shared_boundary_length_mm,
    }
}

fn project_short_side_contact_strip_trace(
    trace: &crate::short_side::contact_strip::IntrinsicShortSideContactStripTrace,
) -> polygon_nesting_protocol::result::ShortSideContactStripTrace {
    use crate::short_side::contact_strip::{
        IntrinsicShortSideContactStripOrderPolicy as CoreOrder,
        IntrinsicShortSideContactStripSelectionPolicy as CoreSelection,
        IntrinsicShortSideContactStripStatus as CoreStatus,
    };
    use polygon_nesting_protocol::result::{
        ShortSideContactStripStatus as ProtocolStatus, ShortSideContactStripVersion,
        ShortSideExecutionModel, ShortSideOrderPolicy, ShortSideSelectionPolicy,
    };

    match trace.version {
        "intrinsic-short-side-contact-strip-v3" => {}
        _ => unreachable!("invalid intrinsic short-side contact-strip version"),
    }
    match trace.execution_model {
        "single-process-sequential" => {}
        _ => unreachable!("invalid intrinsic short-side contact-strip execution model"),
    }
    polygon_nesting_protocol::result::ShortSideContactStripTrace {
        version: ShortSideContactStripVersion::V3,
        status: match trace.status {
            CoreStatus::Constructed => ProtocolStatus::Constructed,
            CoreStatus::NoLegalPlacement => ProtocolStatus::NoLegalPlacement,
            CoreStatus::Deadline => ProtocolStatus::Deadline,
            CoreStatus::MemoryCap => ProtocolStatus::MemoryCap,
            CoreStatus::EvaluationCap => ProtocolStatus::EvaluationCap,
            CoreStatus::FailedProtectedFallback => ProtocolStatus::FailedProtectedFallback,
        },
        execution_model: ShortSideExecutionModel::SingleProcessSequential,
        selection_policy: match trace.selection_policy {
            CoreSelection::DepthFirst => ShortSideSelectionPolicy::DepthFirst,
            CoreSelection::ContactFirst => ShortSideSelectionPolicy::ContactFirst,
        },
        order_policy: match trace.order_policy {
            CoreOrder::Prepared => ShortSideOrderPolicy::Prepared,
            CoreOrder::Reverse => ShortSideOrderPolicy::Reverse,
            CoreOrder::PieceIdAscending => ShortSideOrderPolicy::PieceIdAscending,
        },
        strip_short_axis_mm: trace.strip_short_axis_mm,
        strip_long_axis_mm: trace.strip_long_axis_mm,
        transform_evaluations: trace.transform_evaluations,
        candidate_evaluations: trace.candidate_evaluations,
        backtrack_count: trace.backtrack_count,
        reused_prefix_placements: trace.reused_prefix_placements,
        placed_count: trace.placed_count,
        requested_count: trace.requested_count,
        runtime_ms: trace.runtime_ms,
        peak_rss_delta_bytes: trace.peak_rss_delta_bytes,
        failure_reason: trace.failure_reason.clone(),
    }
}

fn project_short_side_construction_summary(
    summary: &crate::short_side::pair_fold::IntrinsicShortSideConstructionSummary,
) -> polygon_nesting_protocol::result::ShortSideConstructionSummary {
    polygon_nesting_protocol::result::ShortSideConstructionSummary {
        used_short_axis_span_mm: summary.used_short_axis_span_mm,
        used_long_axis_depth_mm: summary.used_long_axis_depth_mm,
        envelope_area_mm2: summary.envelope_area_mm2,
        used_short_axis_span_grid: summary.used_short_axis_span_grid,
        used_long_axis_depth_grid: summary.used_long_axis_depth_grid,
        envelope_area_grid2: summary
            .envelope_area_grid2
            .as_ref()
            .map(|value| exact_decimal(value.clone())),
        collision_material_doubled_area_grid2: summary
            .collision_material_doubled_area_grid2
            .as_ref()
            .map(|value| exact_decimal(value.clone())),
        admission: summary.admission.as_ref().map(project_short_side_admission),
        interlocking: summary
            .interlocking
            .as_ref()
            .map(project_short_side_interlocking),
        status: project_short_side_pair_fold_status(summary.status),
        failure_reason: summary.failure_reason.clone(),
    }
}

fn project_short_side_contact_strip_promotion(
    promotion: &crate::short_side::pair_fold::IntrinsicShortSideContactStripPromotion,
) -> polygon_nesting_protocol::result::ShortSideContactStripPromotion {
    polygon_nesting_protocol::result::ShortSideContactStripPromotion {
        incumbent_construction_kind: promotion
            .incumbent_construction_kind
            .map(project_short_side_construction_kind),
        contact_strip_summary: promotion
            .contact_strip_summary
            .as_ref()
            .map(project_short_side_construction_summary),
        contact_strip_admitted: promotion.contact_strip_admitted,
        fill_not_regressed: promotion.fill_not_regressed,
        envelope_area_not_regressed: promotion.envelope_area_not_regressed,
        depth_not_regressed: promotion.depth_not_regressed,
        density_not_regressed: promotion.density_not_regressed,
        hull_gap_not_regressed: promotion.hull_gap_not_regressed,
        isolated_pieces_not_regressed: promotion.isolated_pieces_not_regressed,
        positive_contact_components_not_regressed: promotion
            .positive_contact_components_not_regressed,
        largest_contact_component_not_regressed: promotion.largest_contact_component_not_regressed,
        strictly_improved: promotion.strictly_improved,
        promoted: promotion.promoted,
    }
}

fn project_state_snapshot(
    snapshot: &crate::result::IrregularStateSnapshot,
) -> polygon_nesting_protocol::StateSnapshot {
    polygon_nesting_protocol::StateSnapshot {
        step_index: snapshot.step_index,
        beam_rank: snapshot.beam_rank,
        candidate_count: snapshot.candidate_count,
        source: snapshot.source.map(|source| match source {
            crate::result::IrregularStateSnapshotSource::Beam => {
                polygon_nesting_protocol::StateSnapshotSource::Beam
            }
            crate::result::IrregularStateSnapshotSource::SharedArchive => {
                polygon_nesting_protocol::StateSnapshotSource::SharedArchive
            }
        }),
        placements: snapshot
            .state
            .placed_collision_geometries
            .iter()
            .map(|placed| project_placed_collision_geometry(placed))
            .collect(),
        remaining_prepared_pieces: snapshot
            .state
            .remaining_prepared_pieces
            .iter()
            .map(|piece| project_prepared_piece(piece))
            .collect(),
        unplaced_piece_ids: snapshot
            .state
            .unplaced_piece_ids
            .iter()
            .map(|id| id.as_str().to_string())
            .collect(),
    }
}

fn project_prepared_piece(
    piece: &crate::domain::IrregularPreparedPiece,
) -> polygon_nesting_protocol::SnapshotPreparedPiece {
    polygon_nesting_protocol::SnapshotPreparedPiece {
        piece_id: piece.piece_id.as_ref().map(|id| id.as_str().to_string()),
        interchangeability_key: piece.interchangeability_key.clone(),
        source: project_source_piece(&piece.source),
        allow_mirror: piece.allow_mirror,
        collision_geometry: polygon_nesting_protocol::PreparedCollisionGeometry {
            source_piece_id: piece
                .collision_geometry
                .source_piece_id
                .as_str()
                .to_string(),
            source_bounds: project_bounds(&piece.collision_geometry.source_bounds),
            sampled_points: piece
                .collision_geometry
                .sampled_points
                .iter()
                .map(|point| polygon_nesting_protocol::Point {
                    x: point.x,
                    y: point.y,
                })
                .collect(),
            convex_hull: project_polygon(&piece.collision_geometry.convex_hull),
            collision_polygon: project_polygon(&piece.collision_geometry.collision_polygon),
            placement_reference: polygon_nesting_protocol::Point {
                x: piece.collision_geometry.placement_reference.x,
                y: piece.collision_geometry.placement_reference.y,
            },
            diagnostics: piece
                .collision_geometry
                .diagnostics
                .iter()
                .map(project_diagnostic)
                .collect(),
        },
        transforms: piece
            .transforms
            .iter()
            .map(project_collision_transform)
            .collect(),
        priority_order_key: piece.priority_order_key.map(|key| {
            polygon_nesting_protocol::PriorityOrderKey {
                long_side_mm: key.long_side_mm,
                area_mm2: key.area_mm2,
                imbalance_mm: key.imbalance_mm,
            }
        }),
    }
}

fn project_source_piece(piece: &ImportedPiece) -> polygon_nesting_protocol::SourcePiece {
    polygon_nesting_protocol::SourcePiece {
        id: piece.id.as_str().to_string(),
        source_file_id: piece.source_file_id.as_str().to_string(),
        source_layer: piece.source_layer.clone(),
        label: piece.label.clone(),
        real_bounds: polygon_nesting_protocol::Rect {
            x: piece.real_bounds.x,
            y: piece.real_bounds.y,
            width: piece.real_bounds.width,
            height: piece.real_bounds.height,
        },
        geometry: polygon_nesting_protocol::SourceGeometry {
            entity_type: match piece.geometry.entity_type {
                DxfGeometryEntityType::Line => SourceGeometryEntityType::Line,
                DxfGeometryEntityType::Lwpolyline => SourceGeometryEntityType::Lwpolyline,
                DxfGeometryEntityType::Polyline => SourceGeometryEntityType::Polyline,
                DxfGeometryEntityType::Circle => SourceGeometryEntityType::Circle,
                DxfGeometryEntityType::Arc => SourceGeometryEntityType::Arc,
                DxfGeometryEntityType::Ellipse => SourceGeometryEntityType::Ellipse,
                DxfGeometryEntityType::DxfShape => SourceGeometryEntityType::DxfShape,
                DxfGeometryEntityType::PresetShape => SourceGeometryEntityType::PresetShape,
            },
            closed: piece.geometry.closed,
            segments: piece
                .geometry
                .segments
                .iter()
                .map(project_source_segment)
                .collect(),
        },
        warnings: piece.warnings.iter().map(project_source_warning).collect(),
    }
}

fn project_source_segment(segment: &DxfGeometrySegment) -> SourceGeometrySegment {
    match segment {
        DxfGeometrySegment::Line(line) => {
            SourceGeometrySegment::Line(polygon_nesting_protocol::SourceLineSegment {
                x1: line.x1,
                y1: line.y1,
                x2: line.x2,
                y2: line.y2,
                bulge: line.bulge,
                source_curve: line.source_curve.as_ref().map(|curve| EllipseSource {
                    kind: polygon_nesting_protocol::EllipseSourceKind::Ellipse,
                    source_id: curve.source_id.clone(),
                    cx: curve.cx,
                    cy: curve.cy,
                    major_axis_x: curve.major_axis_x,
                    major_axis_y: curve.major_axis_y,
                    axis_ratio: curve.axis_ratio,
                    start_angle: curve.start_angle,
                    end_angle: curve.end_angle,
                }),
            })
        }
        DxfGeometrySegment::Arc(arc) => {
            SourceGeometrySegment::Arc(polygon_nesting_protocol::SourceArcSegment {
                x1: arc.x1,
                y1: arc.y1,
                x2: arc.x2,
                y2: arc.y2,
                cx: arc.cx,
                cy: arc.cy,
                radius: arc.radius,
                start_angle: arc.start_angle,
                end_angle: arc.end_angle,
            })
        }
    }
}

fn project_source_warning(warning: &ImportWarning) -> SourceWarning {
    SourceWarning {
        code: warning.code.clone(),
        message: warning.message.clone(),
        entity_type: warning.entity_type.clone(),
        entity_handle: warning.entity_handle.as_ref().map(|handle| match handle {
            DxfEntityHandle::Text(value) => SourceEntityHandle::Text(value.clone()),
            DxfEntityHandle::Number(value) => SourceEntityHandle::Number(*value),
        }),
    }
}

fn project_placed_collision_geometry(
    placed: &crate::domain::IrregularPlacedPiece,
) -> polygon_nesting_protocol::PlacedCollisionGeometry {
    polygon_nesting_protocol::PlacedCollisionGeometry {
        placement: project_placement(&placed.placement),
        collision_geometry: polygon_nesting_protocol::CollisionGeometry {
            source_piece_id: placed
                .collision_geometry
                .source_piece_id
                .as_str()
                .to_string(),
            transform: project_collision_transform(&placed.collision_geometry.transform),
            polygon: project_polygon(&placed.collision_geometry.polygon),
            bounds: project_bounds(&placed.collision_geometry.bounds),
        },
    }
}

fn project_placement(
    placement: &crate::domain::IrregularPlacement,
) -> polygon_nesting_protocol::Placement {
    polygon_nesting_protocol::Placement {
        piece_id: placement
            .piece_id
            .as_ref()
            .map(|id| id.as_str().to_string()),
        source_piece_id: placement.source_piece_id.as_str().to_string(),
        placement_reference: placement.placement_reference.map(|point| {
            polygon_nesting_protocol::PlacementReference {
                x: point.x,
                y: point.y,
            }
        }),
        transform: polygon_nesting_protocol::PlacementTransform {
            translate_x: placement.transform.translate_x,
            translate_y: placement.transform.translate_y,
            rotation_deg: placement.transform.rotation_deg,
            mirrored: placement.transform.mirrored,
        },
    }
}

fn project_collision_transform(
    transform: &crate::domain::IrregularTransformCandidate,
) -> polygon_nesting_protocol::CollisionTransform {
    polygon_nesting_protocol::CollisionTransform {
        index: transform.index,
        rotation_deg: transform.rotation_deg,
        mirrored: transform.mirrored,
        reason: match transform.reason {
            crate::domain::IrregularTransformReason::Orthogonal => {
                polygon_nesting_protocol::IrregularTransformReason::Orthogonal
            }
            crate::domain::IrregularTransformReason::EdgeAlignment => {
                polygon_nesting_protocol::IrregularTransformReason::EdgeAlignment
            }
            crate::domain::IrregularTransformReason::Configured => {
                polygon_nesting_protocol::IrregularTransformReason::Configured
            }
        },
    }
}

fn project_polygon(polygon: &crate::domain::IrregularPolygon) -> polygon_nesting_protocol::Polygon {
    polygon_nesting_protocol::Polygon {
        points: polygon
            .points
            .iter()
            .map(|point| polygon_nesting_protocol::Point {
                x: point.x,
                y: point.y,
            })
            .collect(),
    }
}

fn project_bounds(bounds: &crate::domain::IrregularBounds) -> polygon_nesting_protocol::Bounds {
    polygon_nesting_protocol::Bounds {
        min_x: bounds.min_x,
        min_y: bounds.min_y,
        max_x: bounds.max_x,
        max_y: bounds.max_y,
    }
}

fn project_diagnostic(
    diagnostic: &crate::domain::CollisionGeometryDiagnostic,
) -> polygon_nesting_protocol::ResultDiagnostic {
    polygon_nesting_protocol::ResultDiagnostic {
        code: diagnostic.code.clone(),
        message: diagnostic.message.clone(),
        piece_id: diagnostic
            .piece_id
            .as_ref()
            .map(|id| id.as_str().to_string()),
    }
}

fn project_score_summary(
    score: &crate::domain::IrregularLayoutScoreSummary,
) -> polygon_nesting_protocol::LayoutScoreSummary {
    polygon_nesting_protocol::LayoutScoreSummary {
        unplaced_count: score.unplaced_count,
        shared_collision_boundary_length_mm: score.shared_collision_boundary_length_mm,
        shared_collision_boundary_contact_units: score.shared_collision_boundary_contact_units,
        shared_collision_boundary_contact_band: score.shared_collision_boundary_contact_band,
        near_complete_structural_contact_count: score.near_complete_structural_contact_count,
        dominant_near_complete_structural_contact_count: score
            .dominant_near_complete_structural_contact_count,
        largest_net_free_material_region_area_mm2: score.largest_net_free_material_region_area_mm2,
        free_material_region_count: score.free_material_region_count,
        free_material_hole_count: score.free_material_hole_count,
        canonical_enclosed_cavity_count: score.canonical_enclosed_cavity_count,
        free_material_sliver_metric: score.free_material_sliver_metric,
        collision_bounds_worst_normalized_sheet_consumption: score
            .collision_bounds_worst_normalized_sheet_consumption,
        collision_bounds_normalized_span_sum: score.collision_bounds_normalized_span_sum,
        collision_bounds_area_mm2: score.collision_bounds_area_mm2,
        collision_bounds_span_mm: score.collision_bounds_span_mm,
    }
}

fn project_error(error: IrregularComputeErrorType) -> EngineError {
    const OPERATION: &str = "computeIrregularNesting";

    match error {
        IrregularComputeErrorType::NfpIfpControlAbort(error) => match error.reason {
            NfpIfpAbortReason::Cancelled => {
                EngineError::new(EngineErrorCode::Cancelled, OPERATION, error.message)
                    .with_context("reason", "cancelled")
            }
            NfpIfpAbortReason::Deadline => {
                EngineError::new(EngineErrorCode::DeadlineExceeded, OPERATION, error.message)
                    .with_context("reason", "deadline")
            }
        },
        IrregularComputeErrorType::GeometryInput(error) => EngineError::new(
            EngineErrorCode::InvalidGeometry,
            error.operation.clone(),
            error.message,
        )
        .with_context("operation", error.operation),
        IrregularComputeErrorType::Compute(error) => {
            EngineError::new(EngineErrorCode::InvalidGeometry, OPERATION, error.message)
                .with_context("preparedPieceId", error.prepared_piece_id.as_str())
                .with_context("sourcePieceId", error.source_piece_id.as_str())
        }
        IrregularComputeErrorType::NoValidResult(error) => EngineError::new(
            EngineErrorCode::EngineFailure,
            error.operation.clone(),
            error.message,
        )
        .with_context("operation", error.operation),
        IrregularComputeErrorType::NotImplemented(error) => EngineError::new(
            EngineErrorCode::InternalFailure,
            error.operation.clone(),
            error.message,
        )
        .with_context("service", error.service)
        .with_context("operation", error.operation),
        IrregularComputeErrorType::Portfolio(error) => {
            let (category, context_category) = match error.category {
                crate::result::IrregularPortfolioErrorCategory::Geometry => {
                    (EngineErrorCode::InvalidGeometry, "geometry")
                }
                crate::result::IrregularPortfolioErrorCategory::Scoring => {
                    (EngineErrorCode::EngineFailure, "scoring")
                }
                crate::result::IrregularPortfolioErrorCategory::Search => {
                    (EngineErrorCode::EngineFailure, "search")
                }
            };
            EngineError::new(category, error.operation.clone(), error.message)
                .with_context("operation", error.operation)
                .with_context("category", context_category)
        }
        IrregularComputeErrorType::PlacementScoring(error) => EngineError::new(
            EngineErrorCode::EngineFailure,
            error.operation.clone(),
            error.message,
        )
        .with_context("operation", error.operation),
        IrregularComputeErrorType::LayoutScoring(error) => EngineError::new(
            EngineErrorCode::EngineFailure,
            error.operation.clone(),
            error.message,
        )
        .with_context("operation", error.operation),
    }
}

fn control_failure_outcome(control: &CancellationControl) -> Option<EngineOutcome> {
    const OPERATION: &str = "computeIrregularNesting";

    let error = match control.reason()? {
        crate::CancelReason::Cancelled => {
            EngineError::new(EngineErrorCode::Cancelled, OPERATION, "cancelled by caller")
                .with_context("reason", "cancelled")
        }
        crate::CancelReason::Deadline => EngineError::new(
            EngineErrorCode::DeadlineExceeded,
            OPERATION,
            "deadline exceeded",
        )
        .with_context("reason", "deadline"),
    };
    Some(EngineOutcome::Failure {
        error,
        diagnostics: ExecutionDiagnostics::default(),
    })
}

pub struct Job<'a> {
    request: &'a EngineRequest,
    control: &'a CancellationControl,
    sink: &'a mut dyn EngineEventSink,
    thread_count_override: Option<usize>,
}

impl<'a> Job<'a> {
    pub fn new(
        request: &'a EngineRequest,
        control: &'a CancellationControl,
        sink: &'a mut dyn EngineEventSink,
    ) -> Self {
        Self {
            request,
            control,
            sink,
            thread_count_override: None,
        }
    }

    pub fn with_thread_count(
        request: &'a EngineRequest,
        control: &'a CancellationControl,
        sink: &'a mut dyn EngineEventSink,
        thread_count_override: Option<usize>,
    ) -> Self {
        Self {
            request,
            control,
            sink,
            thread_count_override,
        }
    }

    pub fn run(self) -> Result<EngineOutcome, EngineError> {
        self.request.validate().map_err(validation_error)?;
        if let Some(outcome) = control_failure_outcome(self.control) {
            return Ok(outcome);
        }
        if let Some(outcome) = self.request.archive_ineligible_outcome() {
            return Ok(outcome);
        }

        let started = Instant::now();
        let request = prepare_nesting_request(self.request);
        let settings = request
            .options
            .irregular_settings
            .clone()
            .ok_or_else(|| internal_failure("prepare-nesting-request"))?;
        let mut geometry_cache = GeometryCacheStore::new();
        let mut free_material_cache = FreeMaterialCache::new();
        let pool = JobPool::new(self.thread_count_override);
        let thread_counts = pool.thread_counts();
        let mut event_sink = ProtocolEventSink::new(self.sink);
        let mut cancellation_reason = || {
            if self.control.reason().is_none()
                && started.elapsed().as_secs_f64() * 1_000.0 >= self.request.timeout_ms
            {
                self.control.cancel(crate::CancelReason::Deadline);
            }
            match self.control.reason() {
                Some(crate::CancelReason::Cancelled) => Some(NfpIfpAbortReason::Cancelled),
                Some(crate::CancelReason::Deadline) => Some(NfpIfpAbortReason::Deadline),
                None => None,
            }
        };
        let mut options = ComputeIrregularNestingOptions {
            event_sink: Some(&mut event_sink),
            cancellation_reason: Some(&mut cancellation_reason),
            focused_complete_reconstruction_enabled: true,
        };
        let execution = catch_unwind(AssertUnwindSafe(|| {
            pool.run_scoped(|| {
                compute_irregular_nesting(
                    &request,
                    &settings,
                    &mut options,
                    &mut geometry_cache,
                    &mut free_material_cache,
                )
            })
        }));
        free_material_cache.clear_and_shrink();
        geometry_cache.clear_and_shrink();
        let diagnostics = execution_diagnostics(
            thread_counts,
            started.elapsed().as_secs_f64() * 1_000.0,
            geometry_cache.telemetry(),
            free_material_cache.telemetry(),
        );

        match execution {
            Ok(Ok(result)) => Ok(EngineOutcome::Success {
                result: project_result(result, self.request.diagnostic_trace_mode),
                diagnostics,
            }),
            Ok(Err(error)) => Ok(EngineOutcome::Failure {
                error: project_error(error),
                diagnostics,
            }),
            Err(_) => Ok(EngineOutcome::Failure {
                error: internal_failure("compute-irregular-nesting"),
                diagnostics,
            }),
        }
    }
}

pub fn run(
    request: &EngineRequest,
    control: &CancellationControl,
    sink: &mut dyn EngineEventSink,
) -> Result<EngineOutcome, EngineError> {
    Job::new(request, control, sink).run()
}

#[cfg(test)]
mod tests {
    use polygon_nesting_protocol as protocol;

    use super::*;

    const COMPUTE_OPERATION: &str = "computeIrregularNesting";

    #[test]
    fn project_error_maps_compute_contract_exactly() {
        let actual = project_error(IrregularComputeErrorType::Compute(
            crate::result::IrregularComputeError {
                prepared_piece_id: PieceId::new("prepared-1"),
                source_piece_id: PieceId::new("source-1"),
                message: "missing source geometry".to_owned(),
            },
        ));

        let expected = EngineError::new(
            EngineErrorCode::InvalidGeometry,
            COMPUTE_OPERATION,
            "missing source geometry",
        )
        .with_context("preparedPieceId", "prepared-1")
        .with_context("sourcePieceId", "source-1");
        assert_eq!(actual, expected);
    }

    #[test]
    fn project_error_maps_geometry_input_contract_exactly() {
        let actual = project_error(IrregularComputeErrorType::GeometryInput(
            crate::validation::placement::IrregularGeometryInputError {
                operation: "buildCollisionGeometry".to_owned(),
                message: "invalid polygon".to_owned(),
            },
        ));

        let expected = EngineError::new(
            EngineErrorCode::InvalidGeometry,
            "buildCollisionGeometry",
            "invalid polygon",
        )
        .with_context("operation", "buildCollisionGeometry");
        assert_eq!(actual, expected);
    }

    #[test]
    fn project_error_maps_cancellation_contract_exactly() {
        for (reason, category, context_reason, message) in [
            (
                NfpIfpAbortReason::Cancelled,
                EngineErrorCode::Cancelled,
                "cancelled",
                "cancelled by caller",
            ),
            (
                NfpIfpAbortReason::Deadline,
                EngineErrorCode::DeadlineExceeded,
                "deadline",
                "deadline exceeded",
            ),
        ] {
            let actual = project_error(IrregularComputeErrorType::NfpIfpControlAbort(
                crate::nfp_ifp::NfpIfpControlAbortError {
                    reason,
                    message: message.to_owned(),
                },
            ));

            let expected = EngineError::new(category, COMPUTE_OPERATION, message)
                .with_context("reason", context_reason);
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn project_error_maps_no_valid_result_contract_exactly() {
        let actual = project_error(IrregularComputeErrorType::NoValidResult(
            crate::result::IrregularNoValidResultError {
                operation: "selectPortfolioResult".to_owned(),
                message: "no valid result".to_owned(),
            },
        ));

        let expected = EngineError::new(
            EngineErrorCode::EngineFailure,
            "selectPortfolioResult",
            "no valid result",
        )
        .with_context("operation", "selectPortfolioResult");
        assert_eq!(actual, expected);
    }

    #[test]
    fn project_error_maps_not_implemented_contract_exactly() {
        let actual = project_error(IrregularComputeErrorType::NotImplemented(
            crate::result::IrregularNestingNotImplementedError {
                service: "freeMaterial".to_owned(),
                operation: "extendFreeMaterial".to_owned(),
                message: "not implemented".to_owned(),
            },
        ));

        let expected = EngineError::new(
            EngineErrorCode::InternalFailure,
            "extendFreeMaterial",
            "not implemented",
        )
        .with_context("service", "freeMaterial")
        .with_context("operation", "extendFreeMaterial");
        assert_eq!(actual, expected);
    }

    #[test]
    fn project_error_maps_portfolio_contract_exactly() {
        for (category, expected_code, context_category) in [
            (
                crate::result::IrregularPortfolioErrorCategory::Geometry,
                EngineErrorCode::InvalidGeometry,
                "geometry",
            ),
            (
                crate::result::IrregularPortfolioErrorCategory::Scoring,
                EngineErrorCode::EngineFailure,
                "scoring",
            ),
            (
                crate::result::IrregularPortfolioErrorCategory::Search,
                EngineErrorCode::EngineFailure,
                "search",
            ),
        ] {
            let actual = project_error(IrregularComputeErrorType::Portfolio(
                crate::result::IrregularPortfolioError {
                    operation: "runPortfolio".to_owned(),
                    category,
                    message: "portfolio failed".to_owned(),
                },
            ));

            let expected = EngineError::new(expected_code, "runPortfolio", "portfolio failed")
                .with_context("operation", "runPortfolio")
                .with_context("category", context_category);
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn project_error_maps_placement_scoring_contract_exactly() {
        let actual = project_error(IrregularComputeErrorType::PlacementScoring(
            crate::search::placement_scorer::IrregularPlacementScoringError {
                operation: "scorePlacement".to_owned(),
                message: "placement scoring failed".to_owned(),
            },
        ));

        let expected = EngineError::new(
            EngineErrorCode::EngineFailure,
            "scorePlacement",
            "placement scoring failed",
        )
        .with_context("operation", "scorePlacement");
        assert_eq!(actual, expected);
    }

    #[test]
    fn project_error_maps_layout_scoring_contract_exactly() {
        let actual = project_error(IrregularComputeErrorType::LayoutScoring(
            crate::search::layout_scorer::IrregularLayoutScoringError {
                operation: "scoreLayout".to_owned(),
                message: "layout scoring failed".to_owned(),
            },
        ));

        let expected = EngineError::new(
            EngineErrorCode::EngineFailure,
            "scoreLayout",
            "layout scoring failed",
        )
        .with_context("operation", "scoreLayout");
        assert_eq!(actual, expected);
    }

    #[test]
    fn execution_diagnostics_flattens_every_namespace_counter_with_stable_keys() {
        let namespace = crate::caches::CacheNamespaceTelemetry {
            lookups: 1,
            hits: 2,
            misses: 3,
            stores: 4,
            stale_detections: 5,
            stale_removals: 6,
            duplicate_computations: 7,
            single_flight_waits: 8,
            shard_lock_wait_nanos: 9,
            shard_lock_contended_acquisitions: 10,
            front_cache_hits: 11,
            backing_cache_hits: 12,
            cloning_hits: 13,
            cap_bytes: 14,
            admissions: 15,
            replacements: 16,
            evictions: 17,
            evicted_bytes: 18,
            oversized_rejections: 19,
            entries: 20,
            approx_bytes: 21,
            peak_bytes: 22,
            computation_time_nanos: 23,
        };
        let geometry = crate::caches::CacheTelemetrySnapshot {
            namespaces: BTreeMap::from([("test-namespace".to_owned(), namespace)]),
            ..Default::default()
        };
        let diagnostics = execution_diagnostics(
            crate::parallel::JobThreadCounts {
                requested: 1,
                actual: 1,
            },
            0.0,
            &geometry,
            &Default::default(),
        );
        let prefix = "geometry_cache.namespace.test-namespace.";
        let actual = diagnostics
            .counters
            .iter()
            .filter_map(|(key, value)| {
                key.strip_prefix(prefix)
                    .map(|suffix| (suffix.to_owned(), *value))
            })
            .collect::<BTreeMap<_, _>>();
        let expected = BTreeMap::from([
            ("lookups".to_owned(), 1),
            ("hits".to_owned(), 2),
            ("misses".to_owned(), 3),
            ("stores".to_owned(), 4),
            ("stale_detections".to_owned(), 5),
            ("stale_removals".to_owned(), 6),
            ("duplicate_computations".to_owned(), 7),
            ("single_flight_waits".to_owned(), 8),
            ("shard_lock_wait_nanos".to_owned(), 9),
            ("shard_lock_contended_acquisitions".to_owned(), 10),
            ("front_cache_hits".to_owned(), 11),
            ("backing_cache_hits".to_owned(), 12),
            ("cloning_hits".to_owned(), 13),
            ("cap_bytes".to_owned(), 14),
            ("admissions".to_owned(), 15),
            ("replacements".to_owned(), 16),
            ("evictions".to_owned(), 17),
            ("evicted_bytes".to_owned(), 18),
            ("oversized_rejections".to_owned(), 19),
            ("entries".to_owned(), 20),
            ("approx_bytes".to_owned(), 21),
            ("peak_bytes".to_owned(), 22),
            ("computation_time_nanos".to_owned(), 23),
        ]);
        assert_eq!(actual, expected);
    }

    #[test]
    fn preparation_preserves_request_data_and_maps_intrinsic_objective_profile() {
        let compact_request = valid_request(protocol::EngineProfile::Compact);
        let short_side_request = valid_request(protocol::EngineProfile::CompactShortSide);

        compact_request.validate().unwrap();
        short_side_request.validate().unwrap();

        let compact = prepare_nesting_request(&compact_request);
        let short_side = prepare_nesting_request(&short_side_request);
        let compact_settings = compact.options.irregular_settings.as_ref().unwrap();
        let short_side_settings = short_side.options.irregular_settings.as_ref().unwrap();

        assert_eq!(
            compact_settings.optimizer.intrinsic_objective_profile_id,
            crate::domain::IntrinsicObjectiveProfileId::Compact
        );
        assert_eq!(
            short_side_settings.optimizer.intrinsic_objective_profile_id,
            crate::domain::IntrinsicObjectiveProfileId::ShortSide
        );
        let mut expected_short_side_settings = compact_settings.clone();
        expected_short_side_settings
            .optimizer
            .intrinsic_objective_profile_id = crate::domain::IntrinsicObjectiveProfileId::ShortSide;
        assert_eq!(*short_side_settings, expected_short_side_settings);
        assert_eq!(compact.sheet.width, compact_request.sheet.width);
        assert_eq!(compact.sheet.height, compact_request.sheet.height);
        assert_eq!(compact.sheet.label, compact_request.sheet.label);
        assert_eq!(compact.padding, compact_request.settings.padding);
        assert_eq!(
            compact_settings.geometry.flattening_sag_tolerance_mm,
            compact_request
                .settings
                .geometry
                .flattening_sag_tolerance_mm
        );
        assert_eq!(
            compact_settings.geometry.clearance_safety_margin_mm,
            compact_request.settings.geometry.clearance_safety_margin_mm
        );
        assert_eq!(
            compact_settings.geometry.geometry_backend_id,
            compact_request.settings.geometry.geometry_backend_id
        );
        assert_eq!(
            compact_settings.geometry.geometry_backend_version,
            compact_request.settings.geometry.geometry_backend_version
        );
        assert_eq!(
            compact_settings.optimizer.intrinsic_shared_archive_enabled,
            compact_request
                .settings
                .optimizer
                .intrinsic_shared_archive_enabled
        );
        assert_eq!(
            compact_settings.optimizer.order_window,
            compact_request.settings.optimizer.order_window
        );
        assert_eq!(
            compact_settings.optimizer.beam_width,
            compact_request.settings.optimizer.beam_width
        );
        assert_eq!(
            compact_settings.optimizer.local_candidate_fanout,
            compact_request.settings.optimizer.local_candidate_fanout
        );
        assert_eq!(
            compact_settings.optimizer.local_repair_budget,
            compact_request.settings.optimizer.local_repair_budget
        );
        assert_eq!(
            compact_settings.optimizer.transform_cap,
            compact_request.settings.optimizer.transform_cap
        );
        assert_eq!(
            compact_settings.optimizer.transform_minimum_edge_length_mm,
            compact_request
                .settings
                .optimizer
                .transform_minimum_edge_length_mm
        );
        assert_eq!(
            compact_settings
                .optimizer
                .transform_angle_deduplication_tolerance_deg,
            compact_request
                .settings
                .optimizer
                .transform_angle_deduplication_tolerance_deg
        );
        assert_eq!(
            compact_settings.optimizer.configured_rotation_enabled,
            compact_request
                .settings
                .optimizer
                .configured_rotation_enabled
        );
        assert_eq!(
            compact_settings.optimizer.edge_alignment_enabled,
            compact_request.settings.optimizer.edge_alignment_enabled
        );
        assert_eq!(
            compact_settings.optimizer.configured_rotation_deg,
            compact_request.settings.optimizer.configured_rotation_deg
        );
        assert_eq!(
            compact_settings.optimizer.ga_enabled,
            compact_request.settings.optimizer.ga_enabled
        );
        assert_eq!(
            compact_settings.optimizer.baseline_only,
            compact_request.settings.optimizer.baseline_only
        );
        assert_eq!(
            compact_settings.optimizer.ga_population,
            compact_request.settings.optimizer.ga_population
        );
        assert_eq!(
            compact_settings.optimizer.ga_generation_budget,
            compact_request.settings.optimizer.ga_generation_budget
        );
        assert_eq!(
            compact_settings.optimizer.ga_evaluation_budget,
            compact_request.settings.optimizer.ga_evaluation_budget
        );
        assert_eq!(
            compact_settings.optimizer.ga_time_budget_ms,
            compact_request.settings.optimizer.ga_time_budget_ms
        );
        assert_eq!(
            compact_settings.optimizer.ga_seed,
            compact_request.settings.optimizer.ga_seed
        );
        assert_eq!(
            compact_settings.optimizer.priority_order_mutation_enabled,
            compact_request
                .settings
                .optimizer
                .priority_order_mutation_enabled
        );
        assert_eq!(
            compact_settings
                .optimizer
                .transform_preference_mutation_enabled,
            compact_request
                .settings
                .optimizer
                .transform_preference_mutation_enabled
        );
        assert_eq!(
            compact_settings.optimizer.placement_policy_mutation_enabled,
            compact_request
                .settings
                .optimizer
                .placement_policy_mutation_enabled
        );
        assert_eq!(
            compact_settings.optimizer.placement_policy_id,
            crate::domain::IrregularPlacementPolicyId::EdgeContactThenBalancedCompactness
        );
        assert_eq!(
            compact_settings.optimizer.placement_policy_ids,
            vec![
                crate::domain::IrregularPlacementPolicyId::ShortSideFill,
                crate::domain::IrregularPlacementPolicyId::BalancedCompactness,
                crate::domain::IrregularPlacementPolicyId::EdgeContactThenBalancedCompactness,
            ]
        );
        assert_eq!(
            compact.options.history_mode,
            crate::result::HistoryMode::Final
        );
        assert_eq!(
            compact.options.allow_global_rotation,
            compact_request.settings.allow_global_rotation
        );
        assert_eq!(
            compact.options.allow_global_mirror,
            Some(compact_request.settings.allow_global_mirror)
        );
        assert_eq!(
            compact
                .pieces
                .iter()
                .map(|piece| piece.id.as_str())
                .collect::<Vec<_>>(),
            vec!["prepared-1", "prepared-2"]
        );
        assert_eq!(
            compact
                .source_pieces
                .iter()
                .map(|piece| piece.id.as_str())
                .collect::<Vec<_>>(),
            vec!["source-1", "source-2"]
        );
        assert_eq!(compact.pieces[0].id.as_str(), compact_request.pieces[0].id);
        assert_eq!(
            compact.pieces[0].padded_bounds.longest_edge,
            compact_request.pieces[0].padded_bounds.longest_edge
        );
        assert_eq!(
            compact.pieces[0]
                .cut_row_ref
                .as_ref()
                .unwrap()
                .customer_name,
            compact_request.pieces[0]
                .cut_row_ref
                .as_ref()
                .unwrap()
                .customer_name
        );
        assert_eq!(compact.pieces[1].real_bounds.width, 21.0);
        assert_eq!(compact.pieces[1].padded_bounds.area, 616.0);
        assert_eq!(compact.pieces[1].cut_row_ref, None);
        assert_eq!(
            compact.source_pieces[0].source_layer,
            compact_request.source_pieces[0].source_layer
        );
        assert_eq!(compact.source_pieces[1].label, "second source label");
        assert_eq!(compact.source_pieces[1].real_bounds.height, 23.0);
        assert_eq!(
            compact.source_pieces[0].geometry.segments.len(),
            compact_request.source_pieces[0].geometry.segments.len()
        );
        let crate::domain::DxfGeometrySegment::Line(line) =
            &compact.source_pieces[0].geometry.segments[0]
        else {
            panic!("first source geometry segment is a line");
        };
        assert_eq!((line.x1, line.y1, line.x2, line.y2), (1.0, 2.0, 3.0, 4.0));
        assert_eq!(line.bulge, Some(0.5));
        let ellipse = line.source_curve.as_ref().unwrap();
        assert_eq!(ellipse.kind, crate::domain::DxfEllipseSourceKind::Ellipse);
        assert_eq!(ellipse.source_id, "ellipse-1");
        assert_eq!(ellipse.cx, 5.0);
        assert_eq!(ellipse.cy, 6.0);
        assert_eq!(ellipse.major_axis_x, 7.0);
        assert_eq!(ellipse.major_axis_y, 8.0);
        assert_eq!(ellipse.axis_ratio, 0.25);
        assert_eq!(ellipse.start_angle, 9.0);
        assert_eq!(ellipse.end_angle, 10.0);
        let crate::domain::DxfGeometrySegment::Arc(arc) =
            &compact.source_pieces[0].geometry.segments[1]
        else {
            panic!("second source geometry segment is an arc");
        };
        assert_eq!(
            (
                arc.x1,
                arc.y1,
                arc.x2,
                arc.y2,
                arc.cx,
                arc.cy,
                arc.radius,
                arc.start_angle,
                arc.end_angle,
            ),
            (11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0)
        );
        assert_eq!(compact.source_pieces[0].warnings[0].code, "warning-code");
        assert_eq!(
            compact.source_pieces[0].warnings[0].message,
            "warning message"
        );
        assert_eq!(
            compact.source_pieces[0].warnings[0].entity_type,
            Some("ELLIPSE".to_string())
        );
        assert_eq!(
            compact.source_pieces[0].warnings[0].entity_handle,
            Some(crate::domain::DxfEntityHandle::Number(42.0))
        );
        assert_eq!(
            compact.source_pieces[1].warnings[0].entity_handle,
            Some(crate::domain::DxfEntityHandle::Text("handle-2".to_string()))
        );
    }

    #[test]
    fn capacity_projection_preserves_nested_optional_enum_and_exact_fields() {
        use crate::canonical_grid::layout::{
            CanonicalLayoutTopology, CanonicalLayoutTopologyExact,
        };
        use crate::capacity::endpoint::{
            IntrinsicCapacityCavityMetrics, IntrinsicCapacityEndpointOrigin,
            IntrinsicCapacityObjective,
        };
        use crate::capacity::mode::{
            IntrinsicCapacityCohesionShadowTrace, IntrinsicCapacityLaneCoordinatorQuantum,
            IntrinsicCapacityLaneCoordinatorTrace, IntrinsicCapacityPrefixDescriptorSummary,
            IntrinsicCapacityPrefixTrace, IntrinsicCapacityQualityWarmPrefixTrace,
            IntrinsicCapacityRouting, IntrinsicCapacitySelectionTrace, IntrinsicCapacityTrace,
            IntrinsicCapacityWarmPrefixLaneTrace, LaneCoordinatorContinuedProducer,
            LaneCoordinatorQuantumOutcome, LaneCoordinatorQuantumPhase,
            LaneCoordinatorQuantumProducerRole, QualityWarmPrefixOutputInfluence,
            QualityWarmPrefixStatus, WarmPrefixLaneStatus,
        };
        use crate::capacity::preflight::{
            IntrinsicCapacityPreflightMeasurements, IntrinsicCapacityPreflightOutcome,
        };
        use crate::capacity::search::{
            IntrinsicCapacityDecision, IntrinsicCapacityProposalRole, IntrinsicCapacitySearchTrace,
            IntrinsicCapacitySettlement, IntrinsicCapacityTopologyRepresentative,
            IntrinsicCapacityTopologyRepresentativeRole,
            IntrinsicCapacityTopologyRetentionDepthTrace,
        };
        use num_bigint::BigInt;

        let objective = || IntrinsicCapacityObjective {
            placed_count: 3.0,
            placed_doubled_material_area_grid2: BigInt::parse_bytes(b"12345678901234567890", 10)
                .unwrap(),
            enclosed_cavity_count: 2.0,
            total_enclosed_cavity_area_mm2: 4.5,
            total_enclosed_cavity_doubled_area_grid2: "9007199254740993".to_string(),
            envelope_maximum_side_mm: 80.0,
            envelope_area_mm2: 1200.0,
            envelope_span_mm: 60.0,
            envelope_maximum_side_grid: 800.0,
            envelope_area_grid2: "123456789012345678901".to_string(),
            envelope_span_grid: 600.0,
            canonical_geometry_hash: "objective-hash".to_string(),
            origin: IntrinsicCapacityEndpointOrigin::WarmPrefixContinuation,
            prefix_depth: Some(2.0),
            source_role: Some("canonical-grid".to_string()),
        };
        let topology = || CanonicalLayoutTopologyExact {
            topology: CanonicalLayoutTopology {
                enclosed_cavity_count: 1.0,
                largest_occupied_hull_gap_ratio: 0.2,
                occupied_envelope_aspect_ratio: 1.5,
                positive_contact_component_count: 2.0,
                isolated_piece_count: 1.0,
                largest_positive_contact_component_size: 2.0,
                largest_positive_contact_component_ratio: 0.66,
            },
            hull_gap_doubled_area_grid2: 12.5,
            hull_doubled_area_grid2: 80.5,
            exact_hull_gap_doubled_area_grid2: "123456789012345678902".to_string(),
            exact_hull_doubled_area_grid2: "123456789012345678903".to_string(),
        };
        let retention = IntrinsicCapacityTopologyRetentionDepthTrace {
            depth: 2.0,
            piece_id: PieceId::new("piece-retained"),
            measured_survivor_count: 9.0,
            retained_count: 4.0,
            best_accounting_stratum_count: 3.0,
            topology_measurement_count: 8.0,
            topology_measurement_ms: 1.25,
            legal_candidate_count: 7.0,
            contact_measured_candidate_count: 6.0,
            positive_contact_candidate_count: 5.0,
            contact_measurement_ms: 1.5,
            contact_selected_successor_count: 4.0,
            contact_deduplicated_successor_count: 3.0,
            contact_retained_successor_count: 2.0,
            representatives: vec![IntrinsicCapacityTopologyRepresentative {
                role: IntrinsicCapacityTopologyRepresentativeRole::MinimumHullWaste,
                decision_identity: "decision-1".to_string(),
                parent_decision_identity: "decision-0".to_string(),
                decision: IntrinsicCapacityDecision::Skip,
                proposal_role: IntrinsicCapacityProposalRole::Contact,
                piece_id: PieceId::new("piece-representative"),
                anchored_occupied_key: "occupied-key".to_string(),
                placed_count: 2.0,
                placed_doubled_material_area_grid2: BigInt::parse_bytes(b"9876543210987654321", 10)
                    .unwrap(),
                cavities: IntrinsicCapacityCavityMetrics {
                    count: 1.0,
                    total_area_mm2: 2.5,
                    total_doubled_area_grid2: "9007199254740994".to_string(),
                },
                grid_span: crate::capacity::endpoint::IntrinsicCapacityGridSpan {
                    width_grid: 44.0,
                    height_grid: 55.0,
                },
                topology: Some(topology()),
                retained: true,
            }],
        };
        let search = IntrinsicCapacitySearchTrace {
            beam_width: 4.0,
            local_legal_placement_fanout: 5.0,
            placement_evaluation_cap: 6.0,
            placement_evaluation_quota_per_depth: 7.0,
            consumed_placement_evaluations: 8.0,
            auxiliary_placement_evaluations: 1.0,
            pruned_by_attainable_count: 2.0,
            pruned_by_attainable_material: 3.0,
            deduplicated_successors: 4.0,
            fit_rejected_candidates: 5.0,
            invalid_candidates: 6.0,
            endpoint_fit_rejections: 7.0,
            completed_depths: 8.0,
            depth_quota_exhaustions: 9.0,
            piece_count: 10.0,
            settlement: IntrinsicCapacitySettlement::EvaluationCap,
            topology_retention_depths: Some(vec![retention.clone()]),
        };
        let measurements = IntrinsicCapacityPreflightMeasurements {
            piece_count: 3.0,
            sheet_width_grid: 100.0,
            sheet_height_grid: 80.0,
            sheet_doubled_area_grid2: BigInt::parse_bytes(b"11111111111111111111", 10).unwrap(),
            minimum_doubled_collision_area_sum_grid2: BigInt::parse_bytes(
                b"22222222222222222222",
                10,
            )
            .unwrap(),
            minimum_collision_area_pressure_ppm: BigInt::parse_bytes(b"33333333333333333333", 10)
                .unwrap(),
            maximum_singleton_span_pressure_ppm: BigInt::parse_bytes(b"44444444444444444444", 10)
                .unwrap(),
            singleton_infeasible_piece_ids: vec![PieceId::new("piece-infeasible")],
        };
        let trace = IntrinsicCapacityTrace {
            routing: IntrinsicCapacityRouting::BoundedCompleteArchiveMiss,
            preflight: IntrinsicCapacityPreflightOutcome::Inconclusive { measurements },
            prefixes: IntrinsicCapacityPrefixTrace {
                captured_count: 3.0,
                fitting_count: 2.0,
                rejected_count: 1.0,
                terminalized_count: 1.0,
                descriptors: vec![
                    IntrinsicCapacityPrefixDescriptorSummary {
                        role: "canonical-grid".to_string(),
                        depth: 1.0,
                    },
                    IntrinsicCapacityPrefixDescriptorSummary {
                        role: "open-pocket-first".to_string(),
                        depth: 2.0,
                    },
                ],
            },
            prefix_incumbent: Some(crate::capacity::mode::IntrinsicCapacityIncumbentTrace {
                source_role: Some("canonical-grid".to_string()),
                prefix_depth: Some(2.0),
                placed_count: 2.0,
                placed_material_area_mm2: 33.0,
                selected_rotation_deg: 90.0,
                canonical_geometry_hash: "incumbent-hash".to_string(),
            }),
            cold_search: search,
            warm_prefix_lanes: Some(vec![IntrinsicCapacityWarmPrefixLaneTrace {
                source_role: "open-pocket-first".to_string(),
                prefix_depth: 3.0,
                reused_placed_count: 2.0,
                status: WarmPrefixLaneStatus::CheckpointedCensored,
                selected_for_continuation: true,
                checkpoint_retained: true,
                consumed_placement_evaluations: 4.0,
                completed_depths: 5.0,
                elapsed_ms: 6.0,
                endpoint: Some(objective()),
            }]),
            warm_prefix_endpoints_admitted: true,
            cohesion_shadow: Some(IntrinsicCapacityCohesionShadowTrace {
                producer_role: "capacity-cohesion-shadow",
                status: "settled",
                output_influence: "none",
                consumed_placement_evaluations: 7.0,
                completed_depths: 8.0,
                elapsed_ms: 9.0,
                endpoint: Some(objective()),
                retention_depths: Some(vec![retention]),
            }),
            quality_warm_prefix: Some(IntrinsicCapacityQualityWarmPrefixTrace {
                version: "intrinsic-capacity-quality-warm-prefix-v1",
                producer_role: "capacity-quality-warm-prefix",
                policy: "quality-frontier",
                status: QualityWarmPrefixStatus::Settled,
                output_influence: QualityWarmPrefixOutputInfluence::StrictCountImprovement,
                source_role: Some("canonical-grid".to_string()),
                prefix_depth: Some(2.0),
                reused_placed_count: 2.0,
                request_piece_count: 4.0,
                minimum_piece_count: 2.0,
                placement_evaluation_cap: 10.0,
                consumed_placement_evaluations: 8.0,
                completed_depths: 6.0,
                checkpoint_retained: true,
                elapsed_ms: 7.0,
                endpoint: Some(objective()),
            }),
            lane_coordinator: Some(IntrinsicCapacityLaneCoordinatorTrace {
                version: "intrinsic-capacity-lane-coordinator-v3",
                aggregate_placement_evaluation_cap: 20.0,
                aggregate_consumed_placement_evaluations: 15.0,
                warm_pilot_depth_boundaries: 3.0,
                continued_producers: vec![
                    LaneCoordinatorContinuedProducer::CapacityCold,
                    LaneCoordinatorContinuedProducer::CapacityWarmPrefix {
                        source_role: "open-pocket-first".to_string(),
                        prefix_depth: 3.0,
                    },
                    LaneCoordinatorContinuedProducer::CapacityQualityWarmPrefix {
                        source_role: "canonical-grid".to_string(),
                        prefix_depth: 2.0,
                    },
                ],
                retained_checkpoint_count: 2.0,
                censored_lane_count: 1.0,
                quanta: vec![
                    IntrinsicCapacityLaneCoordinatorQuantum {
                        ordinal: 0.0,
                        producer_role: LaneCoordinatorQuantumProducerRole::CapacityCold,
                        source_role: None,
                        prefix_depth: None,
                        phase: LaneCoordinatorQuantumPhase::Initial,
                        from_depth: 0.0,
                        to_depth: 1.0,
                        placement_evaluation_delta: 5.0,
                        outcome: LaneCoordinatorQuantumOutcome::Checkpointed,
                    },
                    IntrinsicCapacityLaneCoordinatorQuantum {
                        ordinal: 1.0,
                        producer_role: LaneCoordinatorQuantumProducerRole::CapacityWarmPrefix,
                        source_role: Some("open-pocket-first".to_string()),
                        prefix_depth: Some(3.0),
                        phase: LaneCoordinatorQuantumPhase::Resume,
                        from_depth: 1.0,
                        to_depth: 3.0,
                        placement_evaluation_delta: 10.0,
                        outcome: LaneCoordinatorQuantumOutcome::Censored,
                    },
                ],
            }),
            selected: IntrinsicCapacitySelectionTrace {
                objective: objective(),
                unplaced_count: 1.0,
                placed_material_area_mm2: 99.0,
                selected_rotation_deg: 0.0,
            },
            preflight_runtime_ms: Some(1.0),
            complete_archive_runtime_ms: Some(2.0),
            prefix_terminalization_ms: 3.0,
            cold_search_ms: 4.0,
            runtime_ms: 5.0,
        };

        let projected = project_capacity_trace(&trace);
        assert_eq!(
            projected.routing,
            protocol::result::CapacityRouting::BoundedCompleteArchiveMiss
        );
        assert_eq!(
            projected
                .prefixes
                .descriptors
                .iter()
                .map(|d| (&d.role, d.depth))
                .collect::<Vec<_>>(),
            vec![
                (&"canonical-grid".to_string(), 1.0),
                (&"open-pocket-first".to_string(), 2.0)
            ]
        );
        assert_eq!(
            projected
                .prefix_incumbent
                .as_ref()
                .unwrap()
                .selected_rotation_deg,
            protocol::result::OrthogonalRotation::Deg90
        );
        assert_eq!(
            projected.selected.selected_rotation_deg,
            protocol::result::OrthogonalRotation::Deg0
        );
        assert_eq!(
            projected
                .selected
                .objective
                .placed_doubled_material_area_grid2
                .as_str(),
            "12345678901234567890"
        );
        assert_eq!(
            projected.selected.objective.envelope_area_grid2.as_str(),
            "123456789012345678901"
        );
        assert_eq!(
            projected
                .cold_search
                .topology_retention_depths
                .as_ref()
                .unwrap()[0]
                .representatives[0]
                .topology
                .as_ref()
                .unwrap()
                .exact_hull_doubled_area_grid2
                .as_str(),
            "123456789012345678903"
        );
        assert_eq!(
            projected.warm_prefix_lanes.as_ref().unwrap()[0].status,
            protocol::result::CapacityWarmPrefixStatus::CheckpointedCensored
        );
        assert_eq!(
            projected.quality_warm_prefix.as_ref().unwrap().version,
            protocol::result::CapacityQualityWarmPrefixVersion::V1
        );
        assert_eq!(
            projected.quality_warm_prefix.as_ref().unwrap().source_role,
            Some(protocol::result::CanonicalGridSourceRole::CanonicalGrid)
        );
        assert_eq!(
            projected.lane_coordinator.as_ref().unwrap().version,
            protocol::result::CapacityLaneCoordinatorVersion::V3
        );
        assert_eq!(
            projected
                .lane_coordinator
                .as_ref()
                .unwrap()
                .continued_producers
                .len(),
            3
        );
        assert_eq!(
            projected.lane_coordinator.as_ref().unwrap().quanta[1].outcome,
            protocol::result::CapacityCoordinatorOutcome::Censored
        );
        assert_eq!(projected.preflight_runtime_ms, Some(1.0));
        assert_eq!(projected.complete_archive_runtime_ms, Some(2.0));
    }

    fn valid_request(profile: protocol::EngineProfile) -> protocol::EngineRequest {
        protocol::EngineRequest {
            version: protocol::ProtocolVersion::CURRENT,
            timeout_ms: 1_000.0,
            profile,
            sheet: protocol::SheetSpec {
                width: 100.0,
                height: 80.0,
                label: "sheet label".to_string(),
            },
            pieces: vec![
                protocol::PreparedPiece {
                    id: "prepared-1".to_string(),
                    source_piece_id: "source-1".to_string(),
                    interchangeability_key: Some("interchangeable".to_string()),
                    real_bounds: protocol::Rect {
                        x: 1.0,
                        y: 2.0,
                        width: 30.0,
                        height: 40.0,
                    },
                    padded_bounds: protocol::RectWithMetrics {
                        x: 0.0,
                        y: 1.0,
                        width: 32.0,
                        height: 42.0,
                        longest_edge: 42.0,
                        area: 1_344.0,
                        imbalance: 10.0,
                    },
                    padding: 2.0,
                    allow_rotation: false,
                    allow_mirror: false,
                    cut_row_ref: Some(protocol::CutRowReference {
                        reference: "reference".to_string(),
                        customer_name: "customer".to_string(),
                        csv_row_id: "row-1".to_string(),
                    }),
                },
                protocol::PreparedPiece {
                    id: "prepared-2".to_string(),
                    source_piece_id: "source-2".to_string(),
                    interchangeability_key: None,
                    real_bounds: protocol::Rect {
                        x: 20.0,
                        y: 21.0,
                        width: 21.0,
                        height: 22.0,
                    },
                    padded_bounds: protocol::RectWithMetrics {
                        x: 19.0,
                        y: 20.0,
                        width: 22.0,
                        height: 28.0,
                        longest_edge: 28.0,
                        area: 616.0,
                        imbalance: 6.0,
                    },
                    padding: 1.0,
                    allow_rotation: true,
                    allow_mirror: true,
                    cut_row_ref: None,
                },
            ],
            source_pieces: vec![
                protocol::SourcePiece {
                    id: "source-1".to_string(),
                    source_file_id: "file-1".to_string(),
                    source_layer: Some("layer-1".to_string()),
                    label: "source label".to_string(),
                    real_bounds: protocol::Rect {
                        x: 1.0,
                        y: 2.0,
                        width: 30.0,
                        height: 40.0,
                    },
                    geometry: protocol::SourceGeometry {
                        entity_type: protocol::SourceGeometryEntityType::Ellipse,
                        closed: true,
                        segments: vec![
                            protocol::SourceGeometrySegment::Line(protocol::SourceLineSegment {
                                x1: 1.0,
                                y1: 2.0,
                                x2: 3.0,
                                y2: 4.0,
                                bulge: Some(0.5),
                                source_curve: Some(protocol::EllipseSource {
                                    kind: protocol::EllipseSourceKind::Ellipse,
                                    source_id: "ellipse-1".to_string(),
                                    cx: 5.0,
                                    cy: 6.0,
                                    major_axis_x: 7.0,
                                    major_axis_y: 8.0,
                                    axis_ratio: 0.25,
                                    start_angle: 9.0,
                                    end_angle: 10.0,
                                }),
                            }),
                            protocol::SourceGeometrySegment::Arc(protocol::SourceArcSegment {
                                x1: 11.0,
                                y1: 12.0,
                                x2: 13.0,
                                y2: 14.0,
                                cx: 15.0,
                                cy: 16.0,
                                radius: 17.0,
                                start_angle: 18.0,
                                end_angle: 19.0,
                            }),
                        ],
                    },
                    warnings: vec![protocol::SourceWarning {
                        code: "warning-code".to_string(),
                        message: "warning message".to_string(),
                        entity_type: Some("ELLIPSE".to_string()),
                        entity_handle: Some(protocol::SourceEntityHandle::Number(42.0)),
                    }],
                },
                protocol::SourcePiece {
                    id: "source-2".to_string(),
                    source_file_id: "file-2".to_string(),
                    source_layer: None,
                    label: "second source label".to_string(),
                    real_bounds: protocol::Rect {
                        x: 20.0,
                        y: 21.0,
                        width: 21.0,
                        height: 23.0,
                    },
                    geometry: protocol::SourceGeometry {
                        entity_type: protocol::SourceGeometryEntityType::Line,
                        closed: false,
                        segments: vec![protocol::SourceGeometrySegment::Line(
                            protocol::SourceLineSegment {
                                x1: 20.0,
                                y1: 21.0,
                                x2: 41.0,
                                y2: 44.0,
                                bulge: None,
                                source_curve: None,
                            },
                        )],
                    },
                    warnings: vec![protocol::SourceWarning {
                        code: "second-warning".to_string(),
                        message: "second warning message".to_string(),
                        entity_type: Some("LINE".to_string()),
                        entity_handle: Some(protocol::SourceEntityHandle::Text(
                            "handle-2".to_string(),
                        )),
                    }],
                },
            ],
            settings: protocol::EngineSettings {
                padding: 2.0,
                allow_global_rotation: false,
                allow_global_mirror: false,
                geometry: protocol::GeometrySettings {
                    flattening_sag_tolerance_mm: 0.1,
                    clearance_safety_margin_mm: 0.2,
                    geometry_backend_id: "backend".to_string(),
                    geometry_backend_version: "v1".to_string(),
                },
                optimizer: protocol::OptimizerSettings {
                    order_window: 3.0,
                    beam_width: 4.0,
                    local_candidate_fanout: 5.0,
                    local_repair_budget: 6.0,
                    intrinsic_shared_archive_enabled: true,
                    transform_cap: 7.0,
                    transform_minimum_edge_length_mm: 8.0,
                    transform_angle_deduplication_tolerance_deg: 9.0,
                    configured_rotation_enabled: false,
                    edge_alignment_enabled: false,
                    configured_rotation_deg: vec![10.0, 20.0],
                    ga_enabled: true,
                    baseline_only: false,
                    ga_population: 11.0,
                    ga_generation_budget: 12.0,
                    ga_evaluation_budget: 13.0,
                    ga_time_budget_ms: 14.0,
                    ga_seed: "seed".to_string(),
                    priority_order_mutation_enabled: false,
                    transform_preference_mutation_enabled: false,
                    placement_policy_mutation_enabled: false,
                    placement_policy_id:
                        protocol::PlacementPolicy::EdgeContactThenBalancedCompactness,
                    placement_policy_ids: vec![
                        protocol::PlacementPolicy::ShortSideFill,
                        protocol::PlacementPolicy::BalancedCompactness,
                        protocol::PlacementPolicy::EdgeContactThenBalancedCompactness,
                    ],
                },
            },
            history_mode: protocol::HistoryMode::Final,
            diagnostic_trace_mode: protocol::DiagnosticTraceMode::Full,
        }
    }
}
