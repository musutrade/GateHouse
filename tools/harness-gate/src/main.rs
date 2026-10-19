mod app;
mod audit;
mod cli;
mod compat;
mod config;
mod doctor;
mod error;
mod failure;
mod findings;
mod hooks;
mod plugins;
mod preset;
mod process;
mod project;
mod recipes;
mod report_evidence;
mod report_history;
mod scope;
mod secrets;
mod service;
#[cfg(test)]
mod test_support;
mod ui;
mod utils;
mod verify;

use error::CodedError;
use std::process::ExitCode;

fn main() -> ExitCode {
    process::install_signal_handlers();
    match app::run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("{}", error.render_terminal());
            eprintln!("help: harness-gate explain {}", error.code());
            ExitCode::FAILURE
        }
    }
}
