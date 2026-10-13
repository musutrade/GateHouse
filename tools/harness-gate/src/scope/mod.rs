mod benchmark;
mod detection;
mod errors;
mod model;
mod report;
#[cfg(test)]
mod tests;

pub use benchmark::benchmark;
pub use detection::detect;
pub use errors::ScopeError;
pub use model::{ScopeMode, ScopeResult};
