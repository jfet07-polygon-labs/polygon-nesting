use std::fs::{self, File};
use std::io::{self, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use nix::fcntl::{openat, renameat, OFlag};
#[cfg(unix)]
use nix::sys::stat::{fchmod, fstat, mkdirat, Mode, SFlag};
#[cfg(unix)]
use nix::unistd::{fsync, geteuid, unlinkat, UnlinkatFlags};
#[cfg(unix)]
use std::os::fd::AsFd;

use polygon_nesting_core::{CancellationControl, EngineEventSink};
use polygon_nesting_protocol::{
    decode_request, encode_event, encode_outcome, EngineError, EngineErrorCode, EngineOutcome,
    ExecutionDiagnostics, SequencedEngineEvent,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitStatus {
    Success = 0,
    InternalFailure = 1,
    MalformedInput = 2,
    TypedDomainFailure = 3,
    CancellationOrDeadline = 4,
    WriteFailure = 5,
}

impl ExitStatus {
    pub fn code(self) -> i32 {
        self as i32
    }
}

#[derive(Default)]
struct BufferedEventSink {
    events: Vec<SequencedEngineEvent>,
}

impl EngineEventSink for BufferedEventSink {
    fn emit(&mut self, event: SequencedEngineEvent) {
        self.events.push(event);
    }
}

pub struct RunPaths<'a> {
    pub input: &'a Path,
    pub output: &'a Path,
    pub events: Option<&'a Path>,
}

pub fn run(paths: RunPaths<'_>, control: &CancellationControl) -> ExitStatus {
    run_with_deadline(paths, control, None)
}

pub fn write_malformed_output(
    output: &Path,
    input_candidates: &[PathBuf],
    events_candidates: &[PathBuf],
) -> ExitStatus {
    if input_candidates
        .iter()
        .chain(events_candidates)
        .any(|candidate| paths_alias(output, candidate))
    {
        return ExitStatus::MalformedInput;
    }
    finish_outcome(
        output,
        None,
        malformed_request_outcome(),
        Vec::new(),
        ExitStatus::MalformedInput,
    )
}

pub fn write_malformed_invocation(paths: RunPaths<'_>) -> ExitStatus {
    if paths_overlap(&paths) {
        return ExitStatus::MalformedInput;
    }
    finish_outcome(
        paths.output,
        paths.events,
        malformed_request_outcome(),
        Vec::new(),
        ExitStatus::MalformedInput,
    )
}

pub fn write_signal_registration_failure(paths: RunPaths<'_>) -> ExitStatus {
    if paths_overlap(&paths) {
        return ExitStatus::InternalFailure;
    }
    finish_outcome(
        paths.output,
        paths.events,
        EngineOutcome::Failure {
            error: EngineError::new(
                EngineErrorCode::InternalFailure,
                "register-signal-handler",
                "signal handler could not be registered",
            ),
            diagnostics: ExecutionDiagnostics::default(),
        },
        Vec::new(),
        ExitStatus::InternalFailure,
    )
}

pub fn run_with_deadline(
    paths: RunPaths<'_>,
    control: &CancellationControl,
    deadline_ms: Option<f64>,
) -> ExitStatus {
    if paths_overlap(&paths) {
        return ExitStatus::MalformedInput;
    }

    let mut request = match fs::read(paths.input)
        .ok()
        .and_then(|input| decode_request(input).ok())
    {
        Some(request) => request,
        None => {
            return finish_outcome(
                paths.output,
                paths.events,
                malformed_request_outcome(),
                Vec::new(),
                ExitStatus::MalformedInput,
            );
        }
    };

    if let Some(deadline_ms) = deadline_ms {
        if !deadline_ms.is_finite() || deadline_ms <= 0.0 {
            return finish_outcome(
                paths.output,
                paths.events,
                malformed_deadline_outcome(),
                Vec::new(),
                ExitStatus::MalformedInput,
            );
        }
        request.timeout_ms = request.timeout_ms.min(deadline_ms);
    }

    let mut events = BufferedEventSink::default();
    let outcome = match catch_unwind(AssertUnwindSafe(|| {
        #[cfg(debug_assertions)]
        if std::env::var_os("POLYGON_NESTING_TEST_PANIC").is_some() {
            panic!("CLI test panic injection");
        }
        polygon_nesting_core::run(&request, control, &mut events)
    })) {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(_)) | Err(_) => internal_failure_outcome(),
    };
    let status = outcome_exit_status(&outcome);
    finish_outcome(paths.output, paths.events, outcome, events.events, status)
}

fn paths_overlap(paths: &RunPaths<'_>) -> bool {
    paths_alias(paths.input, paths.output)
        || paths.events.is_some_and(|events| {
            paths_alias(paths.input, events) || paths_alias(paths.output, events)
        })
}

fn paths_alias(first: &Path, second: &Path) -> bool {
    if path_identity(first) == path_identity(second) {
        return true;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        return fs::metadata(first)
            .ok()
            .zip(fs::metadata(second).ok())
            .is_some_and(|(first, second)| {
                first.dev() == second.dev() && first.ino() == second.ino()
            });
    }
    #[cfg(not(unix))]
    false
}

fn path_identity(path: &Path) -> PathBuf {
    let mut prefix = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    let mut unresolved = Vec::new();

    loop {
        if let Ok(resolved_prefix) = fs::canonicalize(&prefix) {
            return unresolved
                .iter()
                .rev()
                .fold(resolved_prefix, |path, component| path.join(component));
        }
        let Some(component) = prefix.file_name().map(|component| component.to_owned()) else {
            return prefix;
        };
        unresolved.push(component);
        if !prefix.pop() {
            return prefix;
        }
    }
}

fn finish_outcome(
    output: &Path,
    events_path: Option<&Path>,
    outcome: EngineOutcome,
    events: Vec<SequencedEngineEvent>,
    status: ExitStatus,
) -> ExitStatus {
    finish_outcome_with_writer(
        output,
        events_path,
        outcome,
        events,
        status,
        write_atomically,
    )
}

fn finish_outcome_with_writer(
    output: &Path,
    events_path: Option<&Path>,
    outcome: EngineOutcome,
    events: Vec<SequencedEngineEvent>,
    status: ExitStatus,
    mut write: impl FnMut(&Path, &[u8]) -> io::Result<()>,
) -> ExitStatus {
    let outcome = match encode_outcome(&outcome) {
        Ok(outcome) => outcome,
        Err(_) => return ExitStatus::WriteFailure,
    };
    if write(output, &outcome).is_err() {
        return ExitStatus::WriteFailure;
    }
    if let Some(events_path) = events_path {
        let mut encoded_events = Vec::new();
        for event in events {
            let event = match encode_event(&event) {
                Ok(event) => event,
                Err(_) => return ExitStatus::WriteFailure,
            };
            encoded_events.extend_from_slice(&event);
            encoded_events.push(b'\n');
        }
        if write(events_path, &encoded_events).is_err() {
            if let Ok(outcome) = encode_outcome(&event_write_failure_outcome()) {
                let _ = write(output, &outcome);
            }
            return ExitStatus::WriteFailure;
        }
    }
    status
}

fn outcome_exit_status(outcome: &EngineOutcome) -> ExitStatus {
    match outcome {
        EngineOutcome::Success { .. } => ExitStatus::Success,
        EngineOutcome::ArchiveIneligible { .. } => ExitStatus::TypedDomainFailure,
        EngineOutcome::Failure { error, .. } => match error.category {
            EngineErrorCode::Cancelled | EngineErrorCode::DeadlineExceeded => {
                ExitStatus::CancellationOrDeadline
            }
            EngineErrorCode::InternalFailure => ExitStatus::InternalFailure,
            EngineErrorCode::IoFailure => ExitStatus::WriteFailure,
            _ => ExitStatus::TypedDomainFailure,
        },
    }
}

fn malformed_request_outcome() -> EngineOutcome {
    EngineOutcome::Failure {
        error: EngineError::new(
            EngineErrorCode::MalformedInput,
            "decode-request",
            "request could not be decoded",
        ),
        diagnostics: ExecutionDiagnostics::default(),
    }
}

fn malformed_deadline_outcome() -> EngineOutcome {
    EngineOutcome::Failure {
        error: EngineError::new(
            EngineErrorCode::MalformedInput,
            "parse-deadline",
            "deadline must be a positive finite number of milliseconds",
        ),
        diagnostics: ExecutionDiagnostics::default(),
    }
}

fn internal_failure_outcome() -> EngineOutcome {
    EngineOutcome::Failure {
        error: EngineError::new(
            EngineErrorCode::InternalFailure,
            "cli-run",
            "polygon nesting execution failed internally",
        ),
        diagnostics: ExecutionDiagnostics::default(),
    }
}

fn event_write_failure_outcome() -> EngineOutcome {
    EngineOutcome::Failure {
        error: EngineError::new(
            EngineErrorCode::IoFailure,
            "write-events",
            "event artifact could not be written",
        ),
        diagnostics: ExecutionDiagnostics::default(),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use nix::fcntl::{openat, OFlag};
    use nix::sys::stat::Mode;
    use std::fs::{self, File};
    use std::os::fd::AsFd;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        finish_outcome_with_writer, malformed_request_outcome, write_atomically, ExitStatus,
    };

    #[test]
    fn signal_registration_failure_writes_a_sanitized_internal_failure() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("polygon-nesting-signal-{unique}"));
        fs::create_dir_all(&directory).expect("temporary directory should be created");
        let input = directory.join("request.json");
        let output = directory.join("result.json");

        let status = super::write_signal_registration_failure(super::RunPaths {
            input: &input,
            output: &output,
            events: None,
        });

        assert_eq!(status, ExitStatus::InternalFailure);
        let outcome: serde_json::Value =
            serde_json::from_slice(&fs::read(&output).expect("outcome should exist"))
                .expect("outcome should be JSON");
        assert_eq!(outcome["outcome"]["error"]["category"], "internal_failure");
        assert_eq!(
            outcome["outcome"]["error"]["operation"],
            "register-signal-handler"
        );
        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }

    #[test]
    fn event_write_failure_preserves_original_outcome_when_replacement_fails() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("polygon-nesting-events-{unique}"));
        fs::create_dir_all(&directory).expect("temporary directory should be created");
        let output = directory.join("result.json");
        let events = directory.join("events.ndjson");
        let mut writes = 0;

        let status = finish_outcome_with_writer(
            &output,
            Some(&events),
            malformed_request_outcome(),
            Vec::new(),
            ExitStatus::MalformedInput,
            |path, bytes| {
                writes += 1;
                if path == events || writes == 3 {
                    return Err(std::io::Error::other("write failed"));
                }
                fs::write(path, bytes)
            },
        );

        assert_eq!(status, ExitStatus::WriteFailure);
        assert_eq!(writes, 3, "failure replacement should be attempted");
        let original: serde_json::Value =
            serde_json::from_slice(&fs::read(&output).expect("original outcome should remain"))
                .expect("original outcome should be JSON");
        assert_eq!(original["outcome"]["error"]["category"], "malformed_input");
        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }

    #[test]
    fn atomic_writer_uses_a_private_staging_directory_and_cleans_it_after_rename() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("polygon-nesting-atomic-{unique}"));
        fs::create_dir_all(&directory).expect("temporary directory should be created");
        let output = directory.join("result.json");

        write_atomically(&output, b"outcome").expect("atomic write should succeed");

        assert_eq!(
            fs::read(&output).expect("outcome should be readable"),
            b"outcome"
        );
        let staging_root = directory.join(".polygon-nesting-staging");
        assert!(
            staging_root.is_dir(),
            "persistent staging root should exist"
        );
        assert!(
            fs::read_dir(&staging_root)
                .expect("staging root should be readable")
                .next()
                .is_none(),
            "per-job staging directory should be removed"
        );
        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }

    #[test]
    fn staging_cleanup_preserves_a_replacement_directory() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("polygon-nesting-cleanup-{unique}"));
        fs::create_dir_all(&directory).expect("temporary directory should be created");
        let output = directory.join("result.json");
        let replacement = directory.join("replacement-root");
        let mut replacement_root_path = None;

        assert!(
            super::write_atomically_with_staging_hook(&output, b"outcome", |staging| {
                let root = staging.parent().expect("job directory should have a root");
                fs::rename(root, &replacement).expect("trusted root should be moved");
                fs::create_dir(root).expect("replacement root should be created");
                replacement_root_path = Some(root.to_owned());
                Err(std::io::Error::other("rename interrupted"))
            })
            .is_err()
        );

        assert!(
            replacement_root_path
                .expect("replacement root path should be captured")
                .exists(),
            "public root replacement must remain"
        );
        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }

    #[test]
    fn public_staging_root_swap_preserves_replacement_and_uses_opened_root() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "polygon-nesting-root-swap-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("temporary directory should be created");
        let output = directory.join("result.json");
        let root = directory.join(".polygon-nesting-staging");
        let moved_root = directory.join("retained-root");

        super::write_atomically_with_staging_hook(&output, b"outcome", |staging| {
            assert_eq!(
                staging.parent(),
                Some(root.as_path()),
                "the job must be created under the public persistent root"
            );
            fs::rename(&root, &moved_root).expect("opened root should be moved aside");
            fs::create_dir(&root).expect("public root replacement should be created");
            fs::write(root.join("replacement"), b"preserved")
                .expect("replacement marker should be written");
            Ok(())
        })
        .expect("retained descriptors should complete the write");

        assert_eq!(
            fs::read(&output).expect("outcome should be readable"),
            b"outcome"
        );
        assert_eq!(
            fs::read(root.join("replacement")).expect("replacement should remain"),
            b"preserved"
        );
        assert!(
            fs::read_dir(&moved_root)
                .expect("moved opened root should be readable")
                .next()
                .is_none(),
            "job cleanup must target the opened root"
        );
        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }

    #[test]
    fn staging_root_survives_after_per_job_cleanup() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "polygon-nesting-root-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("temporary directory should be created");
        let output = directory.join("result.json");

        write_atomically(&output, b"outcome").expect("atomic write should succeed");

        let staging_root = directory.join(".polygon-nesting-staging");
        assert!(
            staging_root.is_dir(),
            "persistent staging root should exist"
        );
        assert!(
            fs::read_dir(&staging_root)
                .expect("staging root should be readable")
                .next()
                .is_none(),
            "per-job staging directory should be removed"
        );
        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }

    #[test]
    fn sequential_writers_leave_one_empty_persistent_root() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "polygon-nesting-sequential-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("temporary directory should be created");

        for index in 0..3 {
            write_atomically(&directory.join(format!("result-{index}.json")), b"outcome")
                .expect("writer should reuse the persistent root");
        }

        let roots = fs::read_dir(&directory)
            .expect("output directory should remain readable")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name() == ".polygon-nesting-staging")
            .collect::<Vec<_>>();
        assert_eq!(roots.len(), 1, "only one persistent root should remain");
        assert!(
            fs::read_dir(roots[0].path())
                .expect("staging root should be readable")
                .next()
                .is_none(),
            "per-job directories should be removed"
        );
        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }

    #[test]
    fn world_writable_parent_uses_the_private_persistent_root() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "polygon-nesting-parent-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("temporary directory should be created");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o777))
            .expect("parent permissions should be changed");

        write_atomically(&directory.join("result.json"), b"outcome")
            .expect("private persistent root should protect staging artifacts");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .expect("parent permissions should be restored");
        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }

    #[test]
    fn concurrent_writers_share_the_private_persistent_root() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "polygon-nesting-concurrent-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("temporary directory should be created");
        let workers = 8;
        let barrier = Arc::new(Barrier::new(workers));
        let handles = (0..workers)
            .map(|index| {
                let directory = directory.clone();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    let output = directory.join(format!("result-{index}.json"));
                    write_atomically(&output, index.to_string().as_bytes())
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle
                .join()
                .expect("writer thread should not panic")
                .expect("writer should use the shared staging root");
        }
        let roots = fs::read_dir(&directory)
            .expect("output directory should remain readable")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name() == ".polygon-nesting-staging")
            .collect::<Vec<_>>();
        assert_eq!(roots.len(), 1, "only one persistent root should remain");
        assert!(
            fs::read_dir(roots[0].path())
                .expect("staging root should be readable")
                .next()
                .is_none(),
            "per-job directories should be removed"
        );
        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }

    #[test]
    fn job_directory_name_collision_retries_with_a_fresh_name() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "polygon-nesting-job-collision-{}-{unique}",
            std::process::id()
        ));
        let root = directory.join(".polygon-nesting-staging");
        fs::create_dir_all(&root).expect("staging root should be created");
        fs::create_dir(root.join("job-collision")).expect("collision directory should exist");
        let root_fd = File::open(&root).expect("staging root should open");
        let mut names = ["job-collision", "job-success"].into_iter();

        let (name, job) = super::create_staging_job_directory_with_name(&root_fd, 2, |_| {
            names
                .next()
                .expect("test should provide a staging job name")
                .to_owned()
        })
        .expect("writer should retry after a collision");

        assert_eq!(name, "job-success");
        drop(job);
        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }

    #[test]
    fn job_open_failure_removes_the_created_job_directory() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "polygon-nesting-job-open-failure-{}-{unique}",
            std::process::id()
        ));
        let root = directory.join(".polygon-nesting-staging");
        fs::create_dir_all(&root).expect("staging root should be created");
        let root_fd = File::open(&root).expect("staging root should open");

        assert!(super::create_staging_job_directory_with_name_and_opener(
            &root_fd,
            1,
            |_| "job-failure".to_owned(),
            |_, _| Err(std::io::Error::other("injected job open failure")),
        )
        .is_err());
        assert!(
            fs::read_dir(&root)
                .expect("staging root should be readable")
                .next()
                .is_none(),
            "failed job opening must not leave a job directory"
        );
        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }

    #[test]
    fn artifact_open_failure_removes_the_leaf_and_job_directory() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "polygon-nesting-artifact-open-failure-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("temporary directory should be created");
        let output = directory.join("result.json");

        assert!(
            super::write_atomically_with_staging_hook_and_artifact_opener(
                &output,
                b"outcome",
                |_| Ok(()),
                |job| {
                    let artifact = openat(
                        job.as_fd(),
                        Path::new("artifact"),
                        OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_NOFOLLOW,
                        Mode::from_bits_truncate(0o600),
                    )
                    .map(File::from)
                    .map_err(std::io::Error::from)?;
                    drop(artifact);
                    Err(std::io::Error::other("injected artifact open failure"))
                },
            )
            .is_err()
        );
        let root = directory.join(".polygon-nesting-staging");
        assert!(
            fs::read_dir(&root)
                .expect("staging root should be readable")
                .next()
                .is_none(),
            "failed artifact opening must not leave a leaf or job directory"
        );
        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }

    #[test]
    fn preexisting_private_staging_root_is_reused() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "polygon-nesting-root-reuse-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("temporary directory should be created");
        let root = directory.join(".polygon-nesting-staging");
        fs::create_dir(&root).expect("persistent staging root should be created");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .expect("staging root should be private");

        write_atomically(&directory.join("result.json"), b"outcome")
            .expect("writer should use the private persistent root");

        assert!(root.is_dir(), "persistent root should remain");
        assert!(
            fs::read_dir(&root)
                .expect("persistent root should be readable")
                .next()
                .is_none(),
            "per-job artifacts should be cleaned"
        );
        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }

    #[test]
    fn wrong_mode_staging_root_is_rejected() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "polygon-nesting-root-mode-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("temporary directory should be created");
        let root = directory.join(".polygon-nesting-staging");
        fs::create_dir(&root).expect("staging root should be created");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755))
            .expect("staging root mode should be changed");

        assert!(write_atomically(&directory.join("result.json"), b"outcome").is_err());
        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }

    #[test]
    fn symlinked_staging_root_is_rejected() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "polygon-nesting-root-symlink-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("temporary directory should be created");
        let target = directory.join("target");
        fs::create_dir(&target).expect("symlink target should be created");
        std::os::unix::fs::symlink(&target, directory.join("candidate"))
            .expect("staging root symlink should be created");

        fs::rename(
            directory.join("candidate"),
            directory.join(".polygon-nesting-staging"),
        )
        .expect("staging root symlink should be named canonically");
        assert!(write_atomically(&directory.join("result.json"), b"outcome").is_err());
        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }

    #[test]
    fn staging_root_ownership_uses_the_effective_user_identity() {
        assert!(super::staging_root_is_owned_by(41, 41));
        assert!(!super::staging_root_is_owned_by(41, 40));
    }

    #[test]
    fn staging_directory_is_private_on_unix() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("polygon-nesting-atomic-{unique}"));
        fs::create_dir_all(&directory).expect("temporary directory should be created");
        let output = directory.join("result.json");

        super::write_atomically_with_staging_hook(&output, b"outcome", |staging| {
            let root = staging.parent().expect("job directory should have a root");
            assert_eq!(
                fs::metadata(root)
                    .expect("staging root metadata should exist")
                    .permissions()
                    .mode()
                    & 0o7777,
                0o700
            );
            Ok(())
        })
        .expect("atomic write should succeed");
        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }
}

#[cfg(unix)]
fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_atomically_with_staging_hook(path, bytes, |_| Ok(()))
}

#[cfg(unix)]
fn write_atomically_with_staging_hook(
    path: &Path,
    bytes: &[u8],
    before_rename: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<()> {
    write_atomically_with_staging_hook_and_artifact_opener(
        path,
        bytes,
        before_rename,
        open_staging_artifact,
    )
}

#[cfg(unix)]
fn write_atomically_with_staging_hook_and_artifact_opener(
    path: &Path,
    bytes: &[u8],
    before_rename: impl FnOnce(&Path) -> io::Result<()>,
    open_artifact: impl FnOnce(&File) -> io::Result<File>,
) -> io::Result<()> {
    const STAGING_DIRECTORY: &str = ".polygon-nesting-staging";
    const STAGING_LEAF: &str = "artifact";
    const MAX_STAGING_DIRECTORY_ATTEMPTS: u64 = 32;

    let parent_path = path.parent().unwrap_or_else(|| Path::new("."));
    let output_leaf = path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "output path has no filename")
    })?;
    let parent = File::open(parent_path)?;
    let staging_root = open_staging_root(&parent, STAGING_DIRECTORY)?;
    let (job_name, staging) =
        create_staging_job_directory(&staging_root, MAX_STAGING_DIRECTORY_ATTEMPTS)?;

    let artifact = match open_artifact(&staging) {
        Ok(artifact) => artifact,
        Err(error) => {
            cleanup_staging_job(&staging_root, &job_name, staging, true);
            return Err(error);
        }
    };
    let result = (|| {
        let mut artifact = artifact;
        artifact.write_all(bytes)?;
        artifact.flush()?;
        fsync(artifact.as_fd()).map_err(io::Error::from)?;
        before_rename(&parent_path.join(STAGING_DIRECTORY).join(&job_name))?;
        renameat(
            staging.as_fd(),
            Path::new(STAGING_LEAF),
            parent.as_fd(),
            Path::new(output_leaf),
        )
        .map_err(io::Error::from)
    })();

    cleanup_staging_job(&staging_root, &job_name, staging, result.is_err());
    result
}

#[cfg(unix)]
fn cleanup_staging_job(root: &File, job_name: &str, job: File, remove_leaf: bool) {
    if remove_leaf {
        let _ = unlinkat(
            job.as_fd(),
            Path::new("artifact"),
            UnlinkatFlags::NoRemoveDir,
        );
    }
    drop(job);
    let _ = unlinkat(root.as_fd(), Path::new(job_name), UnlinkatFlags::RemoveDir);
}

#[cfg(unix)]
fn open_staging_artifact(staging: &File) -> io::Result<File> {
    openat(
        staging.as_fd(),
        Path::new("artifact"),
        OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_NOFOLLOW,
        Mode::from_bits_truncate(0o600),
    )
    .map(File::from)
    .map_err(io::Error::from)
}

#[cfg(unix)]
fn open_staging_root(parent: &File, name: &str) -> io::Result<File> {
    let created = match mkdirat(
        parent.as_fd(),
        Path::new(name),
        Mode::from_bits_truncate(0o700),
    ) {
        Ok(()) => true,
        Err(nix::errno::Errno::EEXIST) => false,
        Err(error) => return Err(io::Error::from(error)),
    };
    let root = openat(
        parent.as_fd(),
        Path::new(name),
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(io::Error::from)?;
    if created {
        fchmod(root.as_fd(), Mode::from_bits_truncate(0o700)).map_err(io::Error::from)?;
    }
    validate_staging_root(&root)?;
    Ok(root)
}

#[cfg(unix)]
fn staging_root_is_owned_by(owner_uid: u32, effective_uid: u32) -> bool {
    owner_uid == effective_uid
}

#[cfg(unix)]
fn validate_staging_root(root: &File) -> io::Result<()> {
    let metadata = fstat(root.as_fd()).map_err(io::Error::from)?;
    if SFlag::from_bits_truncate(metadata.st_mode) != SFlag::S_IFDIR {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "staging root is not a directory",
        ));
    }
    if !staging_root_is_owned_by(metadata.st_uid, geteuid().as_raw())
        || metadata.st_mode & 0o7777 != 0o700
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "staging root is not a private directory owned by this user",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn create_staging_job_directory(root: &File, attempts: u64) -> io::Result<(String, File)> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    create_staging_job_directory_with_name(root, attempts, |attempt| {
        format!("job-{}-{timestamp}-{attempt}", std::process::id())
    })
}

#[cfg(unix)]
fn create_staging_job_directory_with_name(
    root: &File,
    attempts: u64,
    next_name: impl FnMut(u64) -> String,
) -> io::Result<(String, File)> {
    create_staging_job_directory_with_name_and_opener(root, attempts, next_name, |root, name| {
        openat(
            root.as_fd(),
            Path::new(name),
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(io::Error::from)
    })
}

#[cfg(unix)]
fn create_staging_job_directory_with_name_and_opener(
    root: &File,
    attempts: u64,
    mut next_name: impl FnMut(u64) -> String,
    mut open_job: impl FnMut(&File, &str) -> io::Result<File>,
) -> io::Result<(String, File)> {
    for attempt in 0..attempts {
        let name = next_name(attempt);
        match mkdirat(
            root.as_fd(),
            Path::new(&name),
            Mode::from_bits_truncate(0o700),
        ) {
            Ok(()) => match open_job(root, &name) {
                Ok(job) => return Ok((name, job)),
                Err(error) => {
                    let _ = unlinkat(root.as_fd(), Path::new(&name), UnlinkatFlags::RemoveDir);
                    return Err(error);
                }
            },
            Err(nix::errno::Errno::EEXIST) => continue,
            Err(error) => return Err(io::Error::from(error)),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not create staging job directory",
    ))
}

#[cfg(not(unix))]
fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    const MAX_STAGING_DIRECTORY_ATTEMPTS: u64 = 32;

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let root = parent.join(".polygon-nesting-staging");
    fs::create_dir_all(&root)?;
    let job = (0..MAX_STAGING_DIRECTORY_ATTEMPTS)
        .find_map(|attempt| {
            let job = root.join(format!(
                "job-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
                attempt
            ));
            match fs::create_dir(&job) {
                Ok(()) => Some(Ok(job)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => None,
                Err(error) => Some(Err(error)),
            }
        })
        .unwrap_or_else(|| {
            Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "could not create staging job directory",
            ))
        })?;
    let artifact = job.join("artifact");
    let result = (|| {
        let mut artifact_file = File::options()
            .write(true)
            .create_new(true)
            .open(&artifact)?;
        artifact_file.write_all(bytes)?;
        artifact_file.flush()?;
        artifact_file.sync_all()?;
        fs::rename(&artifact, path)
    })();
    let _ = fs::remove_file(&artifact);
    let _ = fs::remove_dir(&job);
    result
}
