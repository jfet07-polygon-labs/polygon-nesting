use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use jsonschema::{Retrieve, Uri, Validator};
use serde_json::{json, Map, Value};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn schema_root() -> PathBuf {
    repository_root().join("packages/polygon-nesting/schemas")
}

#[derive(Clone)]
struct SchemaRetriever {
    schemas: HashMap<String, Value>,
}

impl Retrieve for SchemaRetriever {
    fn retrieve(
        &self,
        uri: &Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        self.schemas
            .get(uri.as_str())
            .cloned()
            .ok_or_else(|| format!("schema not found: {uri}").into())
    }
}

fn schema_documents() -> HashMap<String, Value> {
    let manifest: Value = serde_json::from_slice(
        &fs::read(schema_root().join("index.json")).expect("schema manifest should be readable"),
    )
    .expect("schema manifest should be JSON");
    manifest["schemas"]
        .as_object()
        .expect("schema manifest should contain schemas")
        .values()
        .map(|relative| {
            let relative = relative.as_str().expect("schema path should be a string");
            let schema: Value = serde_json::from_slice(
                &fs::read(schema_root().join(relative)).expect("schema should be readable"),
            )
            .expect("schema should be JSON");
            let id = schema["$id"]
                .as_str()
                .expect("published schema should have an ID")
                .to_owned();
            (id, schema)
        })
        .collect()
}

fn validator(relative_path: &str) -> Validator {
    let schemas = schema_documents();
    let schema: Value = serde_json::from_slice(
        &fs::read(schema_root().join(relative_path)).expect("root schema should be readable"),
    )
    .expect("root schema should be JSON");
    jsonschema::draft202012::options()
        .with_retriever(SchemaRetriever { schemas })
        .build(&schema)
        .expect("published schema should compile")
}

fn assert_valid(validator: &Validator, value: &Value) {
    if let Err(error) = validator.validate(value) {
        panic!("value should satisfy schema: {error}\n{value}");
    }
}

fn temporary_directory(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should follow epoch")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "polygon-nesting-schema-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    directory
}

fn run_canonical_cli_sample() -> (Value, Vec<Value>, Value) {
    let directory = temporary_directory("cli");
    let result = directory.join("result.json");
    let events = directory.join("events.ndjson");
    let report = directory.join("report.json");
    let status = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            repository_root()
                .join("tests/fixtures/cli/request-v1.json")
                .to_str()
                .expect("fixture path should be UTF-8"),
            "--result-file",
            result.to_str().expect("result path should be UTF-8"),
            "--events",
            events.to_str().expect("events path should be UTF-8"),
            "--report-file",
            report.to_str().expect("report path should be UTF-8"),
        ])
        .status()
        .expect("CLI sample should execute");
    assert!(status.success());
    let outcome = serde_json::from_slice(&fs::read(result).expect("result should exist"))
        .expect("result should be JSON");
    let events = fs::read_to_string(events)
        .expect("events should exist")
        .lines()
        .map(|line| serde_json::from_str(line).expect("event should be JSON"))
        .collect();
    let report = serde_json::from_slice(&fs::read(report).expect("report should exist"))
        .expect("report should be JSON");
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
    (outcome, events, report)
}

fn flatten_napi_event(event: &Value) -> Value {
    let mut frame = event["event"]
        .as_object()
        .expect("event payload should be an object")
        .clone();
    frame.insert("ordinal".to_owned(), event["ordinal"].clone());
    Value::Object(frame)
}

fn zero_cache_namespace() -> Value {
    let fields = [
        "lookups",
        "hits",
        "misses",
        "stores",
        "staleDetections",
        "staleRemovals",
        "duplicateComputations",
        "singleFlightWaits",
        "shardLockWaitNanos",
        "shardLockContendedAcquisitions",
        "frontCacheHits",
        "backingCacheHits",
        "cloningHits",
        "capBytes",
        "admissions",
        "replacements",
        "evictions",
        "evictedBytes",
        "oversizedRejections",
        "entries",
        "approxBytes",
        "peakBytes",
        "computationTimeNanos",
    ];
    Value::Object(
        fields
            .into_iter()
            .map(|field| (field.to_owned(), json!(0)))
            .collect(),
    )
}

fn zero_object(fields: &[&str]) -> Value {
    Value::Object(
        fields
            .iter()
            .map(|field| ((*field).to_owned(), json!(0)))
            .collect(),
    )
}

#[test]
fn published_schemas_accept_canonical_cli_and_napi_values() {
    let (outcome, events, report) = run_canonical_cli_sample();
    let request: Value = serde_json::from_slice(
        &fs::read(repository_root().join("tests/fixtures/cli/request-v1.json"))
            .expect("request fixture should exist"),
    )
    .expect("request fixture should be JSON");
    let polygon_input: Value = serde_json::from_slice(
        &fs::read(repository_root().join("tests/fixtures/cli/polygons-v1.json"))
            .expect("polygon fixture should exist"),
    )
    .expect("polygon fixture should be JSON");
    let desktop_request: Value = serde_json::from_slice(
        &fs::read(repository_root().join("tests/fixtures/mixed-61/300x300-compact/request.json"))
            .expect("desktop request fixture should exist"),
    )
    .expect("desktop request fixture should be JSON");

    assert_valid(&validator("cli/engine-request-v1.schema.json"), &request);
    assert_valid(
        &validator("cli/polygon-input-v1.schema.json"),
        &polygon_input,
    );
    assert_valid(&validator("cli/engine-outcome-v1.schema.json"), &outcome);
    assert_valid(&validator("cli/benchmark-report-v1.schema.json"), &report);
    for event in &events {
        assert_valid(&validator("cli/engine-event-v1.schema.json"), event);
        assert_valid(
            &validator("napi/job-event-v3.schema.json"),
            &flatten_napi_event(event),
        );
    }
    assert_valid(
        &validator("napi/job-event-v3.schema.json"),
        &json!({"kind": "terminal", "ordinal": events.len()}),
    );
    assert_valid(
        &validator("napi/desktop-request-v1.schema.json"),
        &desktop_request,
    );
    assert_valid(
        &validator("napi/job-result-v3.schema.json"),
        &json!({"ok": true, "result": outcome["outcome"]["result"]}),
    );
    assert_valid(
        &validator("napi/native-capability-v3.schema.json"),
        &json!({
            "apiVersion": 3,
            "crateVersion": "0.3.0",
            "targetTriple": "aarch64-apple-darwin",
            "profiles": ["compact", "compact-short-side"]
        }),
    );

    let geometry_cache = zero_object(&[
        "capBytes",
        "currentBytes",
        "peakBytes",
        "admissions",
        "replacements",
        "evictions",
        "evictedBytes",
        "oversizedRejections",
        "cacheInstances",
    ]);
    let mut geometry_cache = geometry_cache.as_object().expect("object").clone();
    geometry_cache.insert(
        "namespaces".to_owned(),
        Value::Object(Map::from_iter([(
            "sheet-ifp-v1".to_owned(),
            zero_cache_namespace(),
        )])),
    );
    let diagnostics = json!({
        "backendVersion": "0.3.0",
        "threadCountUsed": 1,
        "threadCountRequested": 1,
        "wallClockMs": 0,
        "cacheTelemetry": geometry_cache,
        "freeMaterialCacheTelemetry": zero_object(&[
            "capBytes", "currentBytes", "peakBytes", "entries", "admissions", "replacements",
            "evictions", "evictedBytes", "oversizedRejections", "hits", "misses"
        ]),
        "processLifecycle": {
            "terminalCleanupHooksFired": 0,
            "terminalLatchCloseRequestsByCleanup": 0
        }
    });
    assert_valid(
        &validator("napi/last-job-diagnostics-v1.schema.json"),
        &diagnostics,
    );
    assert_valid(
        &validator("napi/last-job-diagnostics-v1.schema.json"),
        &Value::Null,
    );
}

#[test]
fn published_schemas_reject_missing_required_nested_fields() {
    let (mut outcome, _, _) = run_canonical_cli_sample();
    let placed = outcome["outcome"]["result"]["placedCollisionGeometries"]
        .as_array_mut()
        .and_then(|placed| placed.first_mut())
        .and_then(Value::as_object_mut)
        .expect("sample should place a piece");
    placed.remove("collisionGeometry");
    assert!(!validator("cli/engine-outcome-v1.schema.json").is_valid(&outcome));

    let mut event: Value = serde_json::from_slice(
        &fs::read(repository_root().join("tests/vectors/protocol/state-snapshot-event-v1.json"))
            .expect("event fixture should exist"),
    )
    .expect("event fixture should be JSON");
    event["event"]["snapshot"]
        .as_object_mut()
        .expect("snapshot should be an object")
        .remove("placements");
    assert!(!validator("cli/engine-event-v1.schema.json").is_valid(&event));
    assert!(!validator("napi/job-event-v3.schema.json").is_valid(&flatten_napi_event(&event)));

    let malformed_diagnostics = json!({
        "backendVersion": "0.3.0",
        "threadCountUsed": 1,
        "threadCountRequested": 1,
        "wallClockMs": 0,
        "cacheTelemetry": {},
        "freeMaterialCacheTelemetry": {},
        "processLifecycle": {}
    });
    assert!(!validator("napi/last-job-diagnostics-v1.schema.json").is_valid(&malformed_diagnostics));
}
