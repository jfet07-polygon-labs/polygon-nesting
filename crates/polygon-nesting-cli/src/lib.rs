use std::fs::{self, File};
use std::io::{self, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use polygon_nesting_core::{CancellationControl, EngineEventSink};
use polygon_nesting_protocol::{
    decode_request, encode_event, encode_outcome, EngineError, EngineErrorCode, EngineOutcome,
    ExecutionDiagnostics, SequencedEngineEvent,
};

static TEMPORARY_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

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

        fs::metadata(first)
            .ok()
            .zip(fs::metadata(second).ok())
            .is_some_and(|(first, second)| {
                first.dev() == second.dev() && first.ino() == second.ino()
            })
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
    let outcome = match encode_outcome(&outcome) {
        Ok(outcome) => outcome,
        Err(_) => return ExitStatus::WriteFailure,
    };
    if write_atomically(output, &outcome).is_err() {
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
        if write_atomically(events_path, &encoded_events).is_err() {
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

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{write_atomically, TEMPORARY_FILE_COUNTER};

    #[test]
    fn atomic_writer_removes_its_owned_temporary_after_rename_failure() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let output = std::env::temp_dir().join(format!("polygon-nesting-atomic-output-{unique}"));
        fs::create_dir_all(&output).expect("directory output should be created");
        TEMPORARY_FILE_COUNTER.store(0, std::sync::atomic::Ordering::Relaxed);
        let temporary = output.parent().expect("output has parent").join(format!(
            ".{}.{}.0.tmp",
            output
                .file_name()
                .expect("output has a name")
                .to_string_lossy(),
            std::process::id()
        ));

        assert!(write_atomically(&output, b"outcome").is_err());
        assert!(
            !temporary.exists(),
            "owned temporary file should be removed"
        );
        fs::remove_dir_all(output).expect("output directory should be removed");
    }

    #[test]
    fn atomic_writer_does_not_follow_a_preexisting_temporary_symlink() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("polygon-nesting-atomic-{unique}"));
        fs::create_dir_all(&directory).expect("temporary directory should be created");
        let output = directory.join("result.json");
        let victim = directory.join("victim.txt");
        fs::write(&victim, b"victim").expect("victim should be written");
        TEMPORARY_FILE_COUNTER.store(0, std::sync::atomic::Ordering::Relaxed);
        let temporary = directory.join(format!(".result.json.{}.0.tmp", std::process::id()));
        symlink(&victim, &temporary).expect("temporary symlink should be created");

        write_atomically(&output, b"outcome").expect("atomic write should succeed");

        assert_eq!(
            fs::read(&victim).expect("victim should remain readable"),
            b"victim"
        );
        assert_eq!(
            fs::read(&output).expect("outcome should be written"),
            b"outcome"
        );
        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }
}

fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    const MAX_TEMPORARY_FILE_ATTEMPTS: u64 = 32;

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("output");
    let (temporary, mut file) = (0..MAX_TEMPORARY_FILE_ATTEMPTS)
        .find_map(|_| {
            let counter = TEMPORARY_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
            let temporary = parent.join(format!(
                ".{filename}.{}.{}.tmp",
                std::process::id(),
                counter
            ));
            match File::options()
                .write(true)
                .create_new(true)
                .open(&temporary)
            {
                Ok(file) => Some(Ok((temporary, file))),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => None,
                Err(error) => Some(Err(error)),
            }
        })
        .unwrap_or_else(|| {
            Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "could not create a unique temporary artifact",
            ))
        })?;

    let owned_metadata = file.metadata()?;
    let write_result = (|| {
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if let Err(error) = write_result {
        drop(file);
        remove_owned_temporary(&temporary, &owned_metadata);
        return Err(error);
    }
    Ok(())
}

fn remove_owned_temporary(path: &Path, owned_metadata: &fs::Metadata) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        if fs::symlink_metadata(path).is_ok_and(|metadata| {
            metadata.dev() == owned_metadata.dev() && metadata.ino() == owned_metadata.ino()
        }) {
            let _ = fs::remove_file(path);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = owned_metadata;
        let _ = fs::remove_file(path);
    }
}
