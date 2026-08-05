use std::io::{self, Read};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut input = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut input) {
        eprintln!("parity desktop request adapter: failed to read stdin: {error}");
        return ExitCode::FAILURE;
    }
    match polygon_nesting_napi::compat::adapt_desktop_request_to_engine_json(&input) {
        Ok(request) => {
            println!("{request}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("parity desktop request adapter: {}", error.to_json());
            ExitCode::FAILURE
        }
    }
}
