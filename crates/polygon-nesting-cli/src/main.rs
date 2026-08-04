use clap::Parser;
use polygon_nesting_protocol::EngineInfo;

#[derive(Debug, Parser)]
#[command(version, about = "Deterministic polygon nesting engine")]
struct Args {}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _args = Args::parse();

    let info: EngineInfo = polygon_nesting_core::engine_info();
    println!("{}", serde_json::to_string(&info)?);
    Ok(())
}
