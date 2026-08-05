use polygon_nesting_napi::compat::{
    adapt_desktop_request_to_engine_json, decode_desktop_request, AdapterError,
};
use polygon_nesting_protocol::{EngineProfile, HistoryMode, ProtocolVersion};

const COMPACT_REQUEST: &str =
    include_str!("../../../tests/fixtures/mixed-61/300x300-compact/request.json");

fn decode_error(input: &str) -> AdapterError {
    decode_desktop_request(input).expect_err("request must be rejected")
}

fn normalize_json_numbers(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                normalize_json_numbers(value);
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values_mut() {
                normalize_json_numbers(value);
            }
        }
        serde_json::Value::Number(number) => {
            let number = number.as_f64().expect("fixture number is finite");
            *value = serde_json::Value::Number(
                serde_json::Number::from_f64(number).expect("fixture number is representable"),
            );
        }
        _ => {}
    }
}

#[test]
fn public_adapter_serializes_the_production_desktop_decoder_output() {
    let adapted: serde_json::Value = serde_json::from_str(
        &adapt_desktop_request_to_engine_json(COMPACT_REQUEST).expect("fixture adapts"),
    )
    .expect("adapter output is neutral engine request JSON");
    assert_eq!(adapted["version"], 1);
    assert_eq!(adapted["profile"], "compact");
    assert!(adapted.get("jobId").is_none());
}

#[test]
fn public_adapter_rejects_desktop_request_with_job_id_only_after_decoding() {
    let invalid = r#"{"version":1,"jobId":"only-a-job"}"#;
    assert!(adapt_desktop_request_to_engine_json(invalid).is_err());
}

#[test]
fn frozen_desktop_request_converts_to_neutral_engine_request() {
    let desktop: serde_json::Value = serde_json::from_str(COMPACT_REQUEST).expect("fixture JSON");
    let prepared = decode_desktop_request(COMPACT_REQUEST).expect("frozen request decodes");
    let expected_timeout_ms = desktop["options"]["timeoutMs"]
        .as_f64()
        .expect("fixture timeout is binary64");
    assert_eq!(prepared.request.timeout_ms, expected_timeout_ms);
    assert_eq!(prepared.job_id, "run-differential-mixed-61-300x300");
    assert_eq!(prepared.strategy_run_id, None);
    let request = prepared.request;

    assert_eq!(request.version, ProtocolVersion::CURRENT);
    assert_eq!(request.profile, EngineProfile::Compact);
    assert_eq!(request.timeout_ms, 9_007_199_254_740_991.0);
    assert_eq!(request.history_mode, HistoryMode::Off);
    assert_eq!(request.settings.padding, 10.0);
    assert!(request.settings.allow_global_rotation);
    assert!(request.settings.allow_global_mirror);
    assert_eq!(request.pieces.len(), 61);
    assert_eq!(request.source_pieces.len(), 61);
    assert_eq!(request.settings.geometry.flattening_sag_tolerance_mm, 0.25);
    assert_eq!(
        request.settings.optimizer.transform_minimum_edge_length_mm,
        1.2
    );

    let desktop: serde_json::Value = serde_json::from_str(COMPACT_REQUEST).expect("fixture JSON");
    let mut optimizer = desktop["options"]["irregularSettings"]["optimizer"].clone();
    optimizer
        .as_object_mut()
        .expect("optimizer object")
        .remove("intrinsicObjectiveProfileId");
    let mut expected = serde_json::json!({
        "version": 1,
        "timeoutMs": desktop["options"]["timeoutMs"],
        "profile": "compact",
        "sheet": desktop["sheet"],
        "pieces": desktop["pieces"],
        "sourcePieces": desktop["sourcePieces"],
        "settings": {
            "padding": desktop["padding"],
            "allowGlobalRotation": desktop["options"]["allowGlobalRotation"],
            "allowGlobalMirror": desktop["options"]["allowGlobalMirror"],
            "geometry": desktop["options"]["irregularSettings"]["geometry"],
            "optimizer": optimizer,
        },
        "historyMode": desktop["options"]["historyMode"],
    });
    let mut actual = serde_json::to_value(&request).expect("engine request JSON");
    normalize_json_numbers(&mut expected);
    normalize_json_numbers(&mut actual);
    for field in [
        "version",
        "timeoutMs",
        "profile",
        "sheet",
        "pieces",
        "sourcePieces",
        "settings",
        "historyMode",
    ] {
        assert_eq!(
            actual[field], expected[field],
            "desktop mapping mismatch at {field}"
        );
    }
}

#[test]
fn desktop_compatibility_keeps_unknown_fields_and_omitted_defaults() {
    let mut value: serde_json::Value = serde_json::from_str(COMPACT_REQUEST).expect("fixture JSON");
    value["futureDesktopField"] = serde_json::json!(true);
    value["options"]["futureDesktopOption"] = serde_json::json!(true);
    value
        .as_object_mut()
        .expect("request object")
        .remove("sourcePieces");
    value["options"]
        .as_object_mut()
        .expect("options object")
        .remove("allowGlobalMirror");
    value["pieces"][0]
        .as_object_mut()
        .expect("piece object")
        .remove("allowMirror");

    let prepared = decode_desktop_request(&value.to_string()).expect("compatibility decode");
    assert!(prepared.request.settings.allow_global_mirror);
    assert!(prepared.request.pieces[0].allow_mirror);
}

#[test]
fn missing_job_id_preserves_desktop_protocol_error() {
    let mut value: serde_json::Value = serde_json::from_str(COMPACT_REQUEST).expect("fixture JSON");
    value
        .as_object_mut()
        .expect("request object")
        .remove("jobId");

    let error = decode_error(&value.to_string());
    assert_eq!(error.category, "worker_protocol_error");
    assert_eq!(error.operation, "decodeRequestJson");
    assert!(error.context.is_empty());
}

#[test]
fn invalid_job_id_preserves_desktop_revalidation_error() {
    for job_id in ["", "  \t"] {
        let mut value: serde_json::Value =
            serde_json::from_str(COMPACT_REQUEST).expect("fixture JSON");
        value["jobId"] = serde_json::json!(job_id);

        let error = decode_error(&value.to_string());
        assert_eq!(error.category, "irregular_geometry_invalid");
        assert_eq!(error.operation, "nativeBoundaryRevalidation");
        assert_eq!(error.message, "jobId must be a non-empty string");
        assert!(error.context.is_empty());
    }
}

#[test]
fn version_mismatch_preserves_desktop_protocol_error() {
    let mut value: serde_json::Value = serde_json::from_str(COMPACT_REQUEST).expect("fixture JSON");
    value["version"] = serde_json::json!(2);

    let error = decode_error(&value.to_string());
    assert_eq!(error.category, "worker_protocol_error");
    assert_eq!(error.operation, "nativeApiVersion");
    assert_eq!(
        error.message,
        "expected NestingRequest.version 1, received 2"
    );
    assert_eq!(
        error.context,
        std::collections::BTreeMap::from([
            ("nativeApiVersion".to_string(), "1".to_string()),
            ("receivedVersion".to_string(), "2".to_string()),
        ])
    );
}

#[test]
fn malformed_json_preserves_desktop_protocol_error() {
    let input = "{not json";
    let error = decode_error(input);
    let parse_error = serde_json::from_str::<serde_json::Value>(input).expect_err("invalid JSON");

    assert_eq!(error.category, "worker_protocol_error");
    assert_eq!(error.operation, "decodeRequestJson");
    assert_eq!(
        error.message,
        format!("request JSON did not decode as a NestingRequest: {parse_error}")
    );
    assert!(error.context.is_empty());
}

#[test]
fn invalid_strategy_run_id_preserves_desktop_revalidation_error() {
    let mut value: serde_json::Value = serde_json::from_str(COMPACT_REQUEST).expect("fixture JSON");
    value["strategyRunId"] = serde_json::json!("");

    let error = decode_error(&value.to_string());
    assert_eq!(error.category, "irregular_geometry_invalid");
    assert_eq!(error.operation, "nativeBoundaryRevalidation");
    assert_eq!(error.message, "strategyRunId must be a non-empty string");
    assert!(error.context.is_empty());
}

#[test]
fn zero_timeout_projects_protocol_validation_as_desktop_revalidation_error() {
    let mut value: serde_json::Value = serde_json::from_str(COMPACT_REQUEST).expect("fixture JSON");
    value["options"]["timeoutMs"] = serde_json::json!(0);

    let error = decode_error(&value.to_string());
    assert_eq!(error.category, "irregular_geometry_invalid");
    assert_eq!(error.operation, "nativeBoundaryRevalidation");
    assert_eq!(
        error.message,
        "invalid protocol field timeoutMs: must be greater than zero"
    );
    assert!(error.context.is_empty());
}

#[test]
fn unknown_history_mode_preserves_desktop_revalidation_error() {
    let mut value: serde_json::Value = serde_json::from_str(COMPACT_REQUEST).expect("fixture JSON");
    value["options"]["historyMode"] = serde_json::json!("legacy");

    let error = decode_error(&value.to_string());
    assert_eq!(error.category, "irregular_geometry_invalid");
    assert_eq!(error.operation, "nativeBoundaryRevalidation");
    assert_eq!(
        error.message,
        "options.historyMode must be one of 'stream' | 'final' | 'off', received \"legacy\""
    );
    assert!(error.context.is_empty());
}

#[test]
fn overflowing_json_number_preserves_desktop_revalidation_error() {
    let input =
        COMPACT_REQUEST.replacen("\"timeoutMs\": 9007199254740991", "\"timeoutMs\": 1e400", 1);
    let error = decode_error(&input);
    let parse_error =
        serde_json::from_str::<serde_json::Value>(&input).expect_err("overflowing JSON");

    assert_eq!(error.category, "irregular_geometry_invalid");
    assert_eq!(error.operation, "nativeBoundaryRevalidation");
    assert_eq!(
        error.message,
        format!("request numeric field must be finite and within binary64 range: {parse_error}")
    );
    assert!(error.context.is_empty());
}

#[test]
fn invalid_worker_mode_preserves_desktop_revalidation_error() {
    let mut value: serde_json::Value = serde_json::from_str(COMPACT_REQUEST).expect("fixture JSON");
    value["options"]["workerMode"] = serde_json::json!("maxrects-beam-search");

    let error = decode_error(&value.to_string());
    assert_eq!(error.category, "irregular_geometry_invalid");
    assert_eq!(error.operation, "nativeBoundaryRevalidation");
    assert_eq!(
        error.message,
        "options.workerMode must be 'irregular-convex-v2', received \"maxrects-beam-search\""
    );
    assert!(error.context.is_empty());
}
