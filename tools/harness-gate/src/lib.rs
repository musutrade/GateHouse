//! # Harness-Gate
//!
//! `harness-gate` is a configurable development workflow and architecture
//! guard command-line tool.
//!
//! The executable provides workflow commands. The `quality` library owns generic
//! evaluation of normalized evidence, independently of collector implementations.
//!
//! ## Install
//!
//! ```text
//! cargo install harness-gate
//! ```
//!
//! ## Documentation
//!
//! - [User guide](https://github.com/musutrade/Harness-Gate/blob/main/README.md)
//! - [Configuration reference](https://github.com/musutrade/Harness-Gate/blob/main/docs/configuration.md)
//! - [GitHub repository](https://github.com/musutrade/Harness-Gate)
//! - [Crates.io package](https://crates.io/crates/harness-gate)
//!
//! The binary's command reference is available with `harness-gate --help`.

#[path = "../quality-core/mod.rs"]
pub mod quality;
