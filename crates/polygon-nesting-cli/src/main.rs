use std::io::Read;
use std::panic::{catch_unwind, AssertUnwindSafe, PanicHookInfo};
use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;
use std::sync::Arc;

use clap::{Args, Parser, Subcommand, ValueEnum};
use polygon_nesting_cli::{
    artifacts_overlap_inputs, artifacts_within_directory, path_within_directory, paths_alias,
    run_with_deadline_observed, write_artifact_atomically, write_dxf_import_failure,
    write_malformed_invocation, write_malformed_output, write_polygon_import_failure,
    write_request_and_run_observed, write_signal_registration_failure, ExitStatus, RunCompletion,
    RunPaths,
};
use polygon_nesting_core::{CancelReason, CancellationControl};
use polygon_nesting_dxf::{discover_directory, import_files, ImportOptions};
use polygon_nesting_protocol::EngineProfile;

mod benchmark_report;
mod polygon_input;

use polygon_input::{import_polygon_json, PolygonImportOptions, MAX_POLYGON_INPUT_BYTES};

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
    RunDxf(RunDxfArguments),
    RunPolygons(RunPolygonArguments),
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
    #[arg(long = "report-file")]
    report: Option<PathBuf>,
    #[arg(long, value_parser = parse_percentage, requires = "report")]
    best_known_utilization_percent: Option<f64>,
}

#[derive(Debug, Args)]
struct RunDxfArguments {
    #[arg(long = "input-dir")]
    input_dir: PathBuf,
    #[arg(long, value_parser = SheetDimensions::from_str)]
    sheet: SheetDimensions,
    #[arg(long, default_value_t = 10)]
    padding: u64,
    #[arg(long, value_enum, default_value_t = ProfileArgument::Compact)]
    profile: ProfileArgument,
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    allow_mirror: bool,
    #[arg(long, default_value_t = 300_000)]
    timeout_ms: u64,
    #[arg(long = "request-file")]
    request_file: PathBuf,
    #[arg(long = "result-file")]
    output: PathBuf,
    #[arg(long)]
    events: Option<PathBuf>,
    #[arg(long)]
    deadline_ms: Option<f64>,
    #[arg(long = "report-file")]
    report: Option<PathBuf>,
    #[arg(long, value_parser = parse_percentage, requires = "report")]
    best_known_utilization_percent: Option<f64>,
}

#[derive(Debug, Args)]
struct RunPolygonArguments {
    #[arg(long = "polygons-file")]
    polygons_file: PathBuf,
    #[arg(long, value_parser = SheetDimensions::from_str)]
    sheet: SheetDimensions,
    #[arg(long, default_value_t = 10)]
    padding: u64,
    #[arg(long, value_enum, default_value_t = ProfileArgument::Compact)]
    profile: ProfileArgument,
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    allow_mirror: bool,
    #[arg(long, default_value_t = 300_000)]
    timeout_ms: u64,
    #[arg(long = "request-file")]
    request_file: PathBuf,
    #[arg(long = "result-file")]
    output: PathBuf,
    #[arg(long)]
    events: Option<PathBuf>,
    #[arg(long)]
    deadline_ms: Option<f64>,
    #[arg(long = "report-file")]
    report: Option<PathBuf>,
    #[arg(long, value_parser = parse_percentage, requires = "report")]
    best_known_utilization_percent: Option<f64>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ProfileArgument {
    Compact,
    CompactShortSide,
}

impl From<ProfileArgument> for EngineProfile {
    fn from(profile: ProfileArgument) -> Self {
        match profile {
            ProfileArgument::Compact => Self::Compact,
            ProfileArgument::CompactShortSide => Self::CompactShortSide,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SheetDimensions {
    width: f64,
    height: f64,
}

impl FromStr for SheetDimensions {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (width, height) = value
            .split_once(['x', 'X', '×'])
            .ok_or_else(|| "sheet must use WIDTHxHEIGHT, for example 2000x2700".to_owned())?;
        let width = width
            .parse::<f64>()
            .map_err(|_| "sheet width must be numeric".to_owned())?;
        let height = height
            .parse::<f64>()
            .map_err(|_| "sheet height must be numeric".to_owned())?;
        if !width.is_finite() || width <= 0.0 || !height.is_finite() || height <= 0.0 {
            return Err("sheet dimensions must be positive finite numbers".to_owned());
        }
        Ok(Self { width, height })
    }
}

fn parse_percentage(value: &str) -> Result<f64, String> {
    let value = value
        .parse::<f64>()
        .map_err(|_| "percentage must be numeric".to_owned())?;
    if !value.is_finite() || !(0.0..=100.0).contains(&value) {
        return Err("percentage must be a finite number from 0 through 100".to_owned());
    }
    Ok(value)
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
    if let Some(Command::RunPolygons(arguments)) = &cli.command {
        let paths = run_polygon_paths(arguments);
        if artifacts_overlap_inputs(&paths, std::slice::from_ref(&arguments.polygons_file)) {
            return ExitStatus::MalformedInput;
        }
    }
    let dxf_source_paths = match &cli.command {
        Some(Command::RunDxf(arguments)) => {
            let paths = run_dxf_paths(arguments);
            if artifacts_within_directory(&arguments.input_dir, &paths) {
                return ExitStatus::MalformedInput;
            }
            let source_paths = match discover_directory(&arguments.input_dir) {
                Ok(paths) => paths,
                Err(_) if cli.info => return write_malformed_invocation(paths),
                Err(error) => return write_dxf_import_failure(paths, error.to_string()),
            };
            if artifacts_overlap_inputs(&paths, &source_paths) {
                return ExitStatus::MalformedInput;
            }
            Some(source_paths)
        }
        _ => None,
    };

    if cli.info {
        return match cli.command {
            None => print_info(),
            Some(Command::Run(arguments)) => write_malformed_invocation(RunPaths {
                input: &arguments.input,
                output: &arguments.output,
                events: arguments.events.as_deref(),
                report: arguments.report.as_deref(),
            }),
            Some(Command::RunDxf(arguments)) => {
                write_malformed_invocation(run_dxf_paths(&arguments))
            }
            Some(Command::RunPolygons(arguments)) => {
                write_malformed_invocation(run_polygon_paths(&arguments))
            }
        };
    }
    let Some(command) = cli.command else {
        return ExitStatus::MalformedInput;
    };

    let control = Arc::new(CancellationControl::new());
    if !install_signal_handler(Arc::clone(&control)) {
        return match command {
            Command::Run(arguments) => write_signal_registration_failure(RunPaths {
                input: &arguments.input,
                output: &arguments.output,
                events: arguments.events.as_deref(),
                report: arguments.report.as_deref(),
            }),
            Command::RunDxf(arguments) => {
                write_signal_registration_failure(run_dxf_paths(&arguments))
            }
            Command::RunPolygons(arguments) => {
                write_signal_registration_failure(run_polygon_paths(&arguments))
            }
        };
    }

    match command {
        Command::Run(arguments) => {
            let paths = RunPaths {
                input: &arguments.input,
                output: &arguments.output,
                events: arguments.events.as_deref(),
                report: arguments.report.as_deref(),
            };
            let completion = run_with_deadline_observed(paths, &control, arguments.deadline_ms);
            finish_benchmark_report(
                completion,
                arguments.report.as_deref(),
                arguments.best_known_utilization_percent,
            )
        }
        Command::RunDxf(arguments) => run_dxf(
            arguments,
            dxf_source_paths.expect("run-dxf source paths were prepared"),
            &control,
        ),
        Command::RunPolygons(arguments) => run_polygons(arguments, &control),
    }
}

fn run_dxf_paths(arguments: &RunDxfArguments) -> RunPaths<'_> {
    RunPaths {
        input: &arguments.request_file,
        output: &arguments.output,
        events: arguments.events.as_deref(),
        report: arguments.report.as_deref(),
    }
}

fn run_dxf(
    arguments: RunDxfArguments,
    source_paths: Vec<PathBuf>,
    control: &CancellationControl,
) -> ExitStatus {
    let paths = run_dxf_paths(&arguments);
    let request = match import_files(
        &source_paths,
        &ImportOptions {
            sheet_width: arguments.sheet.width,
            sheet_height: arguments.sheet.height,
            padding: arguments.padding,
            profile: arguments.profile.into(),
            allow_mirror: arguments.allow_mirror,
            timeout_ms: arguments.timeout_ms as f64,
        },
    ) {
        Ok(request) => request,
        Err(error) => return write_dxf_import_failure(paths, error.to_string()),
    };
    let completion =
        write_request_and_run_observed(&request, paths, control, arguments.deadline_ms);
    finish_benchmark_report(
        completion,
        arguments.report.as_deref(),
        arguments.best_known_utilization_percent,
    )
}

fn run_polygon_paths(arguments: &RunPolygonArguments) -> RunPaths<'_> {
    RunPaths {
        input: &arguments.request_file,
        output: &arguments.output,
        events: arguments.events.as_deref(),
        report: arguments.report.as_deref(),
    }
}

fn run_polygons(arguments: RunPolygonArguments, control: &CancellationControl) -> ExitStatus {
    let paths = run_polygon_paths(&arguments);
    if artifacts_overlap_inputs(&paths, std::slice::from_ref(&arguments.polygons_file)) {
        return ExitStatus::MalformedInput;
    }
    let input = match read_polygon_input(&arguments.polygons_file) {
        Ok(input) => input,
        Err(error) => {
            return write_polygon_import_failure(
                paths,
                format!(
                    "{}: polygon input file could not be read: {error}",
                    arguments.polygons_file.display()
                ),
            );
        }
    };
    let request = match import_polygon_json(
        &input,
        &PolygonImportOptions {
            sheet_width: arguments.sheet.width,
            sheet_height: arguments.sheet.height,
            padding: arguments.padding,
            profile: arguments.profile.into(),
            allow_mirror: arguments.allow_mirror,
            timeout_ms: arguments.timeout_ms as f64,
        },
    ) {
        Ok(request) => request,
        Err(error) => return write_polygon_import_failure(paths, error.to_string()),
    };
    let completion =
        write_request_and_run_observed(&request, paths, control, arguments.deadline_ms);
    finish_benchmark_report(
        completion,
        arguments.report.as_deref(),
        arguments.best_known_utilization_percent,
    )
}

fn finish_benchmark_report(
    completion: RunCompletion,
    report_path: Option<&std::path::Path>,
    best_known_utilization_percent: Option<f64>,
) -> ExitStatus {
    let status = completion.status;
    let Some(report_path) = report_path else {
        return status;
    };
    let Some(completed) = completion.completed else {
        return status;
    };
    let report = benchmark_report::build_benchmark_report(
        &completed.request,
        &completed.outcome,
        best_known_utilization_percent,
    );
    let Ok(bytes) = serde_json::to_vec(&report) else {
        return ExitStatus::WriteFailure;
    };
    if write_artifact_atomically(report_path, &bytes).is_err() {
        return ExitStatus::WriteFailure;
    }
    status
}

fn read_polygon_input(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let mut input = Vec::new();
    file.take(MAX_POLYGON_INPUT_BYTES + 1)
        .read_to_end(&mut input)?;
    if input.len() as u64 > MAX_POLYGON_INPUT_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("polygon input must not exceed {MAX_POLYGON_INPUT_BYTES} bytes"),
        ));
    }
    Ok(input)
}

fn recover_malformed_invocation() -> ExitStatus {
    let paths = recover_artifact_paths();
    if paths.overlaps_dxf_sources() || paths.overlaps_polygon_sources() {
        return ExitStatus::MalformedInput;
    }
    if let Some((input, output, events, report)) = paths.unique_run_paths() {
        return write_malformed_invocation(RunPaths {
            input,
            output,
            events,
            report,
        });
    }
    paths
        .unique_output()
        .map(|output| {
            let other_artifacts = paths
                .events
                .iter()
                .chain(&paths.reports)
                .cloned()
                .collect::<Vec<_>>();
            write_malformed_output(output, &paths.inputs, &other_artifacts)
        })
        .unwrap_or(ExitStatus::MalformedInput)
}

#[derive(Default)]
struct RecoveredArtifactPaths {
    inputs: Vec<PathBuf>,
    outputs: Vec<PathBuf>,
    events: Vec<PathBuf>,
    reports: Vec<PathBuf>,
    dxf_directories: Vec<PathBuf>,
    polygon_sources: Vec<PathBuf>,
}

impl RecoveredArtifactPaths {
    fn unique_run_paths(
        &self,
    ) -> Option<(
        &std::path::Path,
        &std::path::Path,
        Option<&std::path::Path>,
        Option<&std::path::Path>,
    )> {
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
        let report = match self.reports.as_slice() {
            [] => None,
            [report] => Some(report.as_path()),
            _ => return None,
        };
        Some((input, output, events, report))
    }

    fn unique_output(&self) -> Option<&std::path::Path> {
        let [output] = self.outputs.as_slice() else {
            return None;
        };
        Some(output)
    }

    fn overlaps_dxf_sources(&self) -> bool {
        let artifacts = self
            .inputs
            .iter()
            .chain(&self.outputs)
            .chain(&self.events)
            .chain(&self.reports)
            .collect::<Vec<_>>();
        self.dxf_directories.iter().any(|directory| {
            if artifacts
                .iter()
                .any(|artifact| path_within_directory(directory, artifact))
            {
                return true;
            }
            match discover_directory(directory) {
                Ok(sources) => artifacts
                    .iter()
                    .any(|artifact| sources.iter().any(|source| paths_alias(artifact, source))),
                Err(_) => directory.exists(),
            }
        })
    }

    fn overlaps_polygon_sources(&self) -> bool {
        let artifacts = self
            .inputs
            .iter()
            .chain(&self.outputs)
            .chain(&self.events)
            .chain(&self.reports);
        artifacts.into_iter().any(|artifact| {
            self.polygon_sources
                .iter()
                .any(|source| paths_alias(artifact, source))
        })
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
        let argument = &arguments[index];
        let (flag, value, consumed) = if let Some((flag, value)) = inline_path_argument(argument) {
            (flag, Some(value), 1)
        } else if matches!(
            argument.to_str(),
            Some(
                "--input"
                    | "--input-dir"
                    | "--polygons-file"
                    | "--request-file"
                    | "--result-file"
                    | "--events"
                    | "--report-file"
            )
        ) {
            (
                argument
                    .to_str()
                    .expect("matched path option must be valid UTF-8"),
                arguments.get(index + 1).cloned().map(PathBuf::from),
                2,
            )
        } else {
            index += 1;
            continue;
        };
        if let Some(value) = value {
            match flag {
                "--input" | "--request-file" => paths.inputs.push(value),
                "--input-dir" => paths.dxf_directories.push(value),
                "--polygons-file" => paths.polygon_sources.push(value),
                "--result-file" => paths.outputs.push(value),
                "--events" => paths.events.push(value),
                "--report-file" => paths.reports.push(value),
                _ => {}
            }
        }
        index += consumed;
    }
    paths
}

fn inline_path_argument(argument: &std::ffi::OsStr) -> Option<(&'static str, PathBuf)> {
    const FLAGS: [&str; 7] = [
        "--input",
        "--input-dir",
        "--polygons-file",
        "--request-file",
        "--result-file",
        "--events",
        "--report-file",
    ];
    FLAGS.into_iter().find_map(|flag| {
        strip_os_prefix(argument, &format!("{flag}=")).map(|value| (flag, PathBuf::from(value)))
    })
}

#[cfg(unix)]
fn strip_os_prefix(value: &std::ffi::OsStr, prefix: &str) -> Option<std::ffi::OsString> {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    value
        .as_bytes()
        .strip_prefix(prefix.as_bytes())
        .map(|suffix| std::ffi::OsString::from_vec(suffix.to_vec()))
}

#[cfg(windows)]
fn strip_os_prefix(value: &std::ffi::OsStr, prefix: &str) -> Option<std::ffi::OsString> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    let value = value.encode_wide().collect::<Vec<_>>();
    let prefix = prefix.encode_utf16().collect::<Vec<_>>();
    value
        .strip_prefix(prefix.as_slice())
        .map(std::ffi::OsString::from_wide)
}

#[cfg(not(any(unix, windows)))]
fn strip_os_prefix(value: &std::ffi::OsStr, prefix: &str) -> Option<std::ffi::OsString> {
    value
        .to_str()
        .and_then(|value| value.strip_prefix(prefix))
        .map(std::ffi::OsString::from)
}

fn recover_run_position(arguments: &[std::ffi::OsString]) -> Option<usize> {
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].to_string_lossy();
        if matches!(argument.as_ref(), "run" | "run-dxf" | "run-polygons") {
            let command = argument.into_owned();
            let run_position = index;
            index += 1;
            while index < arguments.len() {
                let argument = arguments[index].to_string_lossy();
                if matches!(
                    argument.as_ref(),
                    "--input"
                        | "--input-dir"
                        | "--polygons-file"
                        | "--sheet"
                        | "--padding"
                        | "--profile"
                        | "--allow-mirror"
                        | "--timeout-ms"
                        | "--request-file"
                        | "--result-file"
                        | "--events"
                        | "--deadline-ms"
                        | "--report-file"
                        | "--best-known-utilization-percent"
                ) {
                    index += 2;
                    continue;
                }
                if argument == command {
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
                    report: None,
                    best_known_utilization_percent: None,
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

    #[cfg(unix)]
    #[test]
    fn malformed_recovery_preserves_a_non_utf8_inline_input_directory() {
        use std::os::unix::ffi::OsStringExt;

        let inline_directory = OsString::from_vec(b"--input-dir=/tmp/dxfs-\x80".to_vec());
        let expected_directory = OsString::from_vec(b"/tmp/dxfs-\x80".to_vec());
        let arguments = [
            OsString::from("run-dxf"),
            inline_directory,
            OsString::from("--result-file"),
            OsString::from("result.json"),
            OsString::from("--padding"),
        ];

        let paths = recover_artifact_paths_from(&arguments);

        assert_eq!(
            paths.dxf_directories,
            vec![std::path::PathBuf::from(expected_directory)]
        );
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
        ExitStatus::WriteFailure => "artifact write failure",
    }
}
