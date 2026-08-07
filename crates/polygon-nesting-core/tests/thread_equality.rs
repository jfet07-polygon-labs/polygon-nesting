use std::fs;

use polygon_nesting_core::{CancellationControl, EngineEventSink, Job};
use polygon_nesting_protocol::{
    EngineOutcome, EngineProfile, EngineRequest, EngineSettings, GeometrySettings, HistoryMode,
    OptimizerSettings, PlacementPolicy, PreparedPiece, ProtocolVersion, Rect, RectWithMetrics,
    SourceGeometry, SourceGeometryEntityType, SourceGeometrySegment, SourceLineSegment,
    SourcePiece,
};
use serde::Deserialize;
use serde_json::Value;

const THREAD_COUNTS: [usize; 4] = [1, 2, 4, 8];
const REPEATS_PER_THREAD_COUNT: usize = 3;
const TIMING_ONLY_FIELD_NAMES: &[&str] = &[
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
];
const TIMING_PRESENT_MARKER: &str = "<timing: present>";

#[test]
fn canonical_semantic_bytes_sort_object_fields_and_preserve_timing_field_presence() {
    let baseline = serde_json::json!({
        "z": [true, 3.5],
        "runtimeMs": 1,
        "a": { "inner": "value" }
    });
    let reordered_with_different_timing = serde_json::json!({
        "a": { "inner": "value" },
        "runtimeMs": 2,
        "z": [true, 3.5]
    });
    let timing_absent = serde_json::json!({
        "a": { "inner": "value" },
        "z": [true, 3.5]
    });

    assert_eq!(
        canonical_semantic_bytes(&baseline),
        canonical_semantic_bytes(&reordered_with_different_timing)
    );
    assert_ne!(
        canonical_semantic_bytes(&baseline),
        canonical_semantic_bytes(&timing_absent)
    );
}

#[test]
fn canonical_semantic_bytes_preserve_semantic_values_and_normalize_only_measurements() {
    let first = serde_json::json!({
        "result": {
            "beta": 2,
            "alpha": ["first", "second"],
            "label": "stable",
            "enabled": true,
            "optional": null
        },
        "runtimeMs": 17,
        "elapsedMs": 18,
        "serializedTraceBytes": 19,
        "peakRssDeltaBytes": 20
    });
    let reordered_with_different_measurements = serde_json::json!({
        "peakRssDeltaBytes": 200,
        "result": {
            "optional": null,
            "enabled": true,
            "label": "stable",
            "alpha": ["first", "second"],
            "beta": 2
        },
        "serializedTraceBytes": 190,
        "elapsedMs": 180,
        "runtimeMs": 170
    });
    let semantic_changes = [
        serde_json::json!({
            "result": {"alpha": ["second", "first"], "beta": 2, "label": "stable", "enabled": true, "optional": null},
            "runtimeMs": 170, "elapsedMs": 180, "serializedTraceBytes": 190, "peakRssDeltaBytes": 200
        }),
        serde_json::json!({
            "result": {"alpha": ["first", "second"], "beta": 3, "label": "stable", "enabled": true, "optional": null},
            "runtimeMs": 170, "elapsedMs": 180, "serializedTraceBytes": 190, "peakRssDeltaBytes": 200
        }),
        serde_json::json!({
            "result": {"alpha": ["first", "second"], "beta": 2, "label": "changed", "enabled": true, "optional": null},
            "runtimeMs": 170, "elapsedMs": 180, "serializedTraceBytes": 190, "peakRssDeltaBytes": 200
        }),
        serde_json::json!({
            "result": {"alpha": ["first", "second"], "beta": 2, "label": "stable", "enabled": false, "optional": null},
            "runtimeMs": 170, "elapsedMs": 180, "serializedTraceBytes": 190, "peakRssDeltaBytes": 200
        }),
        serde_json::json!({
            "result": {"alpha": ["first", "second"], "beta": 2, "label": "stable", "enabled": true, "optional": "present"},
            "runtimeMs": 170, "elapsedMs": 180, "serializedTraceBytes": 190, "peakRssDeltaBytes": 200
        }),
    ];

    let first_bytes = canonical_semantic_bytes(&first);
    assert_eq!(
        first_bytes,
        canonical_semantic_bytes(&reordered_with_different_measurements)
    );
    for changed in semantic_changes {
        assert_ne!(first_bytes, canonical_semantic_bytes(&changed));
    }
}

struct NullSink;

impl EngineEventSink for NullSink {
    fn emit(&mut self, _: polygon_nesting_protocol::SequencedEngineEvent) {}
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrozenRequest {
    version: u32,
    sheet: FrozenSheet,
    padding: f64,
    pieces: Vec<FrozenPreparedPiece>,
    source_pieces: Vec<FrozenSourcePiece>,
    options: FrozenOptions,
}

#[derive(Deserialize)]
struct FrozenSheet {
    width: f64,
    height: f64,
    label: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrozenPreparedPiece {
    id: String,
    source_piece_id: String,
    interchangeability_key: Option<String>,
    real_bounds: FrozenRect,
    padded_bounds: FrozenRectWithMetrics,
    padding: f64,
    allow_rotation: bool,
    allow_mirror: bool,
}

#[derive(Deserialize)]
struct FrozenRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrozenRectWithMetrics {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    longest_edge: f64,
    area: f64,
    imbalance: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrozenSourcePiece {
    id: String,
    source_file_id: String,
    source_layer: Option<String>,
    label: String,
    real_bounds: FrozenRect,
    geometry: FrozenSourceGeometry,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrozenSourceGeometry {
    entity_type: String,
    closed: bool,
    segments: Vec<FrozenSourceSegment>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum FrozenSourceSegment {
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        bulge: Option<f64>,
    },
    Arc {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        cx: f64,
        cy: f64,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrozenOptions {
    allow_global_rotation: bool,
    allow_global_mirror: bool,
    timeout_ms: f64,
    history_mode: String,
    irregular_settings: FrozenIrregularSettings,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrozenIrregularSettings {
    geometry: FrozenGeometrySettings,
    optimizer: FrozenOptimizerSettings,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrozenGeometrySettings {
    flattening_sag_tolerance_mm: f64,
    clearance_safety_margin_mm: f64,
    geometry_backend_id: String,
    geometry_backend_version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrozenOptimizerSettings {
    order_window: f64,
    beam_width: f64,
    local_candidate_fanout: f64,
    local_repair_budget: f64,
    intrinsic_shared_archive_enabled: bool,
    transform_cap: f64,
    transform_minimum_edge_length_mm: f64,
    transform_angle_deduplication_tolerance_deg: f64,
    configured_rotation_enabled: bool,
    edge_alignment_enabled: bool,
    configured_rotation_deg: Vec<f64>,
    ga_enabled: bool,
    baseline_only: bool,
    ga_population: f64,
    ga_generation_budget: f64,
    ga_evaluation_budget: f64,
    ga_time_budget_ms: f64,
    ga_seed: String,
    priority_order_mutation_enabled: bool,
    transform_preference_mutation_enabled: bool,
    placement_policy_mutation_enabled: bool,
    placement_policy_id: String,
    placement_policy_ids: Vec<String>,
}

fn fixture_path(piece_count: usize) -> String {
    format!(
        "{}/../../tests/vectors/core/thread-equality-mixed61-{piece_count}-piece-request.json",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn decode_request(piece_count: usize) -> EngineRequest {
    let path = fixture_path(piece_count);
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read fixture {path}: {error}"));
    let frozen: FrozenRequest = serde_json::from_str(&raw)
        .unwrap_or_else(|error| panic!("failed to decode fixture {path}: {error}"));
    engine_request_from_frozen(frozen)
}

fn engine_request_from_frozen(frozen: FrozenRequest) -> EngineRequest {
    EngineRequest {
        version: ProtocolVersion::new(frozen.version),
        timeout_ms: frozen.options.timeout_ms,
        profile: EngineProfile::Compact,
        sheet: polygon_nesting_protocol::SheetSpec {
            width: frozen.sheet.width,
            height: frozen.sheet.height,
            label: frozen.sheet.label,
        },
        pieces: frozen
            .pieces
            .into_iter()
            .map(prepared_piece_from_frozen)
            .collect(),
        source_pieces: frozen
            .source_pieces
            .into_iter()
            .map(source_piece_from_frozen)
            .collect(),
        settings: EngineSettings {
            padding: frozen.padding,
            allow_global_rotation: frozen.options.allow_global_rotation,
            allow_global_mirror: frozen.options.allow_global_mirror,
            geometry: geometry_settings_from_frozen(frozen.options.irregular_settings.geometry),
            optimizer: optimizer_settings_from_frozen(frozen.options.irregular_settings.optimizer),
        },
        history_mode: history_mode_from_frozen(&frozen.options.history_mode),
        diagnostic_trace_mode: polygon_nesting_protocol::DiagnosticTraceMode::Full,
    }
}

fn prepared_piece_from_frozen(piece: FrozenPreparedPiece) -> PreparedPiece {
    PreparedPiece {
        id: piece.id,
        source_piece_id: piece.source_piece_id,
        interchangeability_key: piece.interchangeability_key,
        real_bounds: rect_from_frozen(piece.real_bounds),
        padded_bounds: RectWithMetrics {
            x: piece.padded_bounds.x,
            y: piece.padded_bounds.y,
            width: piece.padded_bounds.width,
            height: piece.padded_bounds.height,
            longest_edge: piece.padded_bounds.longest_edge,
            area: piece.padded_bounds.area,
            imbalance: piece.padded_bounds.imbalance,
        },
        padding: piece.padding,
        allow_rotation: piece.allow_rotation,
        allow_mirror: piece.allow_mirror,
        cut_row_ref: None,
    }
}

fn source_piece_from_frozen(piece: FrozenSourcePiece) -> SourcePiece {
    SourcePiece {
        id: piece.id,
        source_file_id: piece.source_file_id,
        source_layer: piece.source_layer,
        label: piece.label,
        real_bounds: rect_from_frozen(piece.real_bounds),
        geometry: SourceGeometry {
            entity_type: source_geometry_entity_type_from_frozen(&piece.geometry.entity_type),
            closed: piece.geometry.closed,
            segments: piece
                .geometry
                .segments
                .into_iter()
                .map(source_segment_from_frozen)
                .collect(),
        },
        warnings: Vec::new(),
    }
}

fn rect_from_frozen(rect: FrozenRect) -> Rect {
    Rect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

fn source_geometry_entity_type_from_frozen(value: &str) -> SourceGeometryEntityType {
    match value {
        "LINE" => SourceGeometryEntityType::Line,
        "LWPOLYLINE" => SourceGeometryEntityType::Lwpolyline,
        "POLYLINE" => SourceGeometryEntityType::Polyline,
        "CIRCLE" => SourceGeometryEntityType::Circle,
        "ARC" => SourceGeometryEntityType::Arc,
        "ELLIPSE" => SourceGeometryEntityType::Ellipse,
        "DXF_SHAPE" => SourceGeometryEntityType::DxfShape,
        "PRESET_SHAPE" => SourceGeometryEntityType::PresetShape,
        other => panic!("unsupported source entity type: {other}"),
    }
}

fn source_segment_from_frozen(segment: FrozenSourceSegment) -> SourceGeometrySegment {
    match segment {
        FrozenSourceSegment::Line {
            x1,
            y1,
            x2,
            y2,
            bulge,
        } => SourceGeometrySegment::Line(SourceLineSegment {
            x1,
            y1,
            x2,
            y2,
            bulge,
            source_curve: None,
        }),
        FrozenSourceSegment::Arc {
            x1,
            y1,
            x2,
            y2,
            cx,
            cy,
            radius,
            start_angle,
            end_angle,
        } => SourceGeometrySegment::Arc(polygon_nesting_protocol::SourceArcSegment {
            x1,
            y1,
            x2,
            y2,
            cx,
            cy,
            radius,
            start_angle,
            end_angle,
        }),
    }
}

fn geometry_settings_from_frozen(settings: FrozenGeometrySettings) -> GeometrySettings {
    GeometrySettings {
        flattening_sag_tolerance_mm: settings.flattening_sag_tolerance_mm,
        clearance_safety_margin_mm: settings.clearance_safety_margin_mm,
        geometry_backend_id: settings.geometry_backend_id,
        geometry_backend_version: settings.geometry_backend_version,
    }
}

fn optimizer_settings_from_frozen(settings: FrozenOptimizerSettings) -> OptimizerSettings {
    OptimizerSettings {
        order_window: settings.order_window,
        beam_width: settings.beam_width,
        local_candidate_fanout: settings.local_candidate_fanout,
        local_repair_budget: settings.local_repair_budget,
        intrinsic_shared_archive_enabled: settings.intrinsic_shared_archive_enabled,
        transform_cap: settings.transform_cap,
        transform_minimum_edge_length_mm: settings.transform_minimum_edge_length_mm,
        transform_angle_deduplication_tolerance_deg: settings
            .transform_angle_deduplication_tolerance_deg,
        configured_rotation_enabled: settings.configured_rotation_enabled,
        edge_alignment_enabled: settings.edge_alignment_enabled,
        configured_rotation_deg: settings.configured_rotation_deg,
        ga_enabled: settings.ga_enabled,
        baseline_only: settings.baseline_only,
        ga_population: settings.ga_population,
        ga_generation_budget: settings.ga_generation_budget,
        ga_evaluation_budget: settings.ga_evaluation_budget,
        ga_time_budget_ms: settings.ga_time_budget_ms,
        ga_seed: settings.ga_seed,
        priority_order_mutation_enabled: settings.priority_order_mutation_enabled,
        transform_preference_mutation_enabled: settings.transform_preference_mutation_enabled,
        placement_policy_mutation_enabled: settings.placement_policy_mutation_enabled,
        placement_policy_id: placement_policy_from_frozen(&settings.placement_policy_id),
        placement_policy_ids: settings
            .placement_policy_ids
            .iter()
            .map(|value| placement_policy_from_frozen(value))
            .collect(),
    }
}

fn placement_policy_from_frozen(value: &str) -> PlacementPolicy {
    match value {
        "balanced-compactness" => PlacementPolicy::BalancedCompactness,
        "short-side-fill" => PlacementPolicy::ShortSideFill,
        "edge-contact-then-balanced-compactness" => {
            PlacementPolicy::EdgeContactThenBalancedCompactness
        }
        other => panic!("unsupported placement policy: {other}"),
    }
}

fn history_mode_from_frozen(value: &str) -> HistoryMode {
    match value {
        "stream" => HistoryMode::Stream,
        "final" => HistoryMode::Final,
        "off" => HistoryMode::Off,
        other => panic!("unsupported history mode: {other}"),
    }
}

fn normalize_timing_only_fields(value: &Value) -> Value {
    match value {
        Value::Array(items) => {
            Value::Array(items.iter().map(normalize_timing_only_fields).collect())
        }
        Value::Object(fields) => {
            let mut normalized = serde_json::Map::with_capacity(fields.len());
            for (key, field_value) in fields {
                if TIMING_ONLY_FIELD_NAMES.contains(&key.as_str()) {
                    normalized.insert(key.clone(), Value::String(TIMING_PRESENT_MARKER.to_owned()));
                } else {
                    normalized.insert(key.clone(), normalize_timing_only_fields(field_value));
                }
            }
            Value::Object(normalized)
        }
        other => other.clone(),
    }
}

fn append_length(bytes: &mut Vec<u8>, length: usize) {
    bytes.extend_from_slice(length.to_string().as_bytes());
    bytes.push(b':');
}

fn append_canonical_string(bytes: &mut Vec<u8>, value: &str) {
    append_length(bytes, value.len());
    bytes.extend_from_slice(value.as_bytes());
}

fn append_canonical_json(bytes: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Null => bytes.extend_from_slice(b"n"),
        Value::Bool(value) => bytes.extend_from_slice(if *value { b"b1" } else { b"b0" }),
        Value::Number(value) => {
            bytes.push(b'd');
            append_canonical_string(bytes, &value.to_string());
        }
        Value::String(value) => {
            bytes.push(b's');
            append_canonical_string(bytes, value);
        }
        Value::Array(values) => {
            bytes.push(b'a');
            append_length(bytes, values.len());
            for value in values {
                append_canonical_json(bytes, value);
            }
        }
        Value::Object(values) => {
            bytes.push(b'o');
            append_length(bytes, values.len());
            let mut entries: Vec<_> = values.iter().collect();
            entries.sort_by_key(|(key, _)| *key);
            for (key, value) in entries {
                append_canonical_string(bytes, key);
                append_canonical_json(bytes, value);
            }
        }
    }
}

fn canonical_semantic_bytes(value: &Value) -> Vec<u8> {
    let normalized = normalize_timing_only_fields(value);
    let mut bytes = Vec::new();
    append_canonical_json(&mut bytes, &normalized);
    bytes
}

fn canonical_engine_result_bytes(result: &polygon_nesting_protocol::EngineResult) -> Vec<u8> {
    let value = serde_json::to_value(result).expect("engine result must serialize");
    canonical_semantic_bytes(&value)
}

fn run_once(
    request: &EngineRequest,
    worker_count: usize,
) -> polygon_nesting_protocol::EngineResult {
    let control = CancellationControl::new();
    let mut sink = NullSink;
    let outcome = Job::with_thread_count(request, &control, &mut sink, Some(worker_count))
        .run()
        .expect("job execution must not fail");
    let EngineOutcome::Success {
        result,
        diagnostics,
    } = outcome
    else {
        panic!("fixture must resolve to success: {outcome:?}");
    };
    assert!(!result.placed_collision_geometries.is_empty());
    assert_eq!(diagnostics.requested_workers, Some(worker_count as u32));
    assert_eq!(diagnostics.actual_workers, Some(worker_count as u32));
    result
}

fn thread_equality_case(piece_count: usize) {
    let request = decode_request(piece_count);
    let mut run_counts = [0_usize; THREAD_COUNTS.len()];
    let mut baseline_bytes = None;

    for (worker_index, &worker_count) in THREAD_COUNTS.iter().enumerate() {
        for repeat in 0..REPEATS_PER_THREAD_COUNT {
            let result = run_once(&request, worker_count);
            run_counts[worker_index] += 1;
            let result_bytes = canonical_engine_result_bytes(&result);
            if worker_count == 1 && repeat == 0 {
                baseline_bytes = Some(result_bytes);
            } else {
                assert_eq!(
                    result_bytes,
                    *baseline_bytes
                        .as_ref()
                        .expect("worker=1 repeat=0 must establish the canonical baseline"),
                    "pieces={piece_count} workers={worker_count} repeat={repeat} diverged from the one-worker baseline"
                );
            }
        }
    }

    assert_eq!(run_counts, [REPEATS_PER_THREAD_COUNT; THREAD_COUNTS.len()]);
    assert_eq!(
        run_counts.iter().sum::<usize>(),
        THREAD_COUNTS.len() * REPEATS_PER_THREAD_COUNT
    );
}

#[test]
fn two_piece_fixture_is_thread_count_invariant() {
    thread_equality_case(2);
}

#[test]
fn four_piece_fixture_is_thread_count_invariant() {
    thread_equality_case(4);
}

#[test]
fn eight_piece_fixture_is_thread_count_invariant() {
    thread_equality_case(8);
}

#[test]
fn twenty_piece_fixture_is_thread_count_invariant() {
    thread_equality_case(20);
}
