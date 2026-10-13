//! Migration-only entry point; not installed with harness-gate.
use harness_gate_quality_core::{parse, replay};
use std::{env, fs, process::ExitCode};

fn run() -> Result<bool, Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: differential_replay INPUT.json OUTPUT.json".into());
    }
    let input = parse(&fs::read_to_string(&args[0])?)?;
    let result = replay::replay(&input)?;
    fs::write(
        &args[1],
        format!("{}\n", serde_json::to_string_pretty(&result)?),
    )?;
    Ok(result["mismatch_count"] == 0)
}
fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("non-authoritative replay error: {e}");
            ExitCode::from(2)
        }
    }
}
