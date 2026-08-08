use std::panic::{catch_unwind, AssertUnwindSafe, PanicHookInfo};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Args, Parser, Subcommand};
use polygon_nesting_cli::{
    run_with_deadline, write_malformed_invocation, write_malformed_output,
    write_signal_registration_failure, ExitStatus, RunPaths,
};
use polygon_nesting_core::{CancelReason, CancellationControl};

#[derive(Debug, Parser)]
#[command(version, about = "Deterministic polygon nesting engine")]
struct Cli {
    #[arg(long)]
    info: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    Run(RunArguments),
}

#[derive(Debug, Args)]
struct RunArguments {
    #[arg(long)]
    input: PathBuf,
    #[arg(long = "result-file")]
    output: PathBuf,
    #[arg(long)]
    events: Option<PathBuf>,
    #[arg(long)]
    deadline_ms: Option<f64>,
}

type PanicHook = Box<dyn Fn(&PanicHookInfo<'_>) + Send + Sync + 'static>;

#[allow(clippy::items_after_test_module)]
struct PanicHookGuard(Option<PanicHook>);

impl PanicHookGuard {
    fn silence() -> Self {
        let original = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        Self(Some(original))
    }
}

impl Drop for PanicHookGuard {
    fn drop(&mut self) {
        if let Some(original) = self.0.take() {
            std::panic::set_hook(original);
        }
    }
}

fn main() -> ExitCode {
    let status = {
        let _panic_hook = PanicHookGuard::silence();
        catch_unwind(AssertUnwindSafe(run_main)).unwrap_or(ExitStatus::InternalFailure)
    };
    if status != ExitStatus::Success {
        eprintln!("polygon-nesting: {}", status_message(status));
    }
    ExitCode::from(status.code() as u8)
}

fn run_main() -> ExitStatus {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(_) => return recover_malformed_invocation(),
    };
    run_parsed(cli, install_signal_handlers)
}

#[cfg(unix)]
fn install_signal_handlers(control: Arc<CancellationControl>) -> bool {
    use signal_hook_registry::{register, unregister};

    let interrupt_control = Arc::clone(&control);
    /* SAFETY: The handler performs only a lock-free atomic state transition. */
    let interrupt = unsafe {
        register(nix::libc::SIGINT, move || {
            interrupt_control.cancel(CancelReason::Cancelled);
        })
    };
    let Ok(interrupt) = interrupt else {
        return false;
    };

    /* SAFETY: The handler performs only a lock-free atomic state transition. */
    let termination = unsafe {
        register(nix::libc::SIGTERM, move || {
            control.cancel(CancelReason::Cancelled);
        })
    };
    if termination.is_err() {
        unregister(interrupt);
        return false;
    }
    true
}

#[cfg(not(unix))]
fn install_signal_handlers(control: Arc<CancellationControl>) -> bool {
    ctrlc::set_handler(move || {
        control.cancel(CancelReason::Cancelled);
    })
    .is_ok()
}

fn run_parsed(
    cli: Cli,
    install_signal_handler: impl FnOnce(Arc<CancellationControl>) -> bool,
) -> ExitStatus {
    if cli.info {
        let Some(Command::Run(arguments)) = cli.command else {
            return print_info();
        };
        return write_malformed_invocation(RunPaths {
            input: &arguments.input,
            output: &arguments.output,
            events: arguments.events.as_deref(),
        });
    }
    let Some(Command::Run(arguments)) = cli.command else {
        return ExitStatus::MalformedInput;
    };

    let control = Arc::new(CancellationControl::new());
    if !install_signal_handler(Arc::clone(&control)) {
        return write_signal_registration_failure(RunPaths {
            input: &arguments.input,
            output: &arguments.output,
            events: arguments.events.as_deref(),
        });
    }

    run_with_deadline(
        RunPaths {
            input: &arguments.input,
            output: &arguments.output,
            events: arguments.events.as_deref(),
        },
        &control,
        arguments.deadline_ms,
    )
}

fn recover_malformed_invocation() -> ExitStatus {
    let paths = recover_artifact_paths();
    if let Some((input, output, events)) = paths.unique_run_paths() {
        return write_malformed_invocation(RunPaths {
            input,
            output,
            events,
        });
    }
    paths
        .unique_output()
        .map(|output| write_malformed_output(output, &paths.inputs, &paths.events))
        .unwrap_or(ExitStatus::MalformedInput)
}

#[derive(Default)]
struct RecoveredArtifactPaths {
    inputs: Vec<PathBuf>,
    outputs: Vec<PathBuf>,
    events: Vec<PathBuf>,
}

impl RecoveredArtifactPaths {
    fn unique_run_paths(
        &self,
    ) -> Option<(&std::path::Path, &std::path::Path, Option<&std::path::Path>)> {
        let [input] = self.inputs.as_slice() else {
            return None;
        };
        let [output] = self.outputs.as_slice() else {
            return None;
        };
        let events = match self.events.as_slice() {
            [] => None,
            [events] => Some(events.as_path()),
            _ => return None,
        };
        Some((input, output, events))
    }

    fn unique_output(&self) -> Option<&std::path::Path> {
        let [output] = self.outputs.as_slice() else {
            return None;
        };
        Some(output)
    }
}

fn recover_artifact_paths() -> RecoveredArtifactPaths {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    recover_artifact_paths_from(&arguments)
}

fn recover_artifact_paths_from(arguments: &[std::ffi::OsString]) -> RecoveredArtifactPaths {
    let Some(run_position) = recover_run_position(arguments) else {
        return RecoveredArtifactPaths::default();
    };

    let mut paths = RecoveredArtifactPaths::default();
    let mut index = run_position + 1;
    while index < arguments.len() {
        let Some(argument) = arguments[index].to_str() else {
            index += 1;
            continue;
        };
        let (flag, value, consumed) = if let Some((flag, value)) = argument.split_once('=') {
            (flag, Some(PathBuf::from(value)), 1)
        } else if matches!(argument, "--input" | "--result-file" | "--events") {
            (
                argument,
                arguments.get(index + 1).cloned().map(PathBuf::from),
                2,
            )
        } else {
            index += 1;
            continue;
        };
        if let Some(value) = value {
            match flag {
                "--input" => paths.inputs.push(value),
                "--result-file" => paths.outputs.push(value),
                "--events" => paths.events.push(value),
                _ => {}
            }
        }
        index += consumed;
    }
    paths
}

fn recover_run_position(arguments: &[std::ffi::OsString]) -> Option<usize> {
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].to_string_lossy();
        if argument == "run" {
            let run_position = index;
            index += 1;
            while index < arguments.len() {
                let argument = arguments[index].to_string_lossy();
                if matches!(
                    argument.as_ref(),
                    "--input" | "--result-file" | "--events" | "--deadline-ms"
                ) {
                    index += 2;
                    continue;
                }
                if argument == "run" {
                    return None;
                }
                index += 1;
            }
            return Some(run_position);
        }
        if argument == "--info" || argument.contains('=') {
            index += 1;
        } else if argument.starts_with('-') {
            index += 2;
        } else {
            index += 1;
        }
    }
    None
}

fn print_info() -> ExitStatus {
    match serde_json::to_string(&polygon_nesting_core::engine_info()) {
        Ok(info) => {
            println!("{info}");
            ExitStatus::Success
        }
        Err(_) => ExitStatus::InternalFailure,
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use std::ffi::OsString;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{recover_artifact_paths_from, Cli, Command, PanicHookGuard, RunArguments};

    #[test]
    fn signal_registration_failure_writes_internal_envelope_before_reading_input() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("polygon-nesting-signal-main-{unique}"));
        std::fs::create_dir_all(&directory).expect("temporary directory should be created");
        let output = directory.join("result.json");
        let status = super::run_parsed(
            Cli {
                info: false,
                command: Some(Command::Run(RunArguments {
                    input: directory.join("does-not-exist.json"),
                    output: output.clone(),
                    events: None,
                    deadline_ms: None,
                })),
            },
            |_| false,
        );

        assert_eq!(status, polygon_nesting_cli::ExitStatus::InternalFailure);
        let outcome: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&output).expect("internal outcome should be written"),
        )
        .expect("internal outcome should be JSON");
        assert_eq!(outcome["outcome"]["error"]["category"], "internal_failure");
        std::fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }

    #[test]
    fn malformed_recovery_finds_one_run_after_top_level_flags() {
        let arguments = [
            OsString::from("--info"),
            OsString::from("run"),
            OsString::from("--input"),
            OsString::from("request.json"),
            OsString::from("--result-file"),
            OsString::from("result.json"),
            OsString::from("--deadline-ms"),
        ];

        let paths = recover_artifact_paths_from(&arguments);
        assert!(paths.unique_run_paths().is_some());
    }

    #[test]
    fn panic_hook_guard_restores_the_original_hook() {
        let original = std::panic::take_hook();
        let observed = Arc::new(AtomicBool::new(false));
        let observed_by_hook = Arc::clone(&observed);
        std::panic::set_hook(Box::new(move |_| {
            observed_by_hook.store(true, Ordering::SeqCst);
        }));

        {
            let _guard = PanicHookGuard::silence();
            assert!(catch_unwind(AssertUnwindSafe(|| panic!("silenced"))).is_err());
            assert!(!observed.load(Ordering::SeqCst));
        }
        assert!(catch_unwind(AssertUnwindSafe(|| panic!("restored"))).is_err());
        assert!(observed.load(Ordering::SeqCst));
        std::panic::set_hook(original);
    }
}

fn status_message(status: ExitStatus) -> &'static str {
    match status {
        ExitStatus::Success => "success",
        ExitStatus::InternalFailure => "internal failure",
        ExitStatus::MalformedInput => "malformed input or invocation",
        ExitStatus::TypedDomainFailure => "typed domain failure",
        ExitStatus::CancellationOrDeadline => "cancelled or deadline exceeded",
        ExitStatus::WriteFailure => "output or event write failure",
    }
}
