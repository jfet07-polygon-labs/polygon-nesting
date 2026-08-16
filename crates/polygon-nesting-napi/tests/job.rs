use polygon_nesting_core::{CancellationControl, EngineEventSink, Job};
use polygon_nesting_napi::job::{
    cancellation_reason_from_wire, complete_engine_outcome, project_adapter_error,
    project_delivery_failure, project_engine_error, project_panic,
};
use polygon_nesting_protocol::{EngineError, EngineErrorCode, EngineOutcome, SequencedEngineEvent};

fn error(category: EngineErrorCode, operation: &str, message: &str) -> serde_json::Value {
    serde_json::from_str(&project_engine_error(EngineError::new(
        category, operation, message,
    )))
    .expect("valid envelope JSON")
}

#[test]
fn compat_decode_error_resolves_as_the_desktop_failure_envelope() {
    let error = polygon_nesting_napi::compat::decode_desktop_request("{not json")
        .expect_err("invalid request must fail decode");
    let envelope: serde_json::Value =
        serde_json::from_str(&project_adapter_error(error)).expect("valid envelope JSON");
    assert_eq!(envelope["error"]["category"], "worker_protocol_error");
    assert_eq!(envelope["error"]["operation"], "decodeRequestJson");
}

#[test]
fn engine_cancelled_projects_to_desktop_worker_cancelled() {
    let envelope = error(
        EngineErrorCode::Cancelled,
        "computeIrregularNesting",
        "cancelled",
    );
    assert_eq!(envelope["error"]["category"], "worker_cancelled");
}

#[test]
fn engine_deadline_projects_to_desktop_worker_timeout() {
    let envelope = error(
        EngineErrorCode::DeadlineExceeded,
        "computeIrregularNesting",
        "deadline exceeded",
    );
    assert_eq!(envelope["error"]["category"], "worker_timeout");
}

#[test]
fn engine_invalid_geometry_projects_to_desktop_geometry_error() {
    let envelope = error(
        EngineErrorCode::InvalidGeometry,
        "validate",
        "invalid geometry",
    );
    assert_eq!(envelope["error"]["category"], "irregular_geometry_invalid");
}

#[test]
fn engine_failure_projects_to_desktop_no_valid_result() {
    let envelope = error(
        EngineErrorCode::EngineFailure,
        "computeIrregularNesting",
        "no result",
    );
    assert_eq!(envelope["error"]["category"], "irregular_no_valid_result");
}

#[test]
fn source_geometry_failure_projects_to_exact_desktop_category() {
    let envelope: serde_json::Value = serde_json::from_str(&project_engine_error(
        EngineError::new(
            EngineErrorCode::InvalidGeometry,
            "computeIrregularNesting",
            "source geometry missing",
        )
        .with_context("preparedPieceId", "prepared-1")
        .with_context("sourcePieceId", "source-1"),
    ))
    .expect("valid envelope JSON");

    assert_eq!(
        envelope["error"]["category"],
        "irregular_source_geometry_missing"
    );
    assert_eq!(
        envelope["error"]["context"]["preparedPieceId"],
        "prepared-1"
    );
    assert_eq!(envelope["error"]["context"]["sourcePieceId"], "source-1");
}

#[test]
fn scoring_engine_failures_project_to_exact_desktop_category() {
    for (operation, category) in [
        ("runPortfolio", Some("scoring")),
        ("searchPortfolio", Some("search")),
        ("scorePlacement", None),
        ("scoreLayout", None),
    ] {
        let mut error =
            EngineError::new(EngineErrorCode::EngineFailure, operation, "scoring failed")
                .with_context("operation", operation);
        if let Some(category) = category {
            error = error.with_context("category", category);
        }
        let envelope: serde_json::Value =
            serde_json::from_str(&project_engine_error(error)).expect("valid envelope JSON");
        assert_eq!(
            envelope["error"]["category"], "irregular_scoring_error",
            "operation {operation}"
        );
    }
}

#[test]
fn accepted_not_implemented_context_projects_to_exact_desktop_category() {
    let envelope: serde_json::Value = serde_json::from_str(&project_engine_error(
        EngineError::new(
            EngineErrorCode::InternalFailure,
            "extendFreeMaterial",
            "not implemented",
        )
        .with_context("service", "freeMaterial")
        .with_context("operation", "extendFreeMaterial"),
    ))
    .expect("valid envelope JSON");

    assert_eq!(envelope["error"]["category"], "not_implemented");
    assert_eq!(envelope["error"]["context"]["service"], "freeMaterial");
    assert_eq!(
        envelope["error"]["context"]["operation"],
        "extendFreeMaterial"
    );
}

#[test]
fn io_failure_projects_to_conservative_desktop_unknown_error() {
    let envelope = error(
        EngineErrorCode::IoFailure,
        "write-events",
        "event artifact could not be written",
    );
    assert_eq!(envelope["error"]["category"], "unknown_error");
}

#[test]
fn generic_engine_failure_omits_empty_desktop_context() {
    let envelope = error(
        EngineErrorCode::EngineFailure,
        "computeIrregularNesting",
        "no result",
    );
    assert!(envelope["error"].get("context").is_none());
}

#[test]
fn archive_ineligible_projects_each_reason_to_accepted_desktop_routing_failure() {
    for (reason, accepted_text) in [
        ("archive-disabled", "intrinsicSharedArchiveEnabled is false"),
        ("ga-active", "GA is active"),
        ("short-side-fill", "placementPolicyId is 'short-side-fill'"),
    ] {
        let envelope: serde_json::Value = serde_json::from_str(&project_engine_error(
            EngineError::new(
                EngineErrorCode::ArchiveIneligible,
                "archive-ineligible",
                "ignored",
            )
            .with_context("reason", reason),
        ))
        .expect("valid envelope JSON");
        assert_eq!(envelope["error"]["category"], "not_implemented");
        assert_eq!(
            envelope["error"]["operation"],
            "legacy-portfolio-unsupported"
        );
        assert_eq!(envelope["error"]["context"]["service"], "irregular-native");
        assert_eq!(
            envelope["error"]["message"],
            format!(
                "request settings are not intrinsic-shared-archive-eligible ({accepted_text}); this backend only implements the Compact / Compact Short Side archive path and does not emulate TypeScript's legacy windowed-beam/GA path -- route this request to the TypeScript backend."
            )
        );
    }
}

#[test]
fn adapter_projects_delivery_failures_after_terminal_acknowledgement() {
    let envelope: serde_json::Value =
        serde_json::from_str(&project_delivery_failure("QueueFull")).expect("valid envelope JSON");
    assert_eq!(envelope["error"]["category"], "worker_protocol_error");
    assert_eq!(envelope["error"]["operation"], "nativeEventDelivery");
}

#[test]
fn adapter_sanitizes_caught_panics_without_raw_payload() {
    let envelope: serde_json::Value =
        serde_json::from_str(&project_panic("runIrregularJob")).expect("valid envelope JSON");
    assert_eq!(envelope["error"]["category"], "unknown_error");
    assert_eq!(envelope["error"]["context"]["backend"], "rust");
}

#[test]
fn only_documented_cancellation_wire_reasons_are_accepted() {
    assert!(cancellation_reason_from_wire("cancelled").is_some());
    assert!(cancellation_reason_from_wire("timeout").is_some());
    assert!(cancellation_reason_from_wire("deadline").is_none());
}

#[test]
fn n_api_full_and_off_requests_have_equivalent_results() {
    let full = run_small_desktop_request("full");
    let off = run_small_desktop_request("off");
    let full_json: serde_json::Value = serde_json::from_str(&full).expect("full result JSON");
    let off_json: serde_json::Value = serde_json::from_str(&off).expect("off result JSON");

    assert_eq!(full_json["ok"], true);
    assert_eq!(off_json["ok"], true);
    assert!(full_json["result"]["intrinsicAnytimeSchedulerTrace"].is_object());
    for field in TRACE_FIELDS {
        assert!(!off_json["result"].as_object().unwrap().contains_key(field));
    }
    assert_eq!(normalize(full_json), normalize(off_json));
}

#[test]
fn n_api_applies_an_explicit_sheet_edge_clearance_to_the_legacy_placement() {
    let mut request: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/mixed-61/300x300-compact/request.json"
    ))
    .expect("compact fixture JSON");
    request["pieces"]
        .as_array_mut()
        .expect("pieces array")
        .truncate(1);
    request["sourcePieces"]
        .as_array_mut()
        .expect("source pieces array")
        .truncate(1);
    request["padding"] = serde_json::json!(5.0);
    request["sheetEdgeClearanceMm"] = serde_json::json!(5.0);
    request["allowGlobalRotation"] = serde_json::json!(false);
    request["allowGlobalMirror"] = serde_json::json!(false);
    request["pieces"][0]["allowRotation"] = serde_json::json!(false);
    request["pieces"][0]["allowMirror"] = serde_json::json!(false);
    request["options"]["historyMode"] = serde_json::json!("off");
    request["options"]["diagnosticTraceMode"] = serde_json::json!("off");

    let result = run_desktop_request(&request);
    assert_eq!(result["ok"], true);
    let placement = &result["result"]["portfolio"]["placements"][0];
    assert_eq!(placement["transform"]["translateX"], serde_json::json!(2.5));
    assert_eq!(placement["transform"]["translateY"], serde_json::json!(2.5));
}

#[test]
fn shapes_17_result_is_independent_of_production_upload_order() {
    const PRODUCTION_ORDER: [&str; 17] = [
        "shapes-17-15",
        "shapes-17-14",
        "shapes-17-13",
        "shapes-17-12",
        "shapes-17-11",
        "shapes-17-10",
        "shapes-17-9",
        "shapes-17-8",
        "shapes-17-7",
        "shapes-17-6",
        "shapes-17-5",
        "shapes-17-4",
        "shapes-17-3",
        "shapes-17-2",
        "shapes-17-1",
        "shapes-17-17",
        "shapes-17-16",
    ];
    let mut baseline: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/shapes-17/2000x2700-compact/request.json"
    ))
    .expect("fixture JSON");
    baseline["options"]["diagnosticTraceMode"] = serde_json::json!("off");
    let mut permuted = baseline.clone();
    for field in ["pieces", "sourcePieces"] {
        permuted[field]
            .as_array_mut()
            .expect("request array")
            .sort_by_key(|piece| {
                let id = piece["id"].as_str().expect("piece id");
                PRODUCTION_ORDER
                    .iter()
                    .position(|expected| *expected == id)
                    .expect("production order contains every piece")
            });
    }

    let baseline_result = run_desktop_request(&baseline);
    let permuted_result = run_desktop_request(&permuted);
    assert_eq!(normalize(baseline_result), normalize(permuted_result));
}

#[test]
fn issue_21_interchangeable_arc_circle_desktop_request_completes() {
    assert_issue_21_round_family_completes(
        include_str!("../../../tests/fixtures/issue-21/repro-2circles.json"),
        2,
    );
}

#[test]
fn issue_21_interchangeable_chord_circle_desktop_request_completes() {
    assert_issue_21_round_family_completes(
        include_str!("../../../tests/fixtures/issue-21/G-2circles-lines.json"),
        2,
    );
}

#[test]
fn issue_21_exact_production_desktop_request_completes() {
    assert_issue_21_round_family_completes(
        include_str!("../../../tests/fixtures/issue-21/A-exact-production.json"),
        10,
    );
}

fn assert_issue_21_round_family_completes(request: &str, expected_piece_count: usize) {
    let prepared = polygon_nesting_napi::compat::decode_desktop_request(request)
        .expect("desktop request decodes");
    let control = CancellationControl::new();
    let mut sink = NoopSink;

    let outcome = Job::new(&prepared.request, &control, &mut sink)
        .run()
        .expect("job completes");

    let EngineOutcome::Success { result, .. } = outcome else {
        panic!("expected success: {outcome:?}");
    };
    assert_eq!(
        result.placed_collision_geometries.len(),
        expected_piece_count
    );
    assert!(result.unplaced_piece_ids.is_empty());
}

const TRACE_FIELDS: [&str; 5] = [
    "capacityTrace",
    "intrinsicAnytimeSchedulerTrace",
    "focusedCompleteReconstructionTrace",
    "intrinsicShortSideObserverTrace",
    "intrinsicShortSidePairFoldTrace",
];

fn run_small_desktop_request(diagnostic_trace_mode: &str) -> String {
    let mut request: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/mixed-61/300x300-compact/request.json"
    ))
    .expect("fixture JSON");
    request["pieces"]
        .as_array_mut()
        .expect("pieces array")
        .truncate(1);
    request["sourcePieces"]
        .as_array_mut()
        .expect("source pieces array")
        .truncate(1);
    request["options"]["timeoutMs"] = serde_json::json!(1_000.0);
    request["options"]["historyMode"] = serde_json::json!("off");
    request["options"]["diagnosticTraceMode"] = serde_json::json!(diagnostic_trace_mode);
    run_desktop_request(&request).to_string()
}

fn run_desktop_request(request: &serde_json::Value) -> serde_json::Value {
    let prepared = polygon_nesting_napi::compat::decode_desktop_request(&request.to_string())
        .expect("desktop request decodes");
    let control = CancellationControl::new();
    let mut sink = NoopSink;
    let outcome = Job::new(&prepared.request, &control, &mut sink)
        .run()
        .expect("job completes");
    serde_json::from_str(&complete_engine_outcome(outcome)).expect("result JSON")
}

#[derive(Default)]
struct NoopSink;

impl EngineEventSink for NoopSink {
    fn emit(&mut self, _event: SequencedEngineEvent) {}
}

fn normalize(mut value: serde_json::Value) -> serde_json::Value {
    normalize_value(&mut value);
    value
}

fn normalize_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => values.iter_mut().for_each(normalize_value),
        serde_json::Value::Object(fields) => {
            for key in [
                "runtimeMs",
                "elapsedMs",
                "preflightRuntimeMs",
                "completeArchiveRuntimeMs",
                "prefixTerminalizationMs",
                "coldSearchMs",
                "topologyMeasurementMs",
                "contactMeasurementMs",
                "serializedTraceBytes",
                "peakRssDeltaBytes",
            ] {
                if fields.contains_key(key) {
                    fields.insert(
                        key.to_owned(),
                        serde_json::Value::String("<measurement>".to_owned()),
                    );
                }
            }
            for field in TRACE_FIELDS {
                fields.remove(field);
            }
            fields.values_mut().for_each(normalize_value);
        }
        _ => {}
    }
}
