use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use polygon_nesting_napi::events::EventFrame;
use polygon_nesting_protocol::{EngineOutcome, SequencedEngineEvent};

fn outcome_path() -> Result<PathBuf, String> {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--outcome")) {
        return Err("expected --outcome <neutral-outcome.json>".to_string());
    }
    arguments
        .next()
        .map(PathBuf::from)
        .filter(|_| arguments.next().is_none())
        .ok_or_else(|| "expected exactly one neutral outcome path".to_string())
}

fn main() -> ExitCode {
    let outcome_path = match outcome_path() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("parity engine event projector: {error}");
            return ExitCode::FAILURE;
        }
    };
    #[derive(serde::Deserialize)]
    struct Envelope {
        outcome: EngineOutcome,
    }

    let outcome: EngineOutcome = match std::fs::read_to_string(outcome_path)
        .ok()
        .and_then(|bytes| serde_json::from_str::<Envelope>(&bytes).ok())
    {
        Some(envelope) => envelope.outcome,
        None => {
            eprintln!("parity engine event projector: invalid neutral outcome");
            return ExitCode::FAILURE;
        }
    };
    let _desktop_result = polygon_nesting_napi::job::complete_engine_outcome(outcome);
    let mut input = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut input) {
        eprintln!("parity engine event projector: failed to read stdin: {error}");
        return ExitCode::FAILURE;
    }
    let mut events = Vec::new();
    for line in input.lines().filter(|line| !line.is_empty()) {
        let event: SequencedEngineEvent = match serde_json::from_str(line) {
            Ok(event) => event,
            Err(error) => {
                eprintln!("parity engine event projector: invalid neutral event: {error}");
                return ExitCode::FAILURE;
            }
        };
        events.push(event);
    }
    let terminal_ordinal = events
        .last()
        .map_or(0, |event| event.ordinal.saturating_add(1));
    for event in events {
        println!("{}", EventFrame::Core(event).json());
    }
    println!(
        "{}",
        EventFrame::Terminal {
            ordinal: terminal_ordinal
        }
        .json()
    );
    ExitCode::SUCCESS
}
