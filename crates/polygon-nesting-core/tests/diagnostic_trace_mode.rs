use polygon_nesting_core::{CancelReason, CancellationControl, EngineEventSink, Job};
use polygon_nesting_protocol::{
    DiagnosticTraceMode, EngineErrorCode, EngineOutcome, EngineProfile, EngineRequest,
    EngineSettings, HistoryMode, OptimizerSettings, PlacementPolicy, PreparedPiece,
    ProtocolVersion, Rect, RectWithMetrics, SourceGeometry, SourceGeometryEntityType,
    SourceGeometrySegment, SourceLineSegment, SourcePiece,
};
use serde_json::Value;

#[derive(Default)]
struct NoopSink;

impl EngineEventSink for NoopSink {
    fn emit(&mut self, _event: polygon_nesting_protocol::SequencedEngineEvent) {}
}

#[test]
fn full_and_off_success_match_except_trace_fields_and_timing() {
    let full = run_success(DiagnosticTraceMode::Full);
    let off = run_success(DiagnosticTraceMode::Off);

    let full_json = outcome_json(full);
    let off_json = outcome_json(off);
    assert!(full_json["result"]["intrinsicAnytimeSchedulerTrace"].is_object());
    assert!(full_json["result"]["focusedCompleteReconstructionTrace"].is_object());
    assert!(off_json["result"]["stateSnapshots"]
        .as_array()
        .is_some_and(Vec::is_empty));
    for field in TRACE_FIELDS {
        assert!(!off_json["result"].as_object().unwrap().contains_key(field));
    }
    assert_eq!(normalize(full_json), normalize(off_json));
}

#[test]
fn full_and_off_short_side_success_match_except_trace_fields_and_timing() {
    let full = run_request(
        short_side_request(DiagnosticTraceMode::Full),
        CancellationControl::new(),
    )
    .unwrap();
    let off = run_request(
        short_side_request(DiagnosticTraceMode::Off),
        CancellationControl::new(),
    )
    .unwrap();

    let full_json = outcome_json(full);
    let off_json = outcome_json(off);
    assert!(full_json["result"]["intrinsicShortSideObserverTrace"].is_object());
    assert!(full_json["result"]["intrinsicShortSidePairFoldTrace"].is_object());
    assert!(off_json["result"]["stateSnapshots"]
        .as_array()
        .is_some_and(Vec::is_empty));
    for field in TRACE_FIELDS {
        assert!(!off_json["result"].as_object().unwrap().contains_key(field));
    }
    assert_eq!(normalize(full_json), normalize(off_json));
}

#[test]
fn full_and_off_pre_cancelled_failures_are_identical() {
    let full = run_pre_cancelled(DiagnosticTraceMode::Full);
    let off = run_pre_cancelled(DiagnosticTraceMode::Off);

    assert!(
        matches!(&full, EngineOutcome::Failure { error, .. } if error.category == EngineErrorCode::Cancelled)
    );
    assert_eq!(normalize(outcome_json(full)), normalize(outcome_json(off)));
}

#[test]
fn full_and_off_archive_ineligible_failures_are_identical() {
    let full = run_archive_ineligible(DiagnosticTraceMode::Full);
    let off = run_archive_ineligible(DiagnosticTraceMode::Off);

    assert!(matches!(&full, EngineOutcome::ArchiveIneligible { .. }));
    assert_eq!(normalize(outcome_json(full)), normalize(outcome_json(off)));
}

#[test]
fn full_and_off_invalid_input_failures_are_identical() {
    let mut full_request = valid_request(DiagnosticTraceMode::Full);
    full_request.pieces.clear();
    let mut off_request = valid_request(DiagnosticTraceMode::Off);
    off_request.pieces.clear();

    let full = run_request(full_request, CancellationControl::new());
    let off = run_request(off_request, CancellationControl::new());

    assert!(matches!(&full, Err(error) if error.category == EngineErrorCode::MalformedInput));
    assert_eq!(
        normalize(error_json(full.unwrap_err())),
        normalize(error_json(off.unwrap_err()))
    );
}

const TRACE_FIELDS: [&str; 5] = [
    "capacityTrace",
    "intrinsicAnytimeSchedulerTrace",
    "focusedCompleteReconstructionTrace",
    "intrinsicShortSideObserverTrace",
    "intrinsicShortSidePairFoldTrace",
];

fn run_success(mode: DiagnosticTraceMode) -> EngineOutcome {
    run_request(valid_request(mode), CancellationControl::new()).unwrap()
}

fn short_side_request(mode: DiagnosticTraceMode) -> EngineRequest {
    let mut request = valid_request(mode);
    request.profile = EngineProfile::CompactShortSide;
    request.sheet.height = 200.0;
    request
}

fn run_pre_cancelled(mode: DiagnosticTraceMode) -> EngineOutcome {
    let control = CancellationControl::new();
    assert!(control.cancel(CancelReason::Cancelled));
    run_request(valid_request(mode), control).unwrap()
}

fn run_archive_ineligible(mode: DiagnosticTraceMode) -> EngineOutcome {
    let mut request = valid_request(mode);
    request.settings.optimizer.intrinsic_shared_archive_enabled = false;
    run_request(request, CancellationControl::new()).unwrap()
}

fn run_request(
    request: EngineRequest,
    control: CancellationControl,
) -> Result<EngineOutcome, polygon_nesting_protocol::EngineError> {
    let mut sink = NoopSink;
    Job::new(&request, &control, &mut sink).run()
}

fn outcome_json(outcome: EngineOutcome) -> Value {
    serde_json::to_value(outcome).expect("outcome serializes")
}

fn error_json(error: polygon_nesting_protocol::EngineError) -> Value {
    serde_json::to_value(error).expect("error serializes")
}

fn normalize(mut value: Value) -> Value {
    normalize_object(&mut value);
    value
}

fn normalize_object(value: &mut Value) {
    match value {
        Value::Array(values) => values.iter_mut().for_each(normalize_object),
        Value::Object(fields) => {
            for key in [
                "elapsedMs",
                "runtimeMs",
                "reconstructionElapsedMs",
                "finalScoreElapsedMs",
            ] {
                if fields.contains_key(key) {
                    fields.insert(key.to_owned(), Value::String("<timing>".to_owned()));
                }
            }
            for field in TRACE_FIELDS {
                fields.remove(field);
            }
            fields.values_mut().for_each(normalize_object);
        }
        _ => {}
    }
}

fn valid_request(mode: DiagnosticTraceMode) -> EngineRequest {
    EngineRequest {
        version: ProtocolVersion::CURRENT,
        timeout_ms: 1_000.0,
        profile: EngineProfile::Compact,
        sheet: polygon_nesting_protocol::SheetSpec {
            width: 100.0,
            height: 100.0,
            label: "sheet".to_owned(),
        },
        pieces: vec![PreparedPiece {
            id: "piece".to_owned(),
            source_piece_id: "source".to_owned(),
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
            id: "source".to_owned(),
            source_file_id: "file".to_owned(),
            source_layer: None,
            label: "source".to_owned(),
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
            sheet_edge_clearance_mm: None,
            allow_global_rotation: true,
            allow_global_mirror: false,
            geometry: polygon_nesting_protocol::GeometrySettings {
                flattening_sag_tolerance_mm: 0.1,
                clearance_safety_margin_mm: 0.1,
                geometry_backend_id: "backend".to_owned(),
                geometry_backend_version: "v1".to_owned(),
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
                ga_seed: "seed".to_owned(),
                priority_order_mutation_enabled: true,
                transform_preference_mutation_enabled: true,
                placement_policy_mutation_enabled: true,
                placement_policy_id: PlacementPolicy::BalancedCompactness,
                placement_policy_ids: vec![PlacementPolicy::BalancedCompactness],
            },
        },
        history_mode: HistoryMode::Off,
        diagnostic_trace_mode: mode,
    }
}
