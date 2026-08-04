//! Non-semantic process diagnostics kept outside engine results and events.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use polygon_nesting_core::caches::{CacheNamespaceTelemetry, CacheTelemetrySnapshot};
use polygon_nesting_core::search::layout_scorer::FreeMaterialCacheTelemetry;
use polygon_nesting_protocol::ExecutionDiagnostics;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JobDiagnostics {
    pub(crate) backend_version: String,
    pub(crate) thread_count_used: u32,
    pub(crate) thread_count_requested: u32,
    pub(crate) wall_clock_ms: f64,
    pub(crate) cache_telemetry: CacheTelemetrySnapshot,
    pub(crate) free_material_cache_telemetry: FreeMaterialCacheTelemetry,
}

impl JobDiagnostics {
    pub(crate) fn from_execution_diagnostics(
        backend_version: impl Into<String>,
        diagnostics: ExecutionDiagnostics,
    ) -> Result<Self, String> {
        let thread_count_requested = diagnostics
            .requested_workers
            .ok_or_else(|| "diagnostics missing requested_workers".to_string())?;
        let thread_count_used = diagnostics
            .actual_workers
            .ok_or_else(|| "diagnostics missing actual_workers".to_string())?;
        let wall_clock_ms = diagnostics
            .elapsed_ms
            .ok_or_else(|| "diagnostics missing elapsed_ms".to_string())?;
        if !wall_clock_ms.is_finite() || wall_clock_ms < 0.0 {
            return Err("diagnostics elapsed_ms must be finite and non-negative".to_string());
        }

        let mut cache_telemetry = CacheTelemetrySnapshot::default();
        let mut free_material_cache_telemetry = FreeMaterialCacheTelemetry::default();
        for (key, value) in diagnostics.counters {
            apply_counter(
                &mut cache_telemetry,
                &mut free_material_cache_telemetry,
                &key,
                value,
            )?;
        }
        Ok(Self {
            backend_version: backend_version.into(),
            thread_count_used,
            thread_count_requested,
            wall_clock_ms,
            cache_telemetry,
            free_material_cache_telemetry,
        })
    }

    #[cfg(test)]
    fn for_test(backend_version: &str) -> Self {
        let mut cache_telemetry = CacheTelemetrySnapshot::default();
        cache_telemetry.cap_bytes = 64;
        cache_telemetry.namespace_mut("sheet-ifp-v1").hits = 3;
        Self {
            backend_version: backend_version.to_string(),
            thread_count_used: 1,
            thread_count_requested: 2,
            wall_clock_ms: 1.5,
            cache_telemetry,
            free_material_cache_telemetry: FreeMaterialCacheTelemetry {
                hits: 4,
                misses: 1,
                ..FreeMaterialCacheTelemetry::default()
            },
        }
    }
}

fn apply_counter(
    geometry: &mut CacheTelemetrySnapshot,
    free_material: &mut FreeMaterialCacheTelemetry,
    key: &str,
    value: u64,
) -> Result<(), String> {
    if let Some(namespace_key) = key.strip_prefix("geometry_cache.namespace.") {
        let (namespace, field) = namespace_key
            .rsplit_once('.')
            .filter(|(namespace, field)| !namespace.is_empty() && !field.is_empty())
            .ok_or_else(|| format!("malformed geometry cache namespace counter: {key}"))?;
        apply_namespace_counter(geometry.namespace_mut(namespace), field, value, key)?;
        return Ok(());
    }
    if let Some(field) = key.strip_prefix("geometry_cache.") {
        match field {
            "cap_bytes" => geometry.cap_bytes = value,
            "current_bytes" => geometry.current_bytes = value,
            "peak_bytes" => geometry.peak_bytes = value,
            "admissions" => geometry.admissions = value,
            "replacements" => geometry.replacements = value,
            "evictions" => geometry.evictions = value,
            "evicted_bytes" => geometry.evicted_bytes = value,
            "oversized_rejections" => geometry.oversized_rejections = value,
            "instances" => geometry.cache_instances = value,
            _ => return Err(format!("unknown geometry cache counter: {key}")),
        }
        return Ok(());
    }
    if let Some(field) = key.strip_prefix("free_material_cache.") {
        match field {
            "cap_bytes" => free_material.cap_bytes = value,
            "current_bytes" => free_material.current_bytes = value,
            "peak_bytes" => free_material.peak_bytes = value,
            "entries" => free_material.entries = value,
            "admissions" => free_material.admissions = value,
            "replacements" => free_material.replacements = value,
            "evictions" => free_material.evictions = value,
            "evicted_bytes" => free_material.evicted_bytes = value,
            "oversized_rejections" => free_material.oversized_rejections = value,
            "hits" => free_material.hits = value,
            "misses" => free_material.misses = value,
            _ => return Err(format!("unknown free material cache counter: {key}")),
        }
        return Ok(());
    }
    Err(format!("unknown diagnostics counter: {key}"))
}

fn apply_namespace_counter(
    counters: &mut CacheNamespaceTelemetry,
    field: &str,
    value: u64,
    key: &str,
) -> Result<(), String> {
    match field {
        "lookups" => counters.lookups = value,
        "hits" => counters.hits = value,
        "misses" => counters.misses = value,
        "stores" => counters.stores = value,
        "stale_detections" => counters.stale_detections = value,
        "stale_removals" => counters.stale_removals = value,
        "duplicate_computations" => counters.duplicate_computations = value,
        "single_flight_waits" => counters.single_flight_waits = value,
        "shard_lock_wait_nanos" => counters.shard_lock_wait_nanos = value,
        "shard_lock_contended_acquisitions" => counters.shard_lock_contended_acquisitions = value,
        "front_cache_hits" => counters.front_cache_hits = value,
        "backing_cache_hits" => counters.backing_cache_hits = value,
        "cloning_hits" => counters.cloning_hits = value,
        "cap_bytes" => counters.cap_bytes = value,
        "admissions" => counters.admissions = value,
        "replacements" => counters.replacements = value,
        "evictions" => counters.evictions = value,
        "evicted_bytes" => counters.evicted_bytes = value,
        "oversized_rejections" => counters.oversized_rejections = value,
        "entries" => counters.entries = value,
        "approx_bytes" => counters.approx_bytes = value,
        "peak_bytes" => counters.peak_bytes = value,
        "computation_time_nanos" => counters.computation_time_nanos = value,
        _ => return Err(format!("unknown geometry cache namespace counter: {key}")),
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProcessLifecycleDiagnostics {
    terminal_cleanup_hooks_fired: u64,
    terminal_latch_close_requests_by_cleanup: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LastJobDiagnosticsJson<'a> {
    #[serde(flatten)]
    job: &'a JobDiagnostics,
    process_lifecycle: ProcessLifecycleDiagnostics,
}

static TERMINAL_CLEANUP_HOOKS_FIRED: AtomicU64 = AtomicU64::new(0);
static TERMINAL_LATCH_CLOSE_REQUESTS_BY_CLEANUP: AtomicU64 = AtomicU64::new(0);

pub(crate) fn increment_terminal_cleanup_hooks_fired() {
    TERMINAL_CLEANUP_HOOKS_FIRED.fetch_add(1, Ordering::SeqCst);
}

pub(crate) fn increment_terminal_latch_close_requests_by_cleanup() {
    TERMINAL_LATCH_CLOSE_REQUESTS_BY_CLEANUP.fetch_add(1, Ordering::SeqCst);
}

fn process_lifecycle_diagnostics() -> ProcessLifecycleDiagnostics {
    ProcessLifecycleDiagnostics {
        terminal_cleanup_hooks_fired: TERMINAL_CLEANUP_HOOKS_FIRED.load(Ordering::SeqCst),
        terminal_latch_close_requests_by_cleanup: TERMINAL_LATCH_CLOSE_REQUESTS_BY_CLEANUP
            .load(Ordering::SeqCst),
    }
}

fn last_job_diagnostics_slot() -> &'static Mutex<Option<JobDiagnostics>> {
    static SLOT: OnceLock<Mutex<Option<JobDiagnostics>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

pub(crate) fn record_last_job_diagnostics(diagnostics: JobDiagnostics) {
    let mut slot = last_job_diagnostics_slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *slot = Some(diagnostics);
}

pub(crate) fn last_job_diagnostics_json() -> String {
    let slot = last_job_diagnostics_slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match slot.as_ref() {
        Some(diagnostics) => serde_json::to_string(&LastJobDiagnosticsJson {
            job: diagnostics,
            process_lifecycle: process_lifecycle_diagnostics(),
        })
        .expect("JobDiagnostics always serializes"),
        None => "null".to_string(),
    }
}

#[cfg(test)]
fn reset_for_test(_guard: &std::sync::MutexGuard<'static, ()>) {
    *last_job_diagnostics_slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    TERMINAL_CLEANUP_HOOKS_FIRED.store(0, Ordering::SeqCst);
    TERMINAL_LATCH_CLOSE_REQUESTS_BY_CLEANUP.store(0, Ordering::SeqCst);
}

#[cfg(test)]
fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn diagnostics_are_null_before_first_completed_job() {
        let _guard = test_guard();
        reset_for_test(&_guard);
        assert_eq!(last_job_diagnostics_json(), "null");
    }

    #[test]
    fn completed_jobs_replace_the_process_global_snapshot() {
        let _guard = test_guard();
        reset_for_test(&_guard);
        record_last_job_diagnostics(JobDiagnostics::for_test("first"));
        record_last_job_diagnostics(JobDiagnostics::for_test("second"));

        let json: serde_json::Value = serde_json::from_str(&last_job_diagnostics_json()).unwrap();
        assert_eq!(json["backendVersion"], "second");
        assert_eq!(json["threadCountUsed"], 1);
        assert_eq!(json["threadCountRequested"], 2);
        assert_eq!(json["wallClockMs"], 1.5);
    }

    #[test]
    fn diagnostics_json_has_typed_populated_telemetry_and_lifecycle() {
        let _guard = test_guard();
        reset_for_test(&_guard);
        record_last_job_diagnostics(JobDiagnostics::for_test("typed"));
        let json: serde_json::Value = serde_json::from_str(&last_job_diagnostics_json()).unwrap();
        assert!(json["cacheTelemetry"].is_object());
        assert_eq!(json["cacheTelemetry"]["capBytes"], 64);
        assert_eq!(
            json["cacheTelemetry"]["namespaces"]["sheet-ifp-v1"]["hits"],
            3
        );
        assert!(json["freeMaterialCacheTelemetry"].is_object());
        assert_eq!(json["freeMaterialCacheTelemetry"]["hits"], 4);
        assert_eq!(json["freeMaterialCacheTelemetry"]["misses"], 1);
        assert!(json["processLifecycle"].is_object());
        assert!(json["processLifecycle"]["terminalCleanupHooksFired"].is_u64());
        assert!(json["processLifecycle"]["terminalLatchCloseRequestsByCleanup"].is_u64());
    }

    #[test]
    fn execution_diagnostics_constructor_reconstructs_all_supported_fields() {
        let mut counters = BTreeMap::new();
        counters.insert("geometry_cache.cap_bytes".to_string(), 10);
        counters.insert("geometry_cache.namespace.nfp.hits".to_string(), 2);
        counters.insert("free_material_cache.misses".to_string(), 4);
        let diagnostics = ExecutionDiagnostics {
            requested_workers: Some(8),
            actual_workers: Some(3),
            elapsed_ms: Some(12.5),
            counters,
        };
        let job = JobDiagnostics::from_execution_diagnostics("typed", diagnostics).unwrap();
        assert_eq!(job.thread_count_requested, 8);
        assert_eq!(job.thread_count_used, 3);
        assert_eq!(job.wall_clock_ms, 12.5);
        assert_eq!(job.cache_telemetry.cap_bytes, 10);
        assert_eq!(job.cache_telemetry.namespace("nfp").hits, 2);
        assert_eq!(job.free_material_cache_telemetry.misses, 4);
    }

    #[test]
    fn execution_diagnostics_constructor_rejects_unknown_counters() {
        let mut counters = BTreeMap::new();
        counters.insert("unknown.counter".to_string(), 1);
        let error = JobDiagnostics::from_execution_diagnostics(
            "typed",
            ExecutionDiagnostics {
                requested_workers: Some(1),
                actual_workers: Some(1),
                elapsed_ms: Some(0.0),
                counters,
            },
        )
        .unwrap_err();
        assert!(error.contains("unknown diagnostics counter"));
    }

    #[test]
    fn lifecycle_counters_are_exposed_and_increment_exactly_once() {
        let _guard = test_guard();
        reset_for_test(&_guard);
        record_last_job_diagnostics(JobDiagnostics::for_test("counters"));
        let initial: serde_json::Value =
            serde_json::from_str(&last_job_diagnostics_json()).unwrap();
        assert_eq!(initial["processLifecycle"]["terminalCleanupHooksFired"], 0);
        assert_eq!(
            initial["processLifecycle"]["terminalLatchCloseRequestsByCleanup"],
            0
        );

        increment_terminal_cleanup_hooks_fired();
        increment_terminal_latch_close_requests_by_cleanup();
        let after: serde_json::Value = serde_json::from_str(&last_job_diagnostics_json()).unwrap();
        assert_eq!(after["processLifecycle"]["terminalCleanupHooksFired"], 1);
        assert_eq!(
            after["processLifecycle"]["terminalLatchCloseRequestsByCleanup"],
            1
        );
    }
}
