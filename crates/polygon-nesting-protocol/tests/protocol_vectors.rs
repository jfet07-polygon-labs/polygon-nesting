use polygon_nesting_protocol::result::CapacityLaneCoordinatorTrace;
use polygon_nesting_protocol::{
    decode_request, encode_event, encode_outcome, encode_request, ArchiveIneligibilityReason,
    CapacityTrace, DiagnosticTraceMode, EngineError, EngineErrorCode, EngineEvent, EngineOutcome,
    EngineProfile, EngineResult, ExactDecimalString, ExecutionDiagnostics,
    FocusedCompleteReconstructionTrace, FreeMaterialSnapshot, IntrinsicAnytimeSchedulerTrace,
    IntrinsicShortSideObserverTrace, IntrinsicShortSidePairFoldTrace, IrregularTransformReason,
    LayoutScoreSummary, PortfolioPhase, PortfolioProgress, PortfolioResult, ProtocolError,
    ProtocolVersion, SequencedEngineEvent, SnapshotPreparedPiece, StateSnapshot,
    EXACT_DECIMAL_FIELD_NAMES, PROTOCOL_VERSION,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

const REQUEST_VECTOR: &str = include_str!("../../../tests/vectors/protocol/request-v1.json");
const EVENT_VECTOR: &str = include_str!("../../../tests/vectors/protocol/event-v1.json");
const STATE_SNAPSHOT_EVENT_VECTOR: &str =
    include_str!("../../../tests/vectors/protocol/state-snapshot-event-v1.json");
const ARCHIVE_INELIGIBLE_OUTCOME_VECTOR: &str =
    include_str!("../../../tests/vectors/protocol/archive-ineligible-outcome-v1.json");
const PROVENANCE: &str = include_str!("../../../tests/vectors/protocol/provenance.json");
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

fn request_value() -> Value {
    serde_json::from_str(REQUEST_VECTOR).expect("frozen neutral request parses")
}

fn source_piece_value() -> Value {
    json!({
        "id": "source-1",
        "sourceFileId": "file-1",
        "sourceLayer": "CUT",
        "label": "source piece",
        "realBounds": { "x": 0, "y": 0, "width": 10, "height": 10 },
        "geometry": {
            "entityType": "LWPOLYLINE",
            "closed": true,
            "segments": [
                { "kind": "line", "x1": 0, "y1": 0, "x2": 10, "y2": 0 }
            ]
        },
        "warnings": []
    })
}

fn short_side_orientation_value(rotation_deg: f64, canonical_geometry_hash: Option<&str>) -> Value {
    let mut value = json!({
        "rotationDeg": rotation_deg,
        "exactLegal": true,
        "cavityCount": 0.0,
        "hullGapRatio": 0.0,
        "cohesionPasses": true,
        "cohesionDeficit": 0.0,
        "intrinsicEnvelopeAreaMm2": 100.0,
        "intrinsicEnvelopeMaximumSideMm": 10.0,
        "intrinsicEnvelopeSpanMm": 20.0,
        "dominantStructuralContacts": 1.0,
        "totalStructuralContacts": 1.0,
        "contactUnits": 10.0,
        "sharedBoundaryLengthMm": 5.0,
        "comparisonTuple": [
            0.0, 1.0, 2.0, 0.0, 0.0, 0.0, 0.0, 100.0, 10.0, 20.0,
            -1.0, -1.0, -10.0, -5.0,
            canonical_geometry_hash.unwrap_or("\u{ffff}")
        ]
    });
    if let Some(hash) = canonical_geometry_hash {
        value["canonicalGeometryHash"] = json!(hash);
    }
    value
}

fn decode_value(value: &Value) -> Result<polygon_nesting_protocol::EngineRequest, ProtocolError> {
    decode_request(serde_json::to_vec(value).expect("request serializes"))
}

fn empty_success() -> EngineOutcome {
    EngineOutcome::Success {
        result: EngineResult::default(),
        diagnostics: ExecutionDiagnostics::default(),
    }
}

fn assert_json_round_trip<T>(value: Value)
where
    T: DeserializeOwned + Serialize,
{
    let decoded: T = serde_json::from_value(value.clone()).expect("typed value decodes");
    assert_eq!(
        serde_json::to_value(decoded).expect("typed value re-encodes"),
        value
    );
}

fn assert_root_exact_decimal_fields<T>(valid: Value, fields: &[&str])
where
    T: DeserializeOwned,
{
    serde_json::from_value::<T>(valid.clone()).expect("canonical decimal strings decode");
    for field in fields {
        let mut invalid = valid.clone();
        invalid[*field] = json!(1);
        assert!(
            serde_json::from_value::<T>(invalid).is_err(),
            "accepted JSON number for {field}"
        );
    }
}

#[test]
fn frozen_request_vector_preserves_source_sheet_label() {
    assert_eq!(request_value()["sheet"]["label"], json!("2000x2700"));
}

#[test]
fn frozen_vector_records_portable_source_provenance() {
    let provenance: Value = serde_json::from_str(PROVENANCE).expect("provenance parses");
    assert_eq!(provenance["version"], json!(1));
    assert_eq!(
        provenance["artifacts"]["request-v1.json"]["sourceArtifactSha256"],
        json!("69103cfdc60e38e9efa028bf33f320c36eecc0b0417a839953990cdd1cc4f6f2")
    );
    assert_eq!(
        provenance["artifacts"]["event-v1.json"]["sourceArtifactSha256"],
        json!("865993c508b4e85fa2d7b62deb19529424eeed4dc88c3d36a207ebf2f6d52464")
    );
    assert!(!PROVENANCE.contains("/Users/"));
}

#[test]
fn protocol_version_one_is_current() {
    assert_eq!(PROTOCOL_VERSION, 1);
    assert_eq!(ProtocolVersion::CURRENT, ProtocolVersion::new(1));
}

#[test]
fn diagnostic_trace_mode_defaults_to_full_when_omitted() {
    let mut request = request_value();
    request
        .as_object_mut()
        .expect("request object")
        .remove("diagnosticTraceMode");

    let decoded = decode_value(&request).expect("omitted diagnostic trace mode decodes");
    assert_eq!(decoded.diagnostic_trace_mode, DiagnosticTraceMode::Full);
}

#[test]
fn diagnostic_trace_mode_round_trips_supported_wire_values() {
    for mode in [
        ("full", DiagnosticTraceMode::Full),
        ("off", DiagnosticTraceMode::Off),
    ] {
        let mut request = request_value();
        request["diagnosticTraceMode"] = json!(mode.0);
        let decoded = decode_value(&request).expect("supported diagnostic trace mode decodes");
        assert_eq!(decoded.diagnostic_trace_mode, mode.1);
        let encoded: Value = serde_json::from_slice(
            &encode_request(&decoded).expect("supported diagnostic trace mode re-encodes"),
        )
        .expect("encoded request parses");
        assert_eq!(encoded["diagnosticTraceMode"], json!(mode.0));
    }
}

#[test]
fn diagnostic_trace_mode_rejects_unknown_wire_values() {
    let mut request = request_value();
    request["diagnosticTraceMode"] = json!("summary");
    assert!(matches!(
        decode_value(&request),
        Err(ProtocolError::MalformedInput { .. })
    ));
}

#[test]
fn decode_rejects_unsupported_versions_with_a_typed_error() {
    let mut request = request_value();
    request["version"] = json!(2);

    assert!(matches!(
        decode_value(&request),
        Err(ProtocolError::UnsupportedVersion {
            expected: ProtocolVersion::CURRENT,
            received: 2
        })
    ));
}

#[test]
fn decode_rejects_malformed_input_with_a_typed_error() {
    assert!(matches!(
        decode_request(b"{not-json"),
        Err(ProtocolError::MalformedInput { .. })
    ));
}

#[test]
fn extreme_magnitude_floats_parse_to_the_nearest_binary64_value() {
    let request = REQUEST_VECTOR.replace("9007199254740991", "5e+293");
    let decoded = decode_request(request).expect("extreme finite timeout decodes");
    assert_eq!(decoded.timeout_ms.to_bits(), 5e293_f64.to_bits());
}

#[test]
fn repeated_encoding_is_byte_deterministic() {
    let outcome = empty_success();
    assert_eq!(
        encode_outcome(&outcome).expect("first encoding succeeds"),
        encode_outcome(&outcome).expect("second encoding succeeds")
    );
}

#[test]
fn request_sheet_label_survives_codec_round_trip() {
    let mut request = request_value();
    request["sheet"]["label"] = json!("production sheet");

    let decoded = decode_value(&request).expect("request with sheet label decodes");
    let encoded: Value = serde_json::from_slice(
        &encode_request(&decoded).expect("request with sheet label re-encodes"),
    )
    .expect("encoded request parses");

    assert_eq!(encoded["sheet"]["label"], json!("production sheet"));
}

#[test]
fn request_sheet_label_is_required() {
    let mut request = request_value();
    request["sheet"].as_object_mut().unwrap().remove("label");

    assert!(
        decode_value(&request).is_err(),
        "missing sheet label was accepted"
    );
}

#[test]
fn unknown_fields_are_accepted_and_adapter_fields_are_not_reencoded() {
    let mut request = request_value();
    request["jobId"] = json!("desktop-job");
    request["strategyRunId"] = json!("desktop-strategy");
    request["workerMode"] = json!("irregular-convex-v2");
    request["electronRoutingId"] = json!("route-1");
    request["azureContainer"] = json!("container");
    request["settings"]["futureSetting"] = json!(true);

    let decoded = decode_value(&request).expect("unknown fields are ignored");
    let encoded: Value = serde_json::from_slice(
        &encode_request(&decoded).expect("neutral request encoding succeeds"),
    )
    .expect("encoded request parses");

    for forbidden in [
        "jobId",
        "strategyRunId",
        "workerMode",
        "electronRoutingId",
        "azureContainer",
        "futureSetting",
    ] {
        assert!(encoded.get(forbidden).is_none(), "found {forbidden}");
        assert!(
            encoded["settings"].get(forbidden).is_none(),
            "found {forbidden}"
        );
    }
}

#[test]
fn decimal_exact_integers_are_canonical_strings_on_the_wire() {
    for value in ["0", "1", "-1", "9007199254740991000000000000000000"] {
        let exact = ExactDecimalString::new(value).expect("canonical decimal is accepted");
        assert_eq!(
            serde_json::to_value(exact).expect("serializes"),
            json!(value)
        );
    }

    for invalid in ["", "+1", "01", "-0", "1.0", " 1", "1 "] {
        assert!(
            ExactDecimalString::new(invalid).is_err(),
            "accepted {invalid:?}"
        );
    }

    assert!(serde_json::from_value::<ExactDecimalString>(json!(42)).is_err());
}

#[test]
fn capacity_lane_coordinator_preserves_snake_case_producer_fields() {
    let value = json!({
        "version": "intrinsic-capacity-lane-coordinator-v3",
        "aggregatePlacementEvaluationCap": 10.0,
        "aggregateConsumedPlacementEvaluations": 2.0,
        "warmPilotDepthBoundaries": 1.0,
        "continuedProducers": [{
            "role": "capacity-warm-prefix",
            "source_role": "warm-prefix",
            "prefix_depth": 2.0
        }],
        "retainedCheckpointCount": 1.0,
        "censoredLaneCount": 0.0,
        "quanta": []
    });
    assert_json_round_trip::<CapacityLaneCoordinatorTrace>(value.clone());

    let mut camel_case = value;
    let producer = &mut camel_case["continuedProducers"][0];
    producer["sourceRole"] = producer["source_role"].take();
    producer["prefixDepth"] = producer["prefix_depth"].take();
    assert!(serde_json::from_value::<CapacityLaneCoordinatorTrace>(camel_case).is_err());
}

#[test]
fn semantic_result_values_round_trip_through_typed_dtos() {
    assert_json_round_trip::<FreeMaterialSnapshot>(json!({
        "sheet": { "width": 100.0, "height": 80.0, "label": "sheet" },
        "regions": [{
            "boundary": { "points": [
                { "x": 0.0, "y": 0.0 },
                { "x": 100.0, "y": 0.0 },
                { "x": 100.0, "y": 80.0 }
            ] },
            "holes": []
        }],
        "diagnostics": []
    }));

    assert_json_round_trip::<CapacityTrace>(json!({
        "routing": "bounded-complete-archive-miss",
        "preflight": {
            "kind": "inconclusive",
            "measurements": {
                "pieceCount": 1.0,
                "sheetWidthGrid": 100.0,
                "sheetHeightGrid": 80.0,
                "sheetDoubledAreaGrid2": "16000",
                "minimumDoubledCollisionAreaSumGrid2": "200",
                "minimumCollisionAreaPressurePpm": "12500",
                "maximumSingletonSpanPressurePpm": "100000",
                "singletonInfeasiblePieceIds": []
            }
        },
        "prefixes": {
            "capturedCount": 0.0,
            "fittingCount": 0.0,
            "rejectedCount": 0.0,
            "terminalizedCount": 0.0,
            "descriptors": []
        },
        "coldSearch": {
            "beamWidth": 4.0,
            "localLegalPlacementFanout": 2.0,
            "placementEvaluationCap": 100.0,
            "placementEvaluationQuotaPerDepth": 25.0,
            "consumedPlacementEvaluations": 10.0,
            "auxiliaryPlacementEvaluations": 0.0,
            "prunedByAttainableCount": 0.0,
            "prunedByAttainableMaterial": 0.0,
            "deduplicatedSuccessors": 0.0,
            "fitRejectedCandidates": 0.0,
            "invalidCandidates": 0.0,
            "endpointFitRejections": 0.0,
            "completedDepths": 1.0,
            "depthQuotaExhaustions": 0.0,
            "pieceCount": 1.0,
            "settlement": "exhausted"
        },
        "warmPrefixEndpointsAdmitted": false,
        "selected": {
            "placedCount": 1.0,
            "placedDoubledMaterialAreaGrid2": "200",
            "enclosedCavityCount": 0.0,
            "totalEnclosedCavityAreaMm2": 0.0,
            "totalEnclosedCavityDoubledAreaGrid2": "0",
            "envelopeMaximumSideMm": 10.0,
            "envelopeAreaMm2": 100.0,
            "envelopeSpanMm": 20.0,
            "envelopeMaximumSideGrid": 10.0,
            "envelopeAreaGrid2": "100",
            "envelopeSpanGrid": 20.0,
            "canonicalGeometryHash": "hash",
            "origin": "cold-search",
            "unplacedCount": 0.0,
            "placedMaterialAreaMm2": 100.0,
            "selectedRotationDeg": 0.0
        },
        "prefixTerminalizationMs": 0.0,
        "coldSearchMs": 1.0,
        "runtimeMs": 1.0
    }));

    assert_json_round_trip::<IntrinsicAnytimeSchedulerTrace>(json!({
        "version": "intrinsic-anytime-scheduler-v1",
        "coldQuantumDepths": 1.0,
        "coldStartStatus": "settled",
        "coldStartCompletedDepths": 1.0,
        "coldStartConsumedPlacementEvaluations": 10.0,
        "coldCheckpointReused": false,
        "warmPrefixEndpointsAdmitted": false,
        "quanta": [{
            "ordinal": 0,
            "cohort": "complete",
            "producerRole": "legacy-complete",
            "outcome": "settled"
        }]
    }));

    assert_json_round_trip::<FocusedCompleteReconstructionTrace>(json!({
        "version": "intrinsic-focused-complete-reconstruction-v1",
        "status": "completed",
        "sourceCanonicalGeometryHash": "source-hash",
        "candidateCanonicalGeometryHash": "candidate-hash",
        "selectedCanonicalGeometryHash": "candidate-hash",
        "consumedCandidateEvaluations": 1.0,
        "candidateEvaluationAccountingComplete": true,
        "runtimeMs": 0.5,
        "outputInfluence": "selected"
    }));

    assert_json_round_trip::<IntrinsicShortSideObserverTrace>(json!({
        "version": "intrinsic-short-side-observer-v6",
        "status": "observed-no-legal-orientation",
        "outputInfluence": "none",
        "requestedSheetWidthMm": 100.0,
        "requestedSheetHeightMm": 80.0,
        "requestedLongAxisMm": 100.0,
        "requestedShortAxisMm": 80.0,
        "requestedLongAxis": "width",
        "settledEndpointCount": 0.0,
        "evaluatedOrientationCount": 0.0,
        "cavityHullGuardEligibleEndpointCount": 0.0,
        "geometricParetoEligibleEndpointCount": 0.0,
        "placementEvaluations": 0.0,
        "candidateEvaluations": 0.0,
        "runtimeMs": 0.25,
        "runtimeBudgetExceeded": false,
        "serializedTraceBytes": 256.0,
        "endpoints": [],
        "rankedCanonicalGeometryHashes": []
    }));

    assert_json_round_trip::<IntrinsicShortSidePairFoldTrace>(json!({
        "version": "intrinsic-short-side-terminal-observer-v6",
        "status": "no-pair",
        "outputInfluence": "none",
        "executionModel": "single-process-sequential",
        "requestedShortAxisMm": 80.0,
        "requestedLongAxisMm": 100.0,
        "productionShortAxisSpanMm": 70.0,
        "productionMaximumSideMm": 90.0,
        "productionEnvelopeAreaMm2": 6300.0,
        "productionShortAxisSpanGrid": 70.0,
        "productionMaximumSideGrid": 90.0,
        "productionEnvelopeAreaGrid2": "6300",
        "transformEvaluations": 0.0,
        "expectedPairCount": 0.0,
        "evaluatedPairCount": 0.0,
        "rowCount": 0.0,
        "placedCount": 0.0,
        "envelopeAreaCostVetoObserved": false,
        "envelopeAreaCostVetoes": [],
        "contactStripLanes": [],
        "runtimeMs": 0.25,
        "peakRssDeltaBytes": 0.0,
        "serializedTraceBytes": 256.0
    }));
}

#[test]
fn nonempty_short_side_observer_trace_accepts_hash_and_sentinel_tuple_strings() {
    let q0 = short_side_orientation_value(0.0, Some("canonical-hash"));
    let q90 = short_side_orientation_value(90.0, None);
    assert_json_round_trip::<IntrinsicShortSideObserverTrace>(json!({
        "version": "intrinsic-short-side-observer-v6",
        "status": "observed",
        "outputInfluence": "selected",
        "requestedSheetWidthMm": 100.0,
        "requestedSheetHeightMm": 80.0,
        "requestedLongAxisMm": 100.0,
        "requestedShortAxisMm": 80.0,
        "requestedLongAxis": "width",
        "settledEndpointCount": 1.0,
        "evaluatedOrientationCount": 2.0,
        "cavityHullGuardEligibleEndpointCount": 1.0,
        "geometricParetoEligibleEndpointCount": 1.0,
        "placementEvaluations": 0.0,
        "candidateEvaluations": 0.0,
        "runtimeMs": 0.25,
        "runtimeBudgetExceeded": false,
        "serializedTraceBytes": 512.0,
        "endpoints": [{
            "archiveIndex": 0.0,
            "role": "terminal",
            "canonicalGeometryHash": "canonical-hash",
            "q0": q0,
            "q90": q90,
            "selectedRotationDeg": 0.0,
            "selected": q0,
            "cavityHullGuardEligible": true,
            "geometricParetoEligible": true
        }],
        "rankedCanonicalGeometryHashes": ["canonical-hash"],
        "observerWinnerCanonicalGeometryHash": "canonical-hash",
        "observerWinnerRotationDeg": 0.0
    }));
}

#[test]
fn scheduler_quantum_ordinal_rejects_fractional_json_numbers() {
    let invalid = json!({
        "version": "intrinsic-anytime-scheduler-v1",
        "coldQuantumDepths": 1.0,
        "coldStartStatus": "settled",
        "coldStartCompletedDepths": 1.0,
        "coldStartConsumedPlacementEvaluations": 10.0,
        "coldCheckpointReused": false,
        "warmPrefixEndpointsAdmitted": false,
        "quanta": [{
            "ordinal": 0.5,
            "cohort": "complete",
            "producerRole": "legacy-complete",
            "outcome": "settled"
        }]
    });

    assert!(serde_json::from_value::<IntrinsicAnytimeSchedulerTrace>(invalid).is_err());
}

#[test]
fn scheduler_trace_round_trips_integer_quantum_ordinals_as_json_integers() {
    let value = json!({
        "version": "intrinsic-anytime-scheduler-v1",
        "coldQuantumDepths": 1.0,
        "coldStartStatus": "settled",
        "coldStartCompletedDepths": 1.0,
        "coldStartConsumedPlacementEvaluations": 10.0,
        "coldCheckpointReused": false,
        "warmPrefixEndpointsAdmitted": false,
        "quanta": [{
            "ordinal": 0,
            "cohort": "complete",
            "producerRole": "legacy-complete",
            "outcome": "settled"
        }]
    });

    let decoded: IntrinsicAnytimeSchedulerTrace =
        serde_json::from_value(value.clone()).expect("typed value decodes");
    assert_eq!(
        serde_json::to_value(decoded).expect("typed value re-encodes"),
        value
    );
}

#[test]
fn semantic_result_values_reject_unknown_or_malformed_values() {
    assert!(serde_json::from_value::<IrregularTransformReason>(json!("desktop-specific")).is_err());
    for malformed in [json!({}), json!([]), json!("opaque")] {
        assert!(serde_json::from_value::<FreeMaterialSnapshot>(malformed.clone()).is_err());
        assert!(serde_json::from_value::<CapacityTrace>(malformed.clone()).is_err());
        assert!(
            serde_json::from_value::<IntrinsicAnytimeSchedulerTrace>(malformed.clone()).is_err()
        );
        assert!(
            serde_json::from_value::<FocusedCompleteReconstructionTrace>(malformed.clone())
                .is_err()
        );
        assert!(
            serde_json::from_value::<IntrinsicShortSideObserverTrace>(malformed.clone()).is_err()
        );
        assert!(
            serde_json::from_value::<IntrinsicShortSidePairFoldTrace>(malformed.clone()).is_err()
        );
    }
}

#[test]
fn capacity_preflight_reason_controls_piece_id_presence() {
    let measurements = json!({
        "pieceCount": 1.0,
        "sheetWidthGrid": 100.0,
        "sheetHeightGrid": 80.0,
        "sheetDoubledAreaGrid2": "16000",
        "minimumDoubledCollisionAreaSumGrid2": "200",
        "minimumCollisionAreaPressurePpm": "12500",
        "maximumSingletonSpanPressurePpm": "100000",
        "singletonInfeasiblePieceIds": ["piece-1"]
    });

    assert!(
        serde_json::from_value::<polygon_nesting_protocol::result::CapacityPreflightOutcome>(
            json!({
                "kind": "proven_impossible",
                "reason": "singleton-transform-set-does-not-fit",
                "measurements": measurements
            })
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<polygon_nesting_protocol::result::CapacityPreflightOutcome>(
            json!({
                "kind": "proven_impossible",
                "reason": "minimum-collision-area-exceeds-sheet-area",
                "pieceId": "piece-1",
                "measurements": measurements
            })
        )
        .is_err()
    );
}

#[test]
fn every_explicit_exact_decimal_field_rejects_numbers_and_accepts_strings() {
    assert_eq!(
        EXACT_DECIMAL_FIELD_NAMES,
        [
            "maximumSingletonSpanPressurePpm",
            "minimumCollisionAreaPressurePpm",
            "minimumDoubledCollisionAreaSumGrid2",
            "placedDoubledMaterialAreaGrid2",
            "sheetDoubledAreaGrid2",
        ]
    );

    assert_root_exact_decimal_fields::<
        polygon_nesting_protocol::result::CapacityPreflightMeasurements,
    >(
        json!({
            "pieceCount": 1.0,
            "sheetWidthGrid": 100.0,
            "sheetHeightGrid": 80.0,
            "sheetDoubledAreaGrid2": "16000",
            "minimumDoubledCollisionAreaSumGrid2": "200",
            "minimumCollisionAreaPressurePpm": "12500",
            "maximumSingletonSpanPressurePpm": "100000",
            "singletonInfeasiblePieceIds": []
        }),
        &[
            "sheetDoubledAreaGrid2",
            "minimumDoubledCollisionAreaSumGrid2",
            "minimumCollisionAreaPressurePpm",
            "maximumSingletonSpanPressurePpm",
        ],
    );

    assert_root_exact_decimal_fields::<polygon_nesting_protocol::result::CapacityObjective>(
        json!({
            "placedCount": 1.0,
            "placedDoubledMaterialAreaGrid2": "200",
            "enclosedCavityCount": 0.0,
            "totalEnclosedCavityAreaMm2": 0.0,
            "totalEnclosedCavityDoubledAreaGrid2": "0",
            "envelopeMaximumSideMm": 10.0,
            "envelopeAreaMm2": 100.0,
            "envelopeSpanMm": 20.0,
            "envelopeMaximumSideGrid": 10.0,
            "envelopeAreaGrid2": "100",
            "envelopeSpanGrid": 20.0,
            "canonicalGeometryHash": "hash",
            "origin": "cold-search"
        }),
        &["placedDoubledMaterialAreaGrid2"],
    );
}

#[test]
fn optional_fields_are_omitted_instead_of_encoded_as_null() {
    let encoded: Value = serde_json::from_slice(
        &encode_outcome(&empty_success()).expect("outcome encoding succeeds"),
    )
    .expect("outcome parses");

    assert_eq!(encoded["version"], json!(1));
    assert_eq!(encoded["outcome"]["status"], json!("success"));
    assert!(encoded["outcome"].get("diagnostics").is_none());
    assert!(encoded["outcome"]["result"].get("capacityTrace").is_none());
    assert_ne!(
        encoded["outcome"]["result"]["portfolio"]["score"],
        json!({})
    );
    assert_eq!(
        encoded["outcome"]["result"]["portfolio"]["score"]["unplacedCount"],
        json!(0.0)
    );
    assert!(!encoded.to_string().contains(":null"));
}

#[test]
fn error_categories_are_application_neutral() {
    let categories = [
        (EngineErrorCode::MalformedInput, "malformed_input"),
        (
            EngineErrorCode::ProtocolVersionMismatch,
            "protocol_version_mismatch",
        ),
        (EngineErrorCode::ArchiveIneligible, "archive_ineligible"),
        (EngineErrorCode::InvalidGeometry, "invalid_geometry"),
        (EngineErrorCode::Cancelled, "cancelled"),
        (EngineErrorCode::DeadlineExceeded, "deadline_exceeded"),
        (EngineErrorCode::EngineFailure, "engine_failure"),
        (EngineErrorCode::InternalFailure, "internal_failure"),
        (EngineErrorCode::IoFailure, "io_failure"),
    ];

    for (category, expected) in categories {
        let outcome = EngineOutcome::Failure {
            error: EngineError::new(category, "test-operation", "test message"),
            diagnostics: ExecutionDiagnostics::default(),
        };
        let encoded = String::from_utf8(encode_outcome(&outcome).expect("outcome encodes"))
            .expect("JSON is UTF-8");
        assert!(
            encoded.contains(&format!(r#""category":"{expected}""#)),
            "missing {expected}: {encoded}"
        );
        for forbidden in ["worker_", "irregular_", "not_implemented", "unknown_error"] {
            assert!(!encoded.contains(forbidden), "found {forbidden}: {encoded}");
        }
    }
}

#[test]
fn archive_ineligible_requests_produce_a_typed_outcome() {
    let mut request = request_value();
    request["settings"]["optimizer"]["intrinsicSharedArchiveEnabled"] = json!(false);
    let decoded = decode_value(&request).expect("request remains structurally valid");

    assert_eq!(
        decoded.archive_ineligibility(),
        Some(ArchiveIneligibilityReason::ArchiveDisabled)
    );
    let outcome = decoded
        .archive_ineligible_outcome()
        .expect("archive-ineligible outcome is typed");
    assert!(matches!(
        outcome,
        EngineOutcome::ArchiveIneligible {
            reason: ArchiveIneligibilityReason::ArchiveDisabled,
            ..
        }
    ));
    assert_eq!(
        String::from_utf8(encode_outcome(&outcome).expect("outcome encodes"))
            .expect("JSON is UTF-8"),
        ARCHIVE_INELIGIBLE_OUTCOME_VECTOR.trim()
    );
}

#[test]
fn frozen_semantic_event_vector_round_trips() {
    let event: SequencedEngineEvent =
        serde_json::from_str(EVENT_VECTOR).expect("frozen event decodes");
    assert_eq!(
        String::from_utf8(encode_event(&event).expect("event re-encodes")).expect("JSON is UTF-8"),
        EVENT_VECTOR.trim()
    );
}

#[test]
fn frozen_state_snapshot_uses_the_result_side_prepared_piece_shape() {
    let event: SequencedEngineEvent =
        serde_json::from_str(STATE_SNAPSHOT_EVENT_VECTOR).expect("snapshot event decodes");
    let EngineEvent::StateSnapshot { snapshot, .. } = &event.event else {
        panic!("expected a state snapshot event");
    };
    let prepared: &SnapshotPreparedPiece = snapshot
        .remaining_prepared_pieces
        .first()
        .expect("snapshot has one remaining prepared piece");
    assert_eq!(prepared.source.id, "source-1");
    assert_eq!(prepared.collision_geometry.sampled_points.len(), 4);
    assert_eq!(prepared.transforms.len(), 1);

    let encoded = String::from_utf8(encode_event(&event).expect("snapshot event re-encodes"))
        .expect("JSON is UTF-8");
    assert_eq!(encoded, STATE_SNAPSHOT_EVENT_VECTOR.trim());
    for forbidden in ["jobId", "strategyRunId", "workerMode", "electronRoutingId"] {
        assert!(!encoded.contains(forbidden), "found {forbidden}");
    }
}

#[test]
fn semantic_events_serialize_with_an_outer_ordinal_and_no_terminal_variant() {
    let event = SequencedEngineEvent {
        ordinal: 7,
        event: EngineEvent::PortfolioProgress {
            progress: PortfolioProgress {
                phase: PortfolioPhase::SharedArchive,
                best_score: None,
                elapsed_ms: 12.5,
            },
        },
    };

    assert_eq!(
        String::from_utf8(encode_event(&event).expect("event encoding succeeds"))
            .expect("JSON is UTF-8"),
        r#"{"ordinal":7,"event":{"kind":"portfolio-progress","progress":{"phase":"shared_archive","elapsedMs":12.5}}}"#
    );
}

#[test]
fn invalid_timeout_is_rejected() {
    for timeout in [0.0, -1.0] {
        let mut request = request_value();
        request["timeoutMs"] = json!(timeout);
        assert!(matches!(
            decode_value(&request),
            Err(ProtocolError::Validation { field, .. }) if field == "timeoutMs"
        ));
    }
}

#[test]
fn invalid_sheet_dimensions_are_rejected() {
    for width in [0.0, 1.5, MAX_SAFE_INTEGER + 1.0] {
        let mut request = request_value();
        request["sheet"]["width"] = json!(width);
        assert!(matches!(
            decode_value(&request),
            Err(ProtocolError::Validation { field, .. }) if field == "sheet.width"
        ));
    }
}

#[test]
fn empty_duplicate_and_invalid_prepared_pieces_are_rejected() {
    let mut empty = request_value();
    empty["pieces"] = json!([]);
    assert!(matches!(
        decode_value(&empty),
        Err(ProtocolError::Validation { field, .. }) if field == "pieces"
    ));

    let mut duplicate = request_value();
    duplicate["pieces"] = json!([
        duplicate["pieces"][0].clone(),
        duplicate["pieces"][0].clone()
    ]);
    assert!(matches!(
        decode_value(&duplicate),
        Err(ProtocolError::Validation { field, .. }) if field == "pieces[1].id"
    ));

    let mut invalid = request_value();
    invalid["pieces"][0]["paddedBounds"]["area"] = json!(1.5);
    assert!(matches!(
        decode_value(&invalid),
        Err(ProtocolError::Validation { field, .. })
            if field == "pieces[0].paddedBounds.area"
    ));
}

#[test]
fn compact_short_side_archive_ineligibility_is_a_typed_outcome() {
    let mut request = request_value();
    request["profile"] = json!("compact-short-side");
    request["settings"]["optimizer"]["intrinsicSharedArchiveEnabled"] = json!(false);

    let decoded = decode_value(&request).expect("archive ineligibility is not malformed input");
    assert!(matches!(
        decoded.archive_ineligible_outcome(),
        Some(EngineOutcome::ArchiveIneligible {
            reason: ArchiveIneligibilityReason::ArchiveDisabled,
            ..
        })
    ));
}

#[test]
fn source_piece_geometry_is_typed_and_source_bounds_are_validated() {
    let mut malformed_geometry = request_value();
    malformed_geometry["sourcePieces"] = json!([source_piece_value()]);
    malformed_geometry["sourcePieces"][0]["geometry"] = json!({});
    assert!(matches!(
        decode_value(&malformed_geometry),
        Err(ProtocolError::MalformedInput { .. })
    ));

    let mut empty_geometry = request_value();
    empty_geometry["sourcePieces"] = json!([source_piece_value()]);
    empty_geometry["sourcePieces"][0]["geometry"]["segments"] = json!([]);
    decode_value(&empty_geometry).expect("outline viability belongs to core validation");

    for (field, values) in [
        ("x", [-1.0, 1.5, MAX_SAFE_INTEGER + 1.0]),
        ("width", [-1.0, 1.5, MAX_SAFE_INTEGER + 1.0]),
    ] {
        for value in values {
            let mut invalid_bounds = request_value();
            invalid_bounds["sourcePieces"] = json!([source_piece_value()]);
            invalid_bounds["sourcePieces"][0]["realBounds"][field] = json!(value);
            assert!(matches!(
                decode_value(&invalid_bounds),
                Err(ProtocolError::Validation { field: error_field, .. })
                    if error_field == format!("sourcePieces[0].realBounds.{field}")
            ));
        }
    }
}

#[test]
fn source_piece_warnings_are_required() {
    let mut request = request_value();
    let mut source_piece = source_piece_value();
    source_piece
        .as_object_mut()
        .expect("source piece")
        .remove("warnings");
    request["sourcePieces"] = json!([source_piece]);

    assert!(matches!(
        decode_value(&request),
        Err(ProtocolError::MalformedInput { .. })
    ));
}

#[test]
fn source_piece_warnings_preserve_the_accepted_wire_fields() {
    let mut request = request_value();
    let mut source_piece = source_piece_value();
    source_piece["warnings"] = json!([{
        "code": "unsupported-entity",
        "message": "entity was skipped",
        "entityType": "TEXT",
        "entityHandle": "A1"
    }]);
    request["sourcePieces"] = json!([source_piece]);

    let decoded = decode_value(&request).expect("source warning decodes");
    let encoded: Value =
        serde_json::from_slice(&encode_request(&decoded).expect("source warning re-encodes"))
            .expect("encoded request parses");
    assert_eq!(
        encoded["sourcePieces"][0]["warnings"],
        json!([{
            "code": "unsupported-entity",
            "message": "entity was skipped",
            "entityType": "TEXT",
            "entityHandle": "A1"
        }])
    );
}

#[test]
fn required_result_fields_are_not_synthesized_during_decode() {
    assert!(serde_json::from_str::<EngineResult>("{}").is_err());
    assert!(serde_json::from_str::<LayoutScoreSummary>("{}").is_err());
    assert!(serde_json::from_str::<StateSnapshot>(
        r#"{"stepIndex":0,"beamRank":0,"candidateCount":0}"#
    )
    .is_err());
    assert!(serde_json::from_str::<PortfolioResult>(
        r#"{"status":"completed","terminationReason":"shared_archive_completed","source":"shared-archive"}"#
    )
    .is_err());
}

#[test]
fn safe_integer_boundaries_are_enforced_for_optimizer_settings() {
    let mut request = request_value();
    request["settings"]["optimizer"]["beamWidth"] = json!(MAX_SAFE_INTEGER + 1.0);
    assert!(matches!(
        decode_value(&request),
        Err(ProtocolError::Validation { field, .. })
            if field == "settings.optimizer.beamWidth"
    ));
}

#[test]
fn required_presence_sensitive_settings_are_not_synthesized() {
    let mut request = request_value();
    request.as_object_mut().expect("object").remove("settings");
    assert!(matches!(
        decode_value(&request),
        Err(ProtocolError::MalformedInput { .. })
    ));
}

#[test]
fn only_supported_profiles_are_accepted() {
    let mut request = request_value();
    request["profile"] = json!("legacy");
    assert!(matches!(
        decode_value(&request),
        Err(ProtocolError::MalformedInput { .. })
    ));

    request["profile"] = json!("compact-short-side");
    request["settings"]["optimizer"]["intrinsicSharedArchiveEnabled"] = json!(true);
    request["settings"]["optimizer"]["gaEnabled"] = json!(false);
    request["settings"]["optimizer"]["placementPolicyId"] =
        json!("edge-contact-then-balanced-compactness");
    let decoded = decode_value(&request).expect("Compact Short Side is supported");
    assert_eq!(decoded.profile, EngineProfile::CompactShortSide);
}
