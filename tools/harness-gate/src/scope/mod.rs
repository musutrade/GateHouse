mod benchmark;
mod detection;
mod errors;
mod model;
mod report;
#[cfg(test)]
mod tests;

pub use benchmark::benchmark;
pub use detection::detect;
pub(crate) use detection::detect_with_access;
pub use errors::ScopeError;
pub use model::{ScopeMode, ScopeResult};
