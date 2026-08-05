use std::io::{self, Read};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut input = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut input) {
        eprintln!("parity engine outcome projector: failed to read stdin: {error}");
        return ExitCode::FAILURE;
    }
    #[derive(serde::Deserialize)]
    struct Envelope {
        outcome: polygon_nesting_protocol::EngineOutcome,
    }

    let outcome = match serde_json::from_str::<Envelope>(&input) {
        Ok(envelope) => envelope.outcome,
        Err(error) => {
            eprintln!("parity engine outcome projector: invalid neutral outcome: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "{}",
        polygon_nesting_napi::job::complete_engine_outcome(outcome)
    );
    ExitCode::SUCCESS
}
