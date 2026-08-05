use std::panic::{catch_unwind, AssertUnwindSafe, PanicHookInfo};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Args, Parser, Subcommand};
use polygon_nesting_cli::{
    run_with_deadline, write_malformed_invocation, write_malformed_output, ExitStatus, RunPaths,
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
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    events: Option<PathBuf>,
    #[arg(long)]
    deadline_ms: Option<f64>,
}

struct PanicHookGuard(Option<Box<dyn Fn(&PanicHookInfo<'_>) + Send + Sync + 'static>>);

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
    if cli.info {
        if cli.command.is_some() {
            return ExitStatus::MalformedInput;
        }
        return print_info();
    }
    let Some(Command::Run(arguments)) = cli.command else {
        return ExitStatus::MalformedInput;
    };

    let control = Arc::new(CancellationControl::new());
    let signal_control = Arc::clone(&control);
    if ctrlc::set_handler(move || {
        signal_control.cancel(CancelReason::Cancelled);
    })
    .is_err()
    {
        return ExitStatus::InternalFailure;
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
    if arguments.first().and_then(|argument| argument.to_str()) != Some("run") {
        return RecoveredArtifactPaths::default();
    }

    let mut paths = RecoveredArtifactPaths::default();
    let mut index = 1;
    while index < arguments.len() {
        let Some(argument) = arguments[index].to_str() else {
            index += 1;
            continue;
        };
        let (flag, value, consumed) = if let Some((flag, value)) = argument.split_once('=') {
            (flag, Some(PathBuf::from(value)), 1)
        } else if matches!(argument, "--input" | "--output" | "--events") {
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
                "--output" => paths.outputs.push(value),
                "--events" => paths.events.push(value),
                _ => {}
            }
        }
        index += consumed;
    }
    paths
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
mod tests {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use super::PanicHookGuard;

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
