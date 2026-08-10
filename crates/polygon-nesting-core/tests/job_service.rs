use std::sync::{Arc, Mutex};

use polygon_nesting_core::{CancelReason, CancellationControl, EngineEventSink, Job};
use polygon_nesting_protocol::{
    EngineEvent, EngineOutcome, EngineProfile, EngineRequest, EngineSettings, GeometrySettings,
    HistoryMode, OptimizerSettings, PlacementPolicy, PreparedPiece, ProtocolVersion, Rect,
    RectWithMetrics, SourceArcSegment, SourceGeometry, SourceGeometryEntityType,
    SourceGeometrySegment, SourceLineSegment, SourcePiece,
};

#[derive(Default)]
struct RecordingSink {
    events: Arc<Mutex<Vec<(bool, polygon_nesting_protocol::SequencedEngineEvent)>>>,
    control: Option<(Arc<CancellationControl>, CancelReason)>,
}

impl EngineEventSink for RecordingSink {
    fn emit(&mut self, event: polygon_nesting_protocol::SequencedEngineEvent) {
        self.events
            .lock()
            .unwrap()
            .push((rayon::current_thread_index().is_some(), event.clone()));
        if event.ordinal == 0 {
            if let Some((control, reason)) = &self.control {
                assert!(control.cancel(*reason));
            }
        }
    }
}

#[test]
fn typed_job_runs_on_its_job_pool_and_emits_semantic_events() {
    let request = valid_request();
    let control = CancellationControl::new();
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut sink = RecordingSink {
        events: Arc::clone(&events),
        control: None,
    };

    let outcome = Job::new(&request, &control, &mut sink).run().unwrap();

    let EngineOutcome::Success {
        result,
        diagnostics,
    } = outcome
    else {
        panic!("expected success: {outcome:?}");
    };
    assert!(!result.state_snapshots.is_empty());
    assert!(!result.placed_collision_geometries.is_empty());
    assert!(result.intrinsic_anytime_scheduler_trace.is_some());
    assert!(result.focused_complete_reconstruction_trace.is_some());
    assert!(result.capacity_trace.is_none());
    assert!(diagnostics.requested_workers.unwrap() > 0);
    assert!(diagnostics.actual_workers.unwrap() > 0);
    let events = events.lock().unwrap();
    assert!(!events.is_empty());
    assert!(events.iter().all(|(on_worker, _)| *on_worker));
    let (_, first) = &events[0];
    assert!(matches!(
        first.event,
        EngineEvent::PortfolioProgress {
            progress: polygon_nesting_protocol::PortfolioProgress {
                phase: polygon_nesting_protocol::PortfolioPhase::SharedArchive,
                best_score: None,
                elapsed_ms: 0.0,
            },
        }
    ));
    assert!(events
        .iter()
        .enumerate()
        .all(|(index, (_, event))| event.ordinal == index as u64));
}

#[test]
fn compact_archive_completes_for_interchangeable_circle_copies() {
    let request = interchangeable_circle_copies_request();
    let control = CancellationControl::new();
    let mut sink = RecordingSink::default();

    let outcome = Job::new(&request, &control, &mut sink).run().unwrap();

    let EngineOutcome::Success { result, .. } = outcome else {
        panic!("expected success: {outcome:?}");
    };
    assert_eq!(result.placed_collision_geometries.len(), 2);
    assert!(result.unplaced_piece_ids.is_empty());
}

#[test]
fn compact_archive_completes_for_interchangeable_regular_polygon_copies() {
    let request = interchangeable_regular_polygon_copies_request();
    let control = CancellationControl::new();
    let mut sink = RecordingSink::default();

    let outcome = Job::new(&request, &control, &mut sink).run().unwrap();

    let EngineOutcome::Success { result, .. } = outcome else {
        panic!("expected success: {outcome:?}");
    };
    assert_eq!(result.placed_collision_geometries.len(), 2);
    assert!(result.unplaced_piece_ids.is_empty());
}

#[test]
fn compact_short_side_projects_short_side_traces_when_emitted() {
    let mut request = valid_request();
    request.profile = EngineProfile::CompactShortSide;
    let control = CancellationControl::new();
    let mut sink = RecordingSink::default();
    let outcome = Job::new(&request, &control, &mut sink).run().unwrap();
    let EngineOutcome::Success { result, .. } = outcome else {
        panic!("expected success")
    };
    assert!(result.intrinsic_short_side_observer_trace.is_some());
    assert!(result.intrinsic_short_side_pair_fold_trace.is_some());
}

#[test]
fn typed_job_returns_archive_ineligible_before_events() {
    for (reason, configure) in [
        (
            polygon_nesting_protocol::ArchiveIneligibilityReason::ArchiveDisabled,
            archive_disabled as fn(&mut EngineRequest),
        ),
        (
            polygon_nesting_protocol::ArchiveIneligibilityReason::ShortSideFill,
            short_side_fill,
        ),
        (
            polygon_nesting_protocol::ArchiveIneligibilityReason::GaActive,
            ga_active,
        ),
    ] {
        let mut request = valid_request();
        configure(&mut request);
        let control = CancellationControl::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut sink = RecordingSink {
            events: Arc::clone(&events),
            control: None,
        };
        let outcome = Job::new(&request, &control, &mut sink).run().unwrap();
        assert!(
            matches!(outcome, EngineOutcome::ArchiveIneligible { reason: actual, diagnostics, .. } if actual == reason && diagnostics.is_empty())
        );
        assert!(events.lock().unwrap().is_empty());
    }
}

#[test]
fn typed_job_projects_pre_cancelled_control_before_archive_ineligible() {
    for (reason, expected) in [
        (
            CancelReason::Cancelled,
            polygon_nesting_protocol::EngineErrorCode::Cancelled,
        ),
        (
            CancelReason::Deadline,
            polygon_nesting_protocol::EngineErrorCode::DeadlineExceeded,
        ),
    ] {
        let mut request = valid_request();
        archive_disabled(&mut request);
        let control = CancellationControl::new();
        assert!(control.cancel(reason));
        let mut sink = RecordingSink::default();

        let outcome = Job::new(&request, &control, &mut sink)
            .run()
            .expect("pre-cancelled jobs should return a typed outcome");

        assert!(
            matches!(outcome, EngineOutcome::Failure { error, diagnostics } if error.category == expected && diagnostics.is_empty())
        );
        assert!(sink.events.lock().unwrap().is_empty());
    }
}

#[test]
fn typed_job_reports_post_cleanup_cache_telemetry() {
    let request = valid_request();
    let control = CancellationControl::new();
    let mut sink = RecordingSink::default();
    let outcome = Job::new(&request, &control, &mut sink).run().unwrap();
    let EngineOutcome::Success { diagnostics, .. } = outcome else {
        panic!("expected success")
    };
    assert_eq!(diagnostics.counters["geometry_cache.current_bytes"], 0);
    assert_eq!(diagnostics.counters["free_material_cache.current_bytes"], 0);
    assert_eq!(diagnostics.counters["free_material_cache.entries"], 0);
    assert!(
        diagnostics.counters["geometry_cache.peak_bytes"] > 0
            || diagnostics.counters["free_material_cache.peak_bytes"] > 0
    );
}

#[test]
fn typed_job_projects_cancelled_control_as_failure() {
    assert_controlled_failure(
        CancelReason::Cancelled,
        polygon_nesting_protocol::EngineErrorCode::Cancelled,
    );
}

#[test]
fn typed_job_projects_deadline_control_as_failure() {
    assert_controlled_failure(
        CancelReason::Deadline,
        polygon_nesting_protocol::EngineErrorCode::DeadlineExceeded,
    );
}

fn assert_controlled_failure(
    reason: CancelReason,
    expected: polygon_nesting_protocol::EngineErrorCode,
) {
    let request = valid_request();
    let control = Arc::new(CancellationControl::new());
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut sink = RecordingSink {
        events: Arc::clone(&events),
        control: Some((Arc::clone(&control), reason)),
    };

    let outcome = Job::new(&request, &control, &mut sink).run().unwrap();

    assert!(matches!(outcome, EngineOutcome::Failure { error, .. } if error.category == expected));
    let events = events.lock().unwrap();
    assert_eq!(events[0].1.ordinal, 0);
}

fn archive_disabled(request: &mut EngineRequest) {
    request.settings.optimizer.intrinsic_shared_archive_enabled = false;
}

fn short_side_fill(request: &mut EngineRequest) {
    request.settings.optimizer.placement_policy_id = PlacementPolicy::ShortSideFill;
    request.settings.optimizer.placement_policy_ids = vec![PlacementPolicy::ShortSideFill];
}

fn ga_active(request: &mut EngineRequest) {
    request.settings.optimizer.ga_enabled = true;
    request.settings.optimizer.baseline_only = false;
    request.settings.optimizer.ga_generation_budget = 1.0;
    request.settings.optimizer.ga_evaluation_budget = 1.0;
    request.settings.optimizer.ga_time_budget_ms = 1.0;
}

fn interchangeable_regular_polygon_copies_request() -> EngineRequest {
    let mut request = interchangeable_circle_copies_request();
    let radius = 52.5;
    let center = 52.5;
    let vertex_count = 64usize;
    let segments = (0..vertex_count)
        .map(|index| {
            let angle = std::f64::consts::TAU * index as f64 / vertex_count as f64;
            let next_angle = std::f64::consts::TAU * (index + 1) as f64 / vertex_count as f64;
            SourceGeometrySegment::Line(SourceLineSegment {
                x1: center + radius * angle.cos(),
                y1: center + radius * angle.sin(),
                x2: center + radius * next_angle.cos(),
                y2: center + radius * next_angle.sin(),
                bulge: None,
                source_curve: None,
            })
        })
        .collect();
    let geometry = SourceGeometry {
        entity_type: SourceGeometryEntityType::Lwpolyline,
        closed: true,
        segments,
    };
    request.source_pieces[0].geometry = geometry.clone();
    request.source_pieces[1].geometry = geometry;
    request
}

fn interchangeable_circle_copies_request() -> EngineRequest {
    let mut request = valid_request();
    let real_bounds = Rect {
        x: 0.0,
        y: 0.0,
        width: 105.0,
        height: 105.0,
    };
    let padded_bounds = RectWithMetrics {
        x: 0.0,
        y: 0.0,
        width: 115.0,
        height: 115.0,
        longest_edge: 115.0,
        area: 13_225.0,
        imbalance: 0.0,
    };
    let circle_geometry = SourceGeometry {
        entity_type: SourceGeometryEntityType::Circle,
        closed: true,
        segments: vec![
            SourceGeometrySegment::Arc(SourceArcSegment {
                x1: 105.0,
                y1: 52.5,
                x2: 0.0,
                y2: 52.5,
                cx: 52.5,
                cy: 52.5,
                radius: 52.5,
                start_angle: 0.0,
                end_angle: 180.0,
            }),
            SourceGeometrySegment::Arc(SourceArcSegment {
                x1: 0.0,
                y1: 52.5,
                x2: 105.0,
                y2: 52.5,
                cx: 52.5,
                cy: 52.5,
                radius: 52.5,
                start_angle: 180.0,
                end_angle: 360.0,
            }),
        ],
    };

    request.timeout_ms = 60_000.0;
    request.sheet.width = 2_400.0;
    request.sheet.height = 1_500.0;
    request.sheet.label = "2400x1500 circle regression sheet".to_string();
    request.settings.padding = 10.0;
    request.settings.optimizer.transform_cap = 8.0;
    request.settings.optimizer.transform_minimum_edge_length_mm = 1.2;
    request
        .settings
        .optimizer
        .transform_angle_deduplication_tolerance_deg = 0.051;
    request.history_mode = HistoryMode::Off;
    request.diagnostic_trace_mode = polygon_nesting_protocol::DiagnosticTraceMode::Off;
    request.pieces = vec![
        PreparedPiece {
            id: "circle-piece-1".to_string(),
            source_piece_id: "circle-source-1".to_string(),
            interchangeability_key: Some("round-family".to_string()),
            real_bounds: real_bounds.clone(),
            padded_bounds: padded_bounds.clone(),
            padding: 5.0,
            allow_rotation: true,
            allow_mirror: false,
            cut_row_ref: None,
        },
        PreparedPiece {
            id: "circle-piece-2".to_string(),
            source_piece_id: "circle-source-2".to_string(),
            interchangeability_key: Some("round-family".to_string()),
            real_bounds: real_bounds.clone(),
            padded_bounds: padded_bounds.clone(),
            padding: 5.0,
            allow_rotation: true,
            allow_mirror: false,
            cut_row_ref: None,
        },
    ];
    request.source_pieces = vec![
        SourcePiece {
            id: "circle-source-1".to_string(),
            source_file_id: "circle-file-1".to_string(),
            source_layer: None,
            label: "circle-1.dxf".to_string(),
            real_bounds: real_bounds.clone(),
            geometry: circle_geometry.clone(),
            warnings: Vec::new(),
        },
        SourcePiece {
            id: "circle-source-2".to_string(),
            source_file_id: "circle-file-2".to_string(),
            source_layer: None,
            label: "circle-2.dxf".to_string(),
            real_bounds,
            geometry: circle_geometry,
            warnings: Vec::new(),
        },
    ];
    request
}

fn valid_request() -> EngineRequest {
    EngineRequest {
        version: ProtocolVersion::CURRENT,
        timeout_ms: 1_000.0,
        profile: EngineProfile::Compact,
        sheet: polygon_nesting_protocol::SheetSpec {
            width: 100.0,
            height: 100.0,
            label: "sheet".to_string(),
        },
        pieces: vec![PreparedPiece {
            id: "piece".to_string(),
            source_piece_id: "source".to_string(),
            interchangeability_key: None,
            real_bounds: Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            padded_bounds: RectWithMetrics {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
                longest_edge: 10.0,
                area: 100.0,
                imbalance: 0.0,
            },
            padding: 0.0,
            allow_rotation: true,
            allow_mirror: false,
            cut_row_ref: None,
        }],
        source_pieces: vec![SourcePiece {
            id: "source".to_string(),
            source_file_id: "file".to_string(),
            source_layer: None,
            label: "source".to_string(),
            real_bounds: Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            geometry: SourceGeometry {
                entity_type: SourceGeometryEntityType::Line,
                closed: true,
                segments: vec![
                    SourceGeometrySegment::Line(SourceLineSegment {
                        x1: 0.0,
                        y1: 0.0,
                        x2: 10.0,
                        y2: 0.0,
                        bulge: None,
                        source_curve: None,
                    }),
                    SourceGeometrySegment::Line(SourceLineSegment {
                        x1: 10.0,
                        y1: 0.0,
                        x2: 10.0,
                        y2: 10.0,
                        bulge: None,
                        source_curve: None,
                    }),
                    SourceGeometrySegment::Line(SourceLineSegment {
                        x1: 10.0,
                        y1: 10.0,
                        x2: 0.0,
                        y2: 10.0,
                        bulge: None,
                        source_curve: None,
                    }),
                    SourceGeometrySegment::Line(SourceLineSegment {
                        x1: 0.0,
                        y1: 10.0,
                        x2: 0.0,
                        y2: 0.0,
                        bulge: None,
                        source_curve: None,
                    }),
                ],
            },
            warnings: Vec::new(),
        }],
        settings: EngineSettings {
            padding: 0.0,
            allow_global_rotation: true,
            allow_global_mirror: false,
            geometry: GeometrySettings {
                flattening_sag_tolerance_mm: 0.1,
                clearance_safety_margin_mm: 0.1,
                geometry_backend_id: "backend".to_string(),
                geometry_backend_version: "v1".to_string(),
            },
            optimizer: OptimizerSettings {
                order_window: 1.0,
                beam_width: 1.0,
                local_candidate_fanout: 1.0,
                local_repair_budget: 0.0,
                intrinsic_shared_archive_enabled: true,
                transform_cap: 4.0,
                transform_minimum_edge_length_mm: 1.0,
                transform_angle_deduplication_tolerance_deg: 0.01,
                configured_rotation_enabled: true,
                edge_alignment_enabled: true,
                configured_rotation_deg: Vec::new(),
                ga_enabled: false,
                baseline_only: true,
                ga_population: 1.0,
                ga_generation_budget: 0.0,
                ga_evaluation_budget: 0.0,
                ga_time_budget_ms: 0.0,
                ga_seed: "seed".to_string(),
                priority_order_mutation_enabled: true,
                transform_preference_mutation_enabled: true,
                placement_policy_mutation_enabled: true,
                placement_policy_id: PlacementPolicy::BalancedCompactness,
                placement_policy_ids: vec![PlacementPolicy::BalancedCompactness],
            },
        },
        history_mode: HistoryMode::Stream,
        diagnostic_trace_mode: polygon_nesting_protocol::DiagnosticTraceMode::Full,
    }
}
