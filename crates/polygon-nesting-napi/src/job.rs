use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use napi::bindgen_prelude::{AsyncTask, Function, Unknown};
use napi::{Env, Error, Result, Status, Task};
use napi_derive::napi;
use polygon_nesting_core::{CancelReason, EngineEventSink, Job};
use polygon_nesting_protocol::{EngineError, EngineEvent, EngineOutcome, SequencedEngineEvent};

use crate::compat::{decode_desktop_request, AdapterError};
use crate::diagnostics::{
    increment_terminal_cleanup_hooks_fired, increment_terminal_latch_close_requests_by_cleanup,
    record_last_job_diagnostics, JobDiagnostics,
};
use crate::events::{EventBridge, JsonEventFn, TerminalLatch};

use crate::registry::{native_cancellation_registry, CancellationLease, CancellationRegistry};

pub fn cancellation_reason_from_wire(reason: &str) -> Option<CancelReason> {
    match reason {
        "cancelled" => Some(CancelReason::Cancelled),
        "timeout" => Some(CancelReason::Deadline),
        _ => None,
    }
}

fn failure_envelope(error: impl serde::Serialize) -> String {
    let mut error = serde_json::to_value(error).expect("failure errors always serialize");
    if error
        .get("context")
        .and_then(serde_json::Value::as_object)
        .is_some_and(serde_json::Map::is_empty)
    {
        error
            .as_object_mut()
            .expect("serialized failure errors are objects")
            .remove("context");
    }
    serde_json::json!({ "ok": false, "error": error }).to_string()
}

pub fn project_adapter_error(error: AdapterError) -> String {
    failure_envelope(error)
}

pub fn project_engine_error(error: EngineError) -> String {
    let category = match error.category {
        polygon_nesting_protocol::EngineErrorCode::Cancelled => "worker_cancelled",
        polygon_nesting_protocol::EngineErrorCode::DeadlineExceeded => "worker_timeout",
        polygon_nesting_protocol::EngineErrorCode::InvalidGeometry
            if error.context.contains_key("preparedPieceId")
                && error.context.contains_key("sourcePieceId") =>
        {
            "irregular_source_geometry_missing"
        }
        polygon_nesting_protocol::EngineErrorCode::InvalidGeometry => "irregular_geometry_invalid",
        polygon_nesting_protocol::EngineErrorCode::EngineFailure
            if matches!(
                error.context.get("category").map(String::as_str),
                Some("scoring" | "search")
            ) || matches!(
                error.context.get("operation").map(String::as_str),
                Some("scoreCandidate" | "scoreState" | "scorePlacement" | "scoreLayout")
            ) =>
        {
            "irregular_scoring_error"
        }
        polygon_nesting_protocol::EngineErrorCode::EngineFailure => "irregular_no_valid_result",
        polygon_nesting_protocol::EngineErrorCode::InternalFailure
            if error.context.contains_key("service") && error.context.contains_key("operation") =>
        {
            "not_implemented"
        }
        polygon_nesting_protocol::EngineErrorCode::InternalFailure => "unknown_error",
        polygon_nesting_protocol::EngineErrorCode::MalformedInput
        | polygon_nesting_protocol::EngineErrorCode::ProtocolVersionMismatch => {
            "worker_protocol_error"
        }
        polygon_nesting_protocol::EngineErrorCode::ArchiveIneligible => {
            let reason = error
                .context
                .get("reason")
                .map(String::as_str)
                .unwrap_or("unknown");
            let reason = match reason {
                "archive-disabled" => "intrinsicSharedArchiveEnabled is false",
                "ga-active" => "GA is active",
                "short-side-fill" => "placementPolicyId is 'short-side-fill'",
                other => other,
            };
            return failure_envelope(serde_json::json!({
                "category": "not_implemented",
                "operation": "legacy-portfolio-unsupported",
                "message": format!("request settings are not intrinsic-shared-archive-eligible ({reason}); this backend only implements the Compact / Compact Short Side archive path and does not emulate TypeScript's legacy windowed-beam/GA path -- route this request to the TypeScript backend."),
                "context": {
                    "service": "irregular-native",
                    "operation": "legacy-portfolio-unsupported",
                },
            }));
        }
    };
    failure_envelope(serde_json::json!({
        "category": category,
        "operation": error.operation,
        "message": error.message,
        "context": error.context,
    }))
}

pub fn project_delivery_failure(status: impl std::fmt::Display) -> String {
    failure_envelope(serde_json::json!({
        "category": "worker_protocol_error",
        "operation": "nativeEventDelivery",
        "message": "native irregular event delivery failed",
        "context": { "napiStatus": status.to_string() },
    }))
}

pub fn project_panic(operation: &str) -> String {
    failure_envelope(serde_json::json!({
        "category": "unknown_error",
        "operation": operation,
        "message": "native irregular nesting job failed unexpectedly",
        "context": { "operation": operation, "backend": "rust" },
    }))
}

pub fn complete_engine_outcome(outcome: EngineOutcome) -> String {
    match outcome {
        EngineOutcome::Success { result, .. } => {
            serde_json::json!({ "ok": true, "result": result }).to_string()
        }
        EngineOutcome::Failure { error, .. } | EngineOutcome::ArchiveIneligible { error, .. } => {
            project_engine_error(error)
        }
    }
}

fn elapsed_millis(elapsed: Duration) -> f64 {
    elapsed.as_secs_f64() * 1_000.0
}

fn pre_execution_failure_diagnostics(elapsed: Duration) -> JobDiagnostics {
    JobDiagnostics::no_pool(env!("CARGO_PKG_VERSION"), elapsed_millis(elapsed))
}

fn diagnostics_for_engine_outcome(outcome: &EngineOutcome, elapsed: Duration) -> JobDiagnostics {
    let execution_diagnostics = match outcome {
        EngineOutcome::Success { diagnostics, .. }
        | EngineOutcome::Failure { diagnostics, .. }
        | EngineOutcome::ArchiveIneligible { diagnostics, .. } => diagnostics.clone(),
    };
    JobDiagnostics::from_execution_diagnostics(env!("CARGO_PKG_VERSION"), execution_diagnostics)
        .unwrap_or_else(|_| pre_execution_failure_diagnostics(elapsed))
}

fn commit_completion_diagnostics(diagnostics: &mut Option<JobDiagnostics>) {
    if let Some(diagnostics) = diagnostics.take() {
        record_last_job_diagnostics(diagnostics);
    }
}

struct NapiEventSink<'a> {
    bridge: &'a mut EventBridge,
    callback: &'a JsonEventFn,
    emit_state_snapshots: bool,
}

impl EngineEventSink for NapiEventSink<'_> {
    fn emit(&mut self, event: SequencedEngineEvent) {
        if !self.emit_state_snapshots && matches!(event.event, EngineEvent::StateSnapshot { .. }) {
            let _ = self.bridge.skip_core(event);
            return;
        }
        let _ = self.bridge.deliver_core_to_js(event, self.callback);
    }
}

struct TerminalCleanupHookData {
    latch: TerminalLatch,
    fired: Arc<AtomicBool>,
    registry: Arc<CancellationRegistry>,
    token: String,
    lease: Arc<CancellationLease>,
}

impl TerminalCleanupHookData {
    fn cleanup(&self) {
        if self.fired.swap(true, Ordering::AcqRel) {
            return;
        }
        increment_terminal_cleanup_hooks_fired();
        increment_terminal_latch_close_requests_by_cleanup();
        self.lease.control().cancel(CancelReason::Cancelled);
        self.latch.close_for_cleanup();
        self.registry.remove_if_current(&self.token, &self.lease);
    }
}

struct TerminalCleanupHook {
    data: *mut TerminalCleanupHookData,
    fired: Arc<AtomicBool>,
}

unsafe impl Send for TerminalCleanupHook {}

impl TerminalCleanupHook {
    fn register(
        env: &Env,
        latch: TerminalLatch,
        registry: Arc<CancellationRegistry>,
        token: String,
        lease: Arc<CancellationLease>,
    ) -> Result<Self> {
        let fired = Arc::new(AtomicBool::new(false));
        let data = Box::into_raw(Box::new(TerminalCleanupHookData {
            latch,
            fired: Arc::clone(&fired),
            registry,
            token,
            lease,
        }));
        let status = Status::from(unsafe {
            napi::sys::napi_add_env_cleanup_hook(
                env.raw(),
                Some(terminal_cleanup_hook),
                data.cast(),
            )
        });
        if status != Status::Ok {
            unsafe { drop(Box::from_raw(data)) };
            return Err(Error::new(
                status,
                "native irregular terminal cleanup hook registration failed",
            ));
        }
        Ok(Self { data, fired })
    }

    fn remove_and_reclaim(self, env: &Env) -> Result<()> {
        if self.fired.load(Ordering::Acquire) {
            return Ok(());
        }
        let status = Status::from(unsafe {
            napi::sys::napi_remove_env_cleanup_hook(
                env.raw(),
                Some(terminal_cleanup_hook),
                self.data.cast(),
            )
        });
        if status != Status::Ok {
            return Err(Error::new(
                status,
                "native irregular terminal cleanup hook removal failed",
            ));
        }
        unsafe { drop(Box::from_raw(self.data)) };
        Ok(())
    }
}

unsafe extern "C" fn terminal_cleanup_hook(data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let data = unsafe { Box::from_raw(data.cast::<TerminalCleanupHookData>()) };
    data.cleanup();
}

struct RegistryCleanup {
    registry: Arc<CancellationRegistry>,
    token: String,
    lease: Arc<CancellationLease>,
}

impl Drop for RegistryCleanup {
    fn drop(&mut self) {
        self.registry.remove_if_current(&self.token, &self.lease);
    }
}

fn run_lifecycle<T>(
    registry: Arc<CancellationRegistry>,
    token: String,
    lease: Arc<CancellationLease>,
    state: &mut T,
    execute: impl FnOnce(&mut T) -> String,
    terminal: impl FnOnce(&mut T) -> Option<Status>,
) -> String {
    let _cleanup = RegistryCleanup {
        registry,
        token,
        lease,
    };
    let mut resolved = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| execute(state)))
        .unwrap_or_else(|_| project_panic("runIrregularJob"));
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| terminal(state))) {
        Ok(Some(status)) => resolved = project_delivery_failure(format!("{status:?}")),
        Ok(None) => {}
        Err(_) => resolved = project_panic("runIrregularJob"),
    }
    resolved
}

pub struct RunIrregularJobTask {
    invocation_token: String,
    registry: Arc<CancellationRegistry>,
    lease: Arc<CancellationLease>,
    request_json: String,
    on_event: JsonEventFn,
    emit_state_snapshots: bool,
    invocation_started: Instant,
    latch: TerminalLatch,
    completion_diagnostics: Option<JobDiagnostics>,
    cleanup_hook: Option<TerminalCleanupHook>,
}

impl Task for RunIrregularJobTask {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> Result<Self::Output> {
        let mut bridge = EventBridge::new(self.latch.clone());
        let mut completion_diagnostics = None;
        let output = run_lifecycle(
            Arc::clone(&self.registry),
            self.invocation_token.clone(),
            Arc::clone(&self.lease),
            &mut bridge,
            |bridge| {
                let prepared = match decode_desktop_request(&self.request_json) {
                    Ok(prepared) => prepared,
                    Err(error) => {
                        completion_diagnostics = Some(pre_execution_failure_diagnostics(
                            self.invocation_started.elapsed(),
                        ));
                        return project_adapter_error(error);
                    }
                };
                let mut sink = NapiEventSink {
                    bridge,
                    callback: &self.on_event,
                    emit_state_snapshots: self.emit_state_snapshots,
                };
                match Job::new(&prepared.request, self.lease.control(), &mut sink).run() {
                    Ok(outcome) => {
                        completion_diagnostics = Some(diagnostics_for_engine_outcome(
                            &outcome,
                            self.invocation_started.elapsed(),
                        ));
                        complete_engine_outcome(outcome)
                    }
                    Err(error) => {
                        completion_diagnostics = Some(pre_execution_failure_diagnostics(
                            self.invocation_started.elapsed(),
                        ));
                        project_engine_error(error)
                    }
                }
            },
            |bridge| {
                let terminal_status = bridge.emit_terminal_to_js_and_wait(&self.on_event);
                bridge
                    .first_delivery_failure()
                    .or((terminal_status != Status::Ok).then_some(terminal_status))
            },
        );
        self.completion_diagnostics = completion_diagnostics;
        Ok(output)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        commit_completion_diagnostics(&mut self.completion_diagnostics);
        Ok(output)
    }

    fn finally(mut self, env: Env) -> Result<()> {
        if let Some(hook) = self.cleanup_hook.take() {
            hook.remove_and_reclaim(&env)?;
        }
        Ok(())
    }
}

#[napi]
pub fn cancel_irregular_job(invocation_token: String, reason: String) -> bool {
    cancellation_reason_from_wire(&reason)
        .is_some_and(|reason| native_cancellation_registry().cancel(&invocation_token, reason))
}

#[napi]
pub fn run_irregular_job(
    env: Env,
    request_json: String,
    invocation_token: String,
    on_event: Function<'_, String, Unknown<'static>>,
    emit_state_snapshots: bool,
) -> Result<AsyncTask<RunIrregularJobTask>> {
    let on_event = on_event.build_threadsafe_function::<String>().build()?;
    let registry = native_cancellation_registry();
    let lease = Arc::new(CancellationLease::new());
    registry
        .register(invocation_token.clone(), Arc::clone(&lease))
        .map_err(|()| {
            Error::new(
                Status::InvalidArg,
                "native irregular invocation token is already registered",
            )
        })?;
    let latch = TerminalLatch::new();
    let cleanup_hook = match TerminalCleanupHook::register(
        &env,
        latch.clone(),
        Arc::clone(&registry),
        invocation_token.clone(),
        Arc::clone(&lease),
    ) {
        Ok(hook) => hook,
        Err(error) => {
            registry.remove_if_current(&invocation_token, &lease);
            return Err(error);
        }
    };
    Ok(AsyncTask::new(RunIrregularJobTask {
        invocation_token,
        registry,
        lease,
        request_json,
        on_event,
        emit_state_snapshots,
        invocation_started: Instant::now(),
        latch,
        completion_diagnostics: None,
        cleanup_hook: Some(cleanup_hook),
    }))
}

#[cfg(test)]
mod tests {
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use super::*;
    use crate::events::EventFrame;

    const COMPACT_REQUEST: &str =
        include_str!("../../../tests/fixtures/mixed-61/300x300-compact/request.json");

    struct LifecycleState {
        bridge: EventBridge,
        terminal_latch: TerminalLatch,
        delivered: Arc<Mutex<Vec<EventFrame>>>,
        terminal_ack_status: Status,
        close_before_terminal: bool,
    }

    impl LifecycleState {
        fn new(terminal_ack_status: Status) -> Self {
            let terminal_latch = TerminalLatch::new();
            Self {
                bridge: EventBridge::new(terminal_latch.clone()),
                terminal_latch,
                delivered: Arc::new(Mutex::new(Vec::new())),
                terminal_ack_status,
                close_before_terminal: false,
            }
        }

        fn deliver_core(&mut self, event: SequencedEngineEvent) {
            let delivered = Arc::clone(&self.delivered);
            assert_eq!(
                self.bridge.deliver_core(event, move |frame| {
                    delivered.lock().expect("delivered frames lock").push(frame);
                    Status::Ok
                }),
                Status::Ok
            );
        }

        fn deliver_terminal(&mut self) -> Option<Status> {
            if self.close_before_terminal {
                self.terminal_latch.close_for_cleanup();
            }
            let delivered = Arc::clone(&self.delivered);
            let acknowledgement_status = self.terminal_ack_status;
            let terminal_status =
                self.bridge
                    .emit_terminal_and_wait(move |frame, acknowledgement| {
                        delivered.lock().expect("delivered frames lock").push(frame);
                        acknowledgement.acknowledge(acknowledgement_status);
                        Status::Ok
                    });
            self.bridge
                .first_delivery_failure()
                .or((terminal_status != Status::Ok).then_some(terminal_status))
        }

        fn frames(&self) -> Vec<EventFrame> {
            self.delivered
                .lock()
                .expect("delivered frames lock")
                .clone()
        }
    }

    struct LifecycleCoreSink<'a> {
        state: &'a mut LifecycleState,
        cancellation: Option<&'a CancellationLease>,
    }

    impl EngineEventSink for LifecycleCoreSink<'_> {
        fn emit(&mut self, event: SequencedEngineEvent) {
            let ordinal = event.ordinal;
            self.state.deliver_core(event);
            if ordinal == 0 {
                if let Some(lease) = self.cancellation {
                    lease.control().cancel(CancelReason::Cancelled);
                }
            }
        }
    }

    fn lifecycle_registry(token: &str) -> (Arc<CancellationRegistry>, Arc<CancellationLease>) {
        let registry = Arc::new(CancellationRegistry::new());
        let lease = Arc::new(CancellationLease::new());
        registry
            .register(token.to_string(), Arc::clone(&lease))
            .expect("test token registers");
        (registry, lease)
    }

    fn small_desktop_request() -> String {
        let mut request: serde_json::Value =
            serde_json::from_str(COMPACT_REQUEST).expect("fixture JSON");
        request["pieces"]
            .as_array_mut()
            .expect("fixture pieces")
            .truncate(1);
        request["sourcePieces"]
            .as_array_mut()
            .expect("fixture source pieces")
            .truncate(1);
        request["options"]["timeoutMs"] = serde_json::json!(1_000.0);
        request["options"]["historyMode"] = serde_json::json!("stream");
        request.to_string()
    }

    fn run_core(
        state: &mut LifecycleState,
        request_json: &str,
        lease: &CancellationLease,
    ) -> String {
        let prepared = decode_desktop_request(request_json).expect("small request decodes");
        let mut sink = LifecycleCoreSink {
            state,
            cancellation: None,
        };
        complete_engine_outcome(
            Job::new(&prepared.request, lease.control(), &mut sink)
                .run()
                .expect("core job outcome"),
        )
    }

    #[test]
    fn completion_diagnostics_follow_completion_order_not_compute_order() {
        let _diagnostics_guard = crate::diagnostics::test_guard();
        let mut first = Some(
            JobDiagnostics::from_execution_diagnostics(
                "first",
                polygon_nesting_protocol::ExecutionDiagnostics {
                    requested_workers: Some(2),
                    actual_workers: Some(2),
                    elapsed_ms: Some(1.0),
                    counters: Default::default(),
                },
            )
            .expect("first diagnostics"),
        );
        let mut second = Some(
            JobDiagnostics::from_execution_diagnostics(
                "second",
                polygon_nesting_protocol::ExecutionDiagnostics {
                    requested_workers: Some(8),
                    actual_workers: Some(4),
                    elapsed_ms: Some(2.0),
                    counters: Default::default(),
                },
            )
            .expect("second diagnostics"),
        );

        commit_completion_diagnostics(&mut second);
        let second_json: serde_json::Value =
            serde_json::from_str(&crate::get_last_job_diagnostics()).expect("diagnostics JSON");
        assert_eq!(second_json["threadCountRequested"], 8);

        commit_completion_diagnostics(&mut first);
        let first_json: serde_json::Value =
            serde_json::from_str(&crate::get_last_job_diagnostics()).expect("diagnostics JSON");
        assert_eq!(first_json["threadCountRequested"], 2);
    }

    #[test]
    fn malformed_pre_execution_completion_replaces_previous_diagnostics() {
        let _diagnostics_guard = crate::diagnostics::test_guard();
        let mut previous = Some(
            JobDiagnostics::from_execution_diagnostics(
                "previous",
                polygon_nesting_protocol::ExecutionDiagnostics {
                    requested_workers: Some(3),
                    actual_workers: Some(2),
                    elapsed_ms: Some(1.0),
                    counters: Default::default(),
                },
            )
            .expect("previous diagnostics"),
        );
        commit_completion_diagnostics(&mut previous);
        let mut malformed_pre_execution =
            Some(pre_execution_failure_diagnostics(Duration::from_millis(7)));

        commit_completion_diagnostics(&mut malformed_pre_execution);

        let json: serde_json::Value =
            serde_json::from_str(&crate::get_last_job_diagnostics()).expect("diagnostics JSON");
        assert_eq!(json["threadCountRequested"], 1);
        assert_eq!(json["threadCountUsed"], 1);
        assert_eq!(json["wallClockMs"], 7.0);
    }

    #[test]
    fn archive_ineligible_diagnostics_commit_measured_no_pool_elapsed_at_completion() {
        let _diagnostics_guard = crate::diagnostics::test_guard();
        let outcome = EngineOutcome::ArchiveIneligible {
            reason: polygon_nesting_protocol::ArchiveIneligibilityReason::ArchiveDisabled,
            error: EngineError::archive_ineligible(
                polygon_nesting_protocol::ArchiveIneligibilityReason::ArchiveDisabled,
            ),
            diagnostics: Default::default(),
        };
        let mut diagnostics = Some(diagnostics_for_engine_outcome(
            &outcome,
            Duration::from_millis(23),
        ));

        assert!(complete_engine_outcome(outcome).contains("not_implemented"));
        commit_completion_diagnostics(&mut diagnostics);

        let json: serde_json::Value =
            serde_json::from_str(&crate::get_last_job_diagnostics()).expect("diagnostics JSON");
        assert_eq!(json["threadCountRequested"], 1);
        assert_eq!(json["threadCountUsed"], 1);
        assert_eq!(json["wallClockMs"], 23.0);
    }

    #[test]
    fn lifecycle_projects_decode_failure_and_removes_current_lease() {
        let (registry, lease) = lifecycle_registry("decode");
        let mut state = LifecycleState::new(Status::Ok);
        let output = run_lifecycle(
            Arc::clone(&registry),
            "decode".into(),
            Arc::clone(&lease),
            &mut state,
            |_| {
                project_adapter_error(
                    decode_desktop_request("{not json").expect_err("decode fails"),
                )
            },
            LifecycleState::deliver_terminal,
        );

        let envelope: serde_json::Value = serde_json::from_str(&output).expect("failure envelope");
        assert_eq!(envelope["error"]["category"], "worker_protocol_error");
        assert_eq!(envelope["error"]["operation"], "decodeRequestJson");
        assert_eq!(state.frames(), vec![EventFrame::Terminal { ordinal: 0 }]);
        assert!(!registry.cancel("decode", CancelReason::Cancelled));
    }

    #[test]
    fn lifecycle_projects_typed_core_failure_and_removes_current_lease() {
        let (registry, lease) = lifecycle_registry("core-failure");
        let mut state = LifecycleState::new(Status::Ok);
        let mut request: serde_json::Value =
            serde_json::from_str(&small_desktop_request()).expect("small request JSON");
        request["options"]["irregularSettings"]["optimizer"]["intrinsicSharedArchiveEnabled"] =
            serde_json::json!(false);
        let request_json = request.to_string();
        let output = run_lifecycle(
            Arc::clone(&registry),
            "core-failure".into(),
            Arc::clone(&lease),
            &mut state,
            |state| run_core(state, &request_json, &lease),
            LifecycleState::deliver_terminal,
        );

        let envelope: serde_json::Value = serde_json::from_str(&output).expect("failure envelope");
        assert_eq!(
            envelope,
            serde_json::json!({
                "ok": false,
                "error": {
                    "category": "not_implemented",
                    "operation": "legacy-portfolio-unsupported",
                    "message": "request settings are not intrinsic-shared-archive-eligible (intrinsicSharedArchiveEnabled is false); this backend only implements the Compact / Compact Short Side archive path and does not emulate TypeScript's legacy windowed-beam/GA path -- route this request to the TypeScript backend.",
                    "context": {
                        "service": "irregular-native",
                        "operation": "legacy-portfolio-unsupported",
                    },
                },
            })
        );
        assert_eq!(state.frames(), vec![EventFrame::Terminal { ordinal: 0 }]);
        assert!(!registry.cancel("core-failure", CancelReason::Cancelled));
    }

    #[test]
    fn lifecycle_projects_terminal_callback_failure_after_terminal_handling() {
        let (registry, lease) = lifecycle_registry("delivery");
        let mut state = LifecycleState::new(Status::QueueFull);
        let output = run_lifecycle(
            Arc::clone(&registry),
            "delivery".into(),
            Arc::clone(&lease),
            &mut state,
            |_| project_panic("core"),
            LifecycleState::deliver_terminal,
        );

        let envelope: serde_json::Value = serde_json::from_str(&output).expect("failure envelope");
        assert_eq!(envelope["error"]["operation"], "nativeEventDelivery");
        assert_eq!(envelope["error"]["context"]["napiStatus"], "QueueFull");
        assert_eq!(state.frames(), vec![EventFrame::Terminal { ordinal: 0 }]);
        assert!(!registry.cancel("delivery", CancelReason::Cancelled));
    }

    #[test]
    fn lifecycle_sanitizes_panic_and_removes_current_lease() {
        let (registry, lease) = lifecycle_registry("panic");
        let mut state = LifecycleState::new(Status::Ok);
        let output = run_lifecycle(
            Arc::clone(&registry),
            "panic".into(),
            Arc::clone(&lease),
            &mut state,
            |_| panic!("secret panic payload"),
            LifecycleState::deliver_terminal,
        );

        let envelope: serde_json::Value = serde_json::from_str(&output).expect("failure envelope");
        assert_eq!(envelope["error"]["category"], "unknown_error");
        assert_eq!(envelope["error"]["operation"], "runIrregularJob");
        assert_eq!(state.frames(), vec![EventFrame::Terminal { ordinal: 0 }]);
        assert!(!registry.cancel("panic", CancelReason::Cancelled));
    }

    #[test]
    fn lifecycle_sanitizes_terminal_delivery_panic_and_removes_current_lease() {
        let (registry, lease) = lifecycle_registry("terminal-panic");
        let mut state = LifecycleState::new(Status::Ok);
        let mut terminal_attempts = 0;
        let output = run_lifecycle(
            Arc::clone(&registry),
            "terminal-panic".into(),
            Arc::clone(&lease),
            &mut state,
            |_| project_panic("core"),
            |_| {
                terminal_attempts += 1;
                panic!("terminal callback panic")
            },
        );

        let envelope: serde_json::Value = serde_json::from_str(&output).expect("failure envelope");
        assert_eq!(envelope["error"]["category"], "unknown_error");
        assert_eq!(envelope["error"]["operation"], "runIrregularJob");
        assert_eq!(terminal_attempts, 1);
        assert!(!registry.cancel("terminal-panic", CancelReason::Cancelled));
    }

    #[test]
    fn lifecycle_observes_cancellation_projects_it_and_removes_current_lease() {
        let _diagnostics_guard = crate::diagnostics::test_guard();
        let (registry, lease) = lifecycle_registry("cancel");
        let request_json = small_desktop_request();
        let mut state = LifecycleState::new(Status::Ok);
        let mut completion_diagnostics = None;
        let output = run_lifecycle(
            Arc::clone(&registry),
            "cancel".into(),
            Arc::clone(&lease),
            &mut state,
            |state| {
                let prepared =
                    decode_desktop_request(&request_json).expect("small request decodes");
                let mut sink = LifecycleCoreSink {
                    state,
                    cancellation: Some(&lease),
                };
                let outcome = Job::new(&prepared.request, lease.control(), &mut sink)
                    .run()
                    .expect("core job outcome");
                completion_diagnostics = Some(diagnostics_for_engine_outcome(
                    &outcome,
                    Duration::from_millis(1),
                ));
                complete_engine_outcome(outcome)
            },
            LifecycleState::deliver_terminal,
        );
        commit_completion_diagnostics(&mut completion_diagnostics);

        let envelope: serde_json::Value = serde_json::from_str(&output).expect("failure envelope");
        assert_eq!(envelope["error"]["category"], "worker_cancelled");
        let diagnostics: serde_json::Value =
            serde_json::from_str(&crate::get_last_job_diagnostics()).expect("diagnostics JSON");
        assert!(diagnostics["threadCountRequested"].is_u64());
        assert!(diagnostics["threadCountUsed"].is_u64());
        assert!(diagnostics["wallClockMs"].is_number());
        let frames = state.frames();
        assert!(matches!(frames.first(), Some(EventFrame::Core(event)) if event.ordinal == 0));
        assert_eq!(frames.last(), Some(&EventFrame::Terminal { ordinal: 1 }));
        assert!(!registry.cancel("cancel", CancelReason::Cancelled));
    }

    #[test]
    fn lifecycle_preserves_core_ordinals_and_appends_one_terminal_frame() {
        let (registry, lease) = lifecycle_registry("events");
        let request_json = small_desktop_request();
        let mut state = LifecycleState::new(Status::Ok);
        let output = run_lifecycle(
            Arc::clone(&registry),
            "events".into(),
            Arc::clone(&lease),
            &mut state,
            |state| run_core(state, &request_json, &lease),
            LifecycleState::deliver_terminal,
        );

        let envelope: serde_json::Value = serde_json::from_str(&output).expect("success envelope");
        assert_eq!(envelope["ok"], true);
        let frames = state.frames();
        let terminal_count = frames
            .iter()
            .filter(|frame| matches!(frame, EventFrame::Terminal { .. }))
            .count();
        assert_eq!(terminal_count, 1);
        let terminal = frames.last().expect("terminal frame");
        assert!(
            matches!(terminal, EventFrame::Terminal { ordinal } if *ordinal + 1 == frames.len() as u64)
        );
        assert!(frames
            .iter()
            .enumerate()
            .all(|(ordinal, frame)| frame.ordinal() == ordinal as u64));
        assert!(!registry.cancel("events", CancelReason::Cancelled));
    }

    #[test]
    fn terminal_cleanup_hook_removes_its_current_lease_and_allows_token_reuse() {
        let _diagnostics_guard = crate::diagnostics::test_guard();
        let (registry, lease) = lifecycle_registry("hook-cleanup");
        let data = Box::new(TerminalCleanupHookData {
            latch: TerminalLatch::new(),
            fired: Arc::new(AtomicBool::new(false)),
            registry: Arc::clone(&registry),
            token: "hook-cleanup".to_string(),
            lease: Arc::clone(&lease),
        });

        unsafe { terminal_cleanup_hook(Box::into_raw(data).cast()) };

        assert_eq!(lease.control().reason(), Some(CancelReason::Cancelled));
        assert!(!registry.cancel("hook-cleanup", CancelReason::Cancelled));
        let replacement = Arc::new(CancellationLease::new());
        registry
            .register("hook-cleanup".to_string(), Arc::clone(&replacement))
            .expect("cleanup releases token for a replacement lease");
        registry.remove_if_current("hook-cleanup", &replacement);
    }

    #[test]
    fn terminal_cleanup_hook_preserves_an_existing_cancellation_reason() {
        let _diagnostics_guard = crate::diagnostics::test_guard();
        let (registry, lease) = lifecycle_registry("hook-reason");
        lease.control().cancel(CancelReason::Deadline);
        let data = TerminalCleanupHookData {
            latch: TerminalLatch::new(),
            fired: Arc::new(AtomicBool::new(false)),
            registry,
            token: "hook-reason".to_string(),
            lease: Arc::clone(&lease),
        };

        data.cleanup();

        assert_eq!(lease.control().reason(), Some(CancelReason::Deadline));
    }

    #[test]
    fn stale_terminal_cleanup_hook_cannot_remove_a_reused_token_lease() {
        let _diagnostics_guard = crate::diagnostics::test_guard();
        let (registry, lease) = lifecycle_registry("stale-hook");
        let data = Box::new(TerminalCleanupHookData {
            latch: TerminalLatch::new(),
            fired: Arc::new(AtomicBool::new(false)),
            registry: Arc::clone(&registry),
            token: "stale-hook".to_string(),
            lease: Arc::clone(&lease),
        });
        registry.remove_if_current("stale-hook", &lease);
        let replacement = Arc::new(CancellationLease::new());
        registry
            .register("stale-hook".to_string(), Arc::clone(&replacement))
            .expect("replacement token registers");

        unsafe { terminal_cleanup_hook(Box::into_raw(data).cast()) };

        assert!(registry.cancel("stale-hook", CancelReason::Cancelled));
        assert_eq!(
            replacement.control().reason(),
            Some(CancelReason::Cancelled)
        );
    }

    #[test]
    fn terminal_cleanup_hook_wakes_terminal_delivery_and_is_idempotent() {
        let _diagnostics_guard = crate::diagnostics::test_guard();
        let (registry, lease) = lifecycle_registry("hook-wakeup");
        let latch = TerminalLatch::new();
        let data = TerminalCleanupHookData {
            latch: latch.clone(),
            fired: Arc::new(AtomicBool::new(false)),
            registry: Arc::clone(&registry),
            token: "hook-wakeup".to_string(),
            lease: Arc::clone(&lease),
        };
        let worker_latch = latch.clone();
        let (admitted_sender, admitted_receiver) = mpsc::channel();
        let (result_sender, result_receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            let mut bridge = EventBridge::new(worker_latch);
            let status = bridge.emit_terminal_and_wait(|_frame, _acknowledgement| {
                admitted_sender.send(()).expect("terminal admission");
                Status::Ok
            });
            result_sender.send(status).expect("terminal result");
        });

        admitted_receiver.recv().expect("terminal is waiting");
        data.cleanup();
        data.cleanup();

        assert_eq!(
            result_receiver.recv().expect("terminal wakeup"),
            Status::Closing
        );
        worker.join().expect("terminal worker joins");
        assert!(!registry.cancel("hook-wakeup", CancelReason::Cancelled));
    }

    #[test]
    fn lifecycle_projects_environment_cleanup_terminal_wakeup_through_delivery_boundary() {
        let (registry, lease) = lifecycle_registry("cleanup");
        let mut state = LifecycleState::new(Status::Ok);
        state.close_before_terminal = true;
        let output = run_lifecycle(
            Arc::clone(&registry),
            "cleanup".into(),
            Arc::clone(&lease),
            &mut state,
            |_| project_panic("core"),
            LifecycleState::deliver_terminal,
        );

        let envelope: serde_json::Value = serde_json::from_str(&output).expect("failure envelope");
        assert_eq!(envelope["error"]["operation"], "nativeEventDelivery");
        assert_eq!(envelope["error"]["context"]["napiStatus"], "Closing");
        assert!(state.frames().is_empty());
        assert!(!registry.cancel("cleanup", CancelReason::Cancelled));
    }
}
