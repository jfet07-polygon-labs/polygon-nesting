use std::io::Write;
use std::process::{Command, Stdio};

const COMPACT_REQUEST: &str =
    include_str!("../../../tests/fixtures/mixed-61/300x300-compact/request.json");

#[test]
fn parity_adapter_binary_emits_neutral_engine_request_json() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_parity-desktop-request-adapter"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("adapter binary starts");
    child
        .stdin
        .take()
        .expect("adapter stdin")
        .write_all(COMPACT_REQUEST.as_bytes())
        .expect("desktop request is written");
    let output = child.wait_with_output().expect("adapter exits");

    assert!(
        output.status.success(),
        "adapter stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let adapted: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("neutral JSON output");
    assert_eq!(adapted["version"], 1);
    assert_eq!(adapted["profile"], "compact");
    assert!(adapted.get("jobId").is_none());
}
