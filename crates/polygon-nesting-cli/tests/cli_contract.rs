use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use polygon_nesting_cli::{run, ExitStatus, RunPaths};
use polygon_nesting_core::{CancelReason, CancellationControl};
use polygon_nesting_protocol::{
    EngineProfile, EngineRequest, EngineSettings, GeometrySettings, HistoryMode, OptimizerSettings,
    PlacementPolicy, PreparedPiece, ProtocolVersion, Rect, RectWithMetrics, SourceGeometry,
    SourceGeometryEntityType, SourceGeometrySegment, SourceLineSegment, SourcePiece,
};
use serde_json::Value;

fn temporary_directory(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("polygon-nesting-cli-{name}-{unique}"));
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    directory
}

#[test]
fn run_writes_a_versioned_success_outcome_and_ordered_events() {
    let directory = temporary_directory("success");
    let input = directory.join("request.json");
    let output = directory.join("result.json");
    let events = directory.join("events.ndjson");
    fs::write(
        &input,
        include_bytes!("../../../tests/fixtures/cli/request-v1.json"),
    )
    .expect("fixture input should be written");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
            "--events",
            events.to_str().expect("events path is UTF-8"),
        ])
        .output()
        .expect("CLI should start");

    assert!(
        process.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&process.stderr)
    );
    let outcome: Value = serde_json::from_slice(&fs::read(&output).expect("result should exist"))
        .expect("result should be valid JSON");
    assert_eq!(outcome["version"], 1);
    assert_eq!(outcome["outcome"]["status"], "success");

    let events = fs::read_to_string(&events).expect("events should exist");
    let parsed_events = events
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("event should be JSON"))
        .collect::<Vec<_>>();
    assert!(!parsed_events.is_empty());
    for (ordinal, event) in parsed_events.iter().enumerate() {
        assert_eq!(event["ordinal"], ordinal as u64);
    }

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[test]
fn run_without_events_does_not_create_an_event_artifact() {
    let directory = temporary_directory("final-only");
    let input = directory.join("request.json");
    let output = directory.join("result.json");
    let events = directory.join("events.ndjson");
    fs::write(
        &input,
        include_bytes!("../../../tests/fixtures/cli/request-v1.json"),
    )
    .expect("fixture input should be written");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
        ])
        .output()
        .expect("CLI should start");

    assert!(process.status.success());
    assert!(output.exists());
    assert!(!events.exists());

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[test]
fn run_writes_the_exact_archive_ineligible_outcome_and_domain_exit() {
    let directory = temporary_directory("archive-ineligible");
    let input = directory.join("request.json");
    let output = directory.join("result.json");
    let mut request = valid_request();
    request.settings.optimizer.intrinsic_shared_archive_enabled = false;
    fs::write(
        &input,
        serde_json::to_vec(&request).expect("request should encode"),
    )
    .expect("input should be written");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(3));
    assert_eq!(
        fs::read(&output).expect("outcome should exist"),
        include_bytes!("../../../tests/vectors/protocol/archive-ineligible-outcome-v1.json")
            .strip_suffix(b"\n")
            .expect("vector has a trailing newline")
    );
    assert_eq!(
        String::from_utf8(process.stderr).expect("stderr should be UTF-8"),
        "polygon-nesting: typed domain failure\n"
    );

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[test]
fn run_rejects_an_events_path_that_would_replace_the_outcome() {
    let directory = temporary_directory("path-alias");
    let input = directory.join("request.json");
    let shared = directory.join("shared.json");
    fs::write(
        &input,
        include_bytes!("../../../tests/fixtures/cli/request-v1.json"),
    )
    .expect("fixture input should be written");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            shared.to_str().expect("output path is UTF-8"),
            "--events",
            shared.to_str().expect("events path is UTF-8"),
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(2));
    assert!(!shared.exists());
    assert_eq!(
        String::from_utf8(process.stderr).expect("stderr should be UTF-8"),
        "polygon-nesting: malformed input or invocation\n"
    );

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[cfg(unix)]
#[test]
fn run_rejects_an_events_alias_through_a_symlinked_parent_and_dotdot() {
    let directory = temporary_directory("events-symlink-parent-alias");
    let input = directory.join("request.json");
    let target = directory.join("target");
    let target_subdirectory = target.join("subdirectory");
    let link = directory.join("link");
    let output = target.join("result.json");
    let events = link.join("..").join("result.json");
    fs::create_dir_all(&target_subdirectory).expect("symlink target should be created");
    std::os::unix::fs::symlink(&target_subdirectory, &link).expect("parent symlink should exist");
    fs::write(
        &input,
        include_bytes!("../../../tests/fixtures/cli/request-v1.json"),
    )
    .expect("fixture input should be written");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
            "--events",
            events.to_str().expect("events path is UTF-8"),
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(2));
    assert!(!output.exists(), "aliased artifacts must not be written");

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[cfg(unix)]
#[test]
fn malformed_recovery_rejects_an_events_alias_through_a_symlinked_parent_and_dotdot() {
    let directory = temporary_directory("malformed-events-symlink-parent-alias");
    let input = directory.join("request.json");
    let target = directory.join("target");
    let target_subdirectory = target.join("subdirectory");
    let link = directory.join("link");
    let output = target.join("result.json");
    let events = link.join("..").join("result.json");
    fs::create_dir_all(&target_subdirectory).expect("symlink target should be created");
    std::os::unix::fs::symlink(&target_subdirectory, &link).expect("parent symlink should exist");
    fs::write(
        &input,
        include_bytes!("../../../tests/fixtures/cli/request-v1.json"),
    )
    .expect("fixture input should be written");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
            "--events",
            events.to_str().expect("events path is UTF-8"),
            "--deadline-ms",
            "nope",
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(2));
    assert!(!output.exists(), "aliased artifacts must not be written");

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[cfg(unix)]
#[test]
fn run_rejects_a_missing_output_alias_behind_a_relative_symlink_parent() {
    let directory = temporary_directory("relative-symlink-parent-output-alias");
    let input = directory.join("request.json");
    let base = directory.join("base");
    let external = directory.join("external");
    let external_subdirectory = external.join("subdirectory");
    let jump = base.join("jump");
    let output = jump.join("..").join("artifact.json");
    let events = external.join("artifact.json");
    fs::create_dir_all(&base).expect("base directory should be created");
    fs::create_dir_all(&external_subdirectory).expect("external target should be created");
    std::os::unix::fs::symlink("../external/subdirectory", &jump)
        .expect("relative parent symlink should exist");
    fs::write(
        &input,
        include_bytes!("../../../tests/fixtures/cli/request-v1.json"),
    )
    .expect("fixture input should be written");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
            "--events",
            events.to_str().expect("events path is UTF-8"),
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(2));
    assert!(!events.exists(), "aliased artifacts must not be written");

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[cfg(unix)]
#[test]
fn malformed_recovery_rejects_a_missing_input_alias_behind_a_relative_symlink_parent() {
    let directory = temporary_directory("relative-symlink-parent-malformed-alias");
    let base = directory.join("base");
    let external = directory.join("external");
    let external_subdirectory = external.join("subdirectory");
    let jump = base.join("jump");
    let input = external.join("missing.json");
    let output = jump.join("..").join("missing.json");
    fs::create_dir_all(&base).expect("base directory should be created");
    fs::create_dir_all(&external_subdirectory).expect("external target should be created");
    std::os::unix::fs::symlink("../external/subdirectory", &jump)
        .expect("relative parent symlink should exist");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
            "--deadline-ms",
            "nope",
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(2));
    assert!(!input.exists(), "aliased input must not be created");

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[cfg(unix)]
#[test]
fn run_rejects_a_symlinked_output_alias_of_the_input() {
    let directory = temporary_directory("input-output-symlink-alias");
    let input = directory.join("request.json");
    let output = directory.join("output-link.json");
    let original = include_bytes!("../../../tests/fixtures/cli/request-v1.json");
    fs::write(&input, original).expect("fixture input should be written");
    std::os::unix::fs::symlink(&input, &output).expect("output symlink should be created");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(2));
    assert_eq!(
        fs::read(&input).expect("input should remain intact"),
        original
    );

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[test]
fn run_rejects_an_events_path_that_would_replace_the_input() {
    let directory = temporary_directory("input-events-alias");
    let input = directory.join("request.json");
    let output = directory.join("result.json");
    let original = include_bytes!("../../../tests/fixtures/cli/request-v1.json");
    fs::write(&input, original).expect("fixture input should be written");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
            "--events",
            input.to_str().expect("input path is UTF-8"),
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(2));
    assert_eq!(
        fs::read(&input).expect("input should remain intact"),
        original
    );
    assert!(!output.exists());

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[cfg(unix)]
#[test]
fn sigterm_writes_a_typed_cancellation_outcome() {
    assert_cooperative_signal(
        "TERM",
        include_bytes!("../../../tests/fixtures/cli/request-v1.json").to_vec(),
    );
}

#[cfg(unix)]
#[test]
fn sigint_writes_a_typed_cancellation_outcome() {
    assert_cooperative_signal(
        "INT",
        include_bytes!("../../../tests/fixtures/cli/request-v1.json").to_vec(),
    );
}

#[cfg(unix)]
#[test]
fn sigterm_before_an_archive_ineligible_request_writes_a_typed_cancellation_outcome() {
    let mut request = valid_request();
    request.settings.optimizer.intrinsic_shared_archive_enabled = false;

    assert_cooperative_signal(
        "TERM",
        serde_json::to_vec(&request).expect("archive-ineligible request should encode"),
    );
}

#[cfg(debug_assertions)]
#[test]
fn panic_injection_writes_a_sanitized_internal_failure() {
    let directory = temporary_directory("panic-injection");
    let input = directory.join("request.json");
    let output = directory.join("result.json");
    fs::write(
        &input,
        include_bytes!("../../../tests/fixtures/cli/request-v1.json"),
    )
    .expect("fixture input should be written");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .env("POLYGON_NESTING_TEST_PANIC", "1")
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(process.stderr).expect("stderr should be UTF-8"),
        "polygon-nesting: internal failure\n"
    );
    let outcome: Value = serde_json::from_slice(&fs::read(&output).expect("outcome should exist"))
        .expect("outcome should be JSON");
    assert_eq!(outcome["outcome"]["error"]["category"], "internal_failure");

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[test]
fn malformed_request_writes_a_sanitized_failure_outcome_and_malformed_exit() {
    let directory = temporary_directory("malformed");
    let input = directory.join("request.json");
    let output = directory.join("result.json");
    fs::write(&input, b"{").expect("invalid input should be written");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(2));
    assert_eq!(
        fs::read(&output).expect("outcome should exist"),
        br#"{"version":1,"outcome":{"status":"failure","error":{"category":"malformed_input","operation":"decode-request","message":"request could not be decoded"}}}"#
    );
    assert_eq!(
        String::from_utf8(process.stderr).expect("stderr should be UTF-8"),
        "polygon-nesting: malformed input or invocation\n"
    );

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[test]
fn structurally_invalid_request_uses_the_malformed_exit() {
    let directory = temporary_directory("invalid-structure");
    let input = directory.join("request.json");
    let output = directory.join("result.json");
    let mut request = valid_request();
    request.pieces.clear();
    fs::write(
        &input,
        serde_json::to_vec(&request).expect("request should encode"),
    )
    .expect("input should be written");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(2));
    let outcome: Value = serde_json::from_slice(&fs::read(&output).expect("outcome should exist"))
        .expect("outcome should be JSON");
    assert_eq!(outcome["outcome"]["error"]["category"], "malformed_input");

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[cfg(unix)]
#[test]
fn run_rejects_hardlinked_output_alias_of_input() {
    let directory = temporary_directory("input-output-hardlink-alias");
    let input = directory.join("request.json");
    let output = directory.join("result.json");
    let original = include_bytes!("../../../tests/fixtures/cli/request-v1.json");
    fs::write(&input, original).expect("fixture input should be written");
    fs::hard_link(&input, &output).expect("output hardlink should be created");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(2));
    assert_eq!(
        fs::read(&input).expect("input should remain intact"),
        original
    );
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[test]
fn malformed_deadline_value_writes_a_typed_failure_when_output_is_available() {
    let directory = temporary_directory("malformed-deadline-value");
    let input = directory.join("request.json");
    let output = directory.join("result.json");
    fs::write(
        &input,
        include_bytes!("../../../tests/fixtures/cli/request-v1.json"),
    )
    .expect("fixture input should be written");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
            "--deadline-ms",
            "nope",
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(2));
    let outcome: Value = serde_json::from_slice(&fs::read(&output).expect("outcome should exist"))
        .expect("outcome should be JSON");
    assert_eq!(outcome["outcome"]["error"]["category"], "malformed_input");
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[test]
fn duplicate_events_does_not_suppress_an_unambiguous_output_envelope() {
    let directory = temporary_directory("duplicate-events-output-recovery");
    let input = directory.join("request.json");
    let output = directory.join("result.json");
    let events = directory.join("events.ndjson");
    fs::write(
        &input,
        include_bytes!("../../../tests/fixtures/cli/request-v1.json"),
    )
    .expect("fixture input should be written");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
            "--events",
            events.to_str().unwrap(),
            "--events",
            events.to_str().unwrap(),
            "--deadline-ms",
            "nope",
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(2));
    let outcome: Value = serde_json::from_slice(&fs::read(&output).expect("outcome should exist"))
        .expect("outcome should be JSON");
    assert_eq!(outcome["outcome"]["error"]["category"], "malformed_input");
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[test]
fn malformed_recovery_does_not_overwrite_a_lexically_equal_input() {
    let directory = temporary_directory("malformed-recovery-lexical-alias");
    let input = directory.join("request.json");
    let original = include_bytes!("../../../tests/fixtures/cli/request-v1.json");
    fs::write(&input, original).expect("fixture input should be written");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            input.to_str().expect("input path is UTF-8"),
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(2));
    assert_eq!(
        fs::read(&input).expect("input should remain intact"),
        original
    );

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[cfg(unix)]
#[test]
fn malformed_recovery_does_not_overwrite_a_symlinked_input_alias() {
    let directory = temporary_directory("malformed-recovery-symlink-alias");
    let input = directory.join("request.json");
    let output = directory.join("result-link.json");
    let original = include_bytes!("../../../tests/fixtures/cli/request-v1.json");
    fs::write(&input, original).expect("fixture input should be written");
    std::os::unix::fs::symlink(&input, &output).expect("output symlink should be created");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(2));
    assert_eq!(
        fs::read(&input).expect("input should remain intact"),
        original
    );

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[cfg(unix)]
#[test]
fn malformed_recovery_does_not_overwrite_a_hardlinked_input_alias() {
    let directory = temporary_directory("malformed-recovery-hardlink-alias");
    let input = directory.join("request.json");
    let output = directory.join("result.json");
    let original = include_bytes!("../../../tests/fixtures/cli/request-v1.json");
    fs::write(&input, original).expect("fixture input should be written");
    fs::hard_link(&input, &output).expect("output hardlink should be created");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(2));
    assert_eq!(
        fs::read(&input).expect("input should remain intact"),
        original
    );

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[test]
fn huge_finite_request_timeout_remains_a_valid_success() {
    let directory = temporary_directory("huge-timeout");
    let input = directory.join("request.json");
    let output = directory.join("result.json");
    let mut request = valid_request();
    request.timeout_ms = 5e293;
    fs::write(
        &input,
        serde_json::to_vec(&request).expect("request should encode"),
    )
    .expect("request should be written");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(0));
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[test]
fn run_deadline_shortens_the_request_and_writes_a_typed_deadline_failure() {
    let directory = temporary_directory("deadline");
    let input = directory.join("request.json");
    let output = directory.join("result.json");
    fs::write(
        &input,
        include_bytes!("../../../tests/fixtures/cli/request-v1.json"),
    )
    .expect("fixture input should be written");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
            "--deadline-ms",
            "0.000001",
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(4));
    let outcome: Value = serde_json::from_slice(&fs::read(&output).expect("outcome should exist"))
        .expect("outcome should be JSON");
    assert_eq!(outcome["outcome"]["error"]["category"], "deadline_exceeded");

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[test]
fn deadline_cannot_lengthen_the_request_timeout() {
    let directory = temporary_directory("deadline-cannot-lengthen");
    let input = directory.join("request.json");
    let output = directory.join("result.json");
    let mut request = valid_request();
    request.timeout_ms = 0.000001;
    fs::write(
        &input,
        serde_json::to_vec(&request).expect("request should encode"),
    )
    .expect("request should be written");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
            "--deadline-ms",
            "1000",
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(4));
    let outcome: Value = serde_json::from_slice(&fs::read(&output).expect("outcome should exist"))
        .expect("outcome should be JSON");
    assert_eq!(outcome["outcome"]["error"]["category"], "deadline_exceeded");

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[test]
fn nonpositive_deadline_is_rejected_as_malformed_input() {
    let directory = temporary_directory("invalid-deadline");
    let input = directory.join("request.json");
    let output = directory.join("result.json");
    fs::write(
        &input,
        include_bytes!("../../../tests/fixtures/cli/request-v1.json"),
    )
    .expect("fixture input should be written");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
            "--deadline-ms",
            "0",
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(2));
    assert_eq!(
        String::from_utf8(process.stderr).expect("stderr should be UTF-8"),
        "polygon-nesting: malformed input or invocation\n"
    );

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[test]
fn output_write_failure_does_not_leave_a_temporary_outcome_artifact() {
    let directory = temporary_directory("output-write-temporary-cleanup");
    let input = directory.join("request.json");
    let output = directory.join("result.json");
    fs::write(
        &input,
        include_bytes!("../../../tests/fixtures/cli/request-v1.json"),
    )
    .expect("fixture input should be written");
    fs::create_dir(&output).expect("directory output should be created");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(5));
    let temporary_prefix = format!(
        ".{}.",
        output
            .file_name()
            .expect("output has a filename")
            .to_string_lossy()
    );
    let temporary_files = fs::read_dir(&directory)
        .expect("directory should remain readable")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(&temporary_prefix)
        })
        .collect::<Vec<_>>();
    assert!(
        temporary_files.is_empty(),
        "temporary outcomes should be removed: {temporary_files:?}"
    );

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[test]
fn output_write_failure_uses_the_write_failure_exit() {
    let directory = temporary_directory("output-write-failure");
    let input = directory.join("request.json");
    let output_parent = directory.join("not-a-directory");
    let output = output_parent.join("result.json");
    fs::write(
        &input,
        serde_json::to_vec(&valid_request()).expect("request should encode"),
    )
    .expect("input should be written");
    fs::write(&output_parent, b"not a directory").expect("blocking file should be written");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(5));
    assert_eq!(
        String::from_utf8(process.stderr).expect("stderr should be UTF-8"),
        "polygon-nesting: output or event write failure\n"
    );

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[test]
fn event_write_failure_uses_the_write_failure_exit() {
    let directory = temporary_directory("event-write-failure");
    let input = directory.join("request.json");
    let output = directory.join("result.json");
    let events_parent = directory.join("not-a-directory");
    let events = events_parent.join("events.ndjson");
    fs::write(
        &input,
        include_bytes!("../../../tests/fixtures/cli/request-v1.json"),
    )
    .expect("fixture input should be written");
    fs::write(&events_parent, b"not a directory").expect("blocking file should be written");

    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
            "--events",
            events.to_str().expect("events path is UTF-8"),
        ])
        .output()
        .expect("CLI should start");

    assert_eq!(process.status.code(), Some(5));
    assert!(output.exists());
    assert_eq!(
        String::from_utf8(process.stderr).expect("stderr should be UTF-8"),
        "polygon-nesting: output or event write failure\n"
    );

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[test]
fn info_is_a_versioned_engine_capability_response() {
    let process = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .arg("--info")
        .output()
        .expect("CLI should start");

    assert!(process.status.success());
    let info: Value = serde_json::from_slice(&process.stdout).expect("info should be JSON");
    assert_eq!(info["name"], "polygon-nesting");
    assert_eq!(info["version"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn run_uses_the_supplied_control_for_deterministic_cancellation() {
    let directory = temporary_directory("cancellation");
    let input = directory.join("request.json");
    let output = directory.join("result.json");
    fs::write(
        &input,
        include_bytes!("../../../tests/fixtures/cli/request-v1.json"),
    )
    .expect("fixture input should be written");
    let control = CancellationControl::new();
    assert!(control.cancel(CancelReason::Cancelled));

    let status = run(
        RunPaths {
            input: &input,
            output: &output,
            events: None,
        },
        &control,
    );

    assert_eq!(status, ExitStatus::CancellationOrDeadline);
    let outcome: Value = serde_json::from_slice(&fs::read(&output).expect("outcome should exist"))
        .expect("outcome should be JSON");
    assert_eq!(outcome["outcome"]["error"]["category"], "cancelled");

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[cfg(unix)]
fn assert_cooperative_signal(signal: &str, request: Vec<u8>) {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::process::Stdio;
    use std::sync::mpsc;
    use std::thread;

    let directory = temporary_directory(&format!("signal-{signal}"));
    let input = directory.join("request.fifo");
    let output = directory.join("result.json");
    let status = Command::new("mkfifo")
        .arg(&input)
        .status()
        .expect("mkfifo should start");
    assert!(status.success(), "mkfifo should succeed");

    let mut child = Command::new(env!("CARGO_BIN_EXE_polygon-nesting"))
        .args([
            "run",
            "--input",
            input.to_str().expect("input path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("CLI should start");
    let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
    let input_for_writer = input.clone();
    let writer = thread::spawn(move || {
        let file = OpenOptions::new()
            .write(true)
            .open(input_for_writer)
            .expect("FIFO reader should open after the signal handler is installed");
        ready_sender.send(file).expect("writer should be delivered");
    });
    let mut input_writer = ready_receiver
        .recv()
        .expect("CLI should be waiting for input");

    let status = Command::new("kill")
        .args([format!("-{signal}"), child.id().to_string()])
        .status()
        .expect("kill should start");
    assert!(status.success(), "signal should be delivered");
    input_writer
        .write_all(&request)
        .expect("request input should be released");
    drop(input_writer);
    writer.join().expect("writer should finish");

    let process = child.wait().expect("CLI should finish");
    assert_eq!(process.code(), Some(4));
    let outcome: Value = serde_json::from_slice(&fs::read(&output).expect("outcome should exist"))
        .expect("outcome should be JSON");
    assert_eq!(outcome["outcome"]["error"]["category"], "cancelled");

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

fn valid_request() -> EngineRequest {
    EngineRequest {
        version: ProtocolVersion::CURRENT,
        timeout_ms: 1_000.0,
        profile: EngineProfile::Compact,
        sheet: polygon_nesting_protocol::SheetSpec {
            width: 100.0,
            height: 100.0,
            label: "cli-fixture-sheet".to_owned(),
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
            source_file_id: "fixture".to_owned(),
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
                    source_line(0.0, 0.0, 10.0, 0.0),
                    source_line(10.0, 0.0, 10.0, 10.0),
                    source_line(10.0, 10.0, 0.0, 10.0),
                    source_line(0.0, 10.0, 0.0, 0.0),
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
                geometry_backend_id: "fixture-backend".to_owned(),
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
                ga_seed: "fixture".to_owned(),
                priority_order_mutation_enabled: true,
                transform_preference_mutation_enabled: true,
                placement_policy_mutation_enabled: true,
                placement_policy_id: PlacementPolicy::BalancedCompactness,
                placement_policy_ids: vec![PlacementPolicy::BalancedCompactness],
            },
        },
        history_mode: HistoryMode::Stream,
    }
}

fn source_line(x1: f64, y1: f64, x2: f64, y2: f64) -> SourceGeometrySegment {
    SourceGeometrySegment::Line(SourceLineSegment {
        x1,
        y1,
        x2,
        y2,
        bulge: None,
        source_curve: None,
    })
}
