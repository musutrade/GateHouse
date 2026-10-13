use super::{detect, ScopeError, ScopeMode};
use crate::project::Project;
use serde::Serialize;
use std::hint::black_box;
use std::time::Instant;

#[derive(Debug, Serialize)]
pub(crate) struct ScopeBenchmark {
    pub mode: String,
    pub iterations: usize,
    pub paths: usize,
    pub cached_total_us: u128,
    pub uncached_total_us: u128,
    pub cached_per_iteration_us: f64,
    pub uncached_per_iteration_us: f64,
    pub speedup: Option<f64>,
    pub equivalent: bool,
}

/// Measure matcher classification with the project's compiled rules versus
/// compiling a fresh globset for every iteration. Git discovery is performed
/// once so the result isolates matcher work rather than process startup.
pub fn benchmark(
    project: &Project,
    mode: &ScopeMode,
    iterations: usize,
) -> std::result::Result<ScopeBenchmark, ScopeError> {
    let iterations = iterations.max(1);
    let detected = detect(project, mode)?;
    let paths = detected.changed_files;
    let expected = project
        .config
        .classify_paths_with(&project.scope_rules, &paths);
    let mut cached_fingerprint = 0usize;
    let cached_started = Instant::now();
    for _ in 0..iterations {
        let result = project
            .config
            .classify_paths_with(&project.scope_rules, &paths);
        cached_fingerprint = cached_fingerprint
            .wrapping_add(result.0.len())
            .wrapping_add(result.1.len());
        if result != expected {
            return Err(ScopeError::configuration(anyhow::anyhow!(
                "cached scope classification changed between iterations"
            )));
        }
        black_box(&result);
    }
    let cached_total = cached_started.elapsed().as_micros();

    let mut uncached_fingerprint = 0usize;
    let uncached_started = Instant::now();
    for _ in 0..iterations {
        let result = project
            .config
            .classify_paths(&paths)
            .map_err(ScopeError::configuration)?;
        uncached_fingerprint = uncached_fingerprint
            .wrapping_add(result.0.len())
            .wrapping_add(result.1.len());
        if result != expected {
            return Err(ScopeError::configuration(anyhow::anyhow!(
                "uncached scope classification differs from cached result"
            )));
        }
        black_box(&result);
    }
    let uncached_total = uncached_started.elapsed().as_micros();
    black_box((cached_fingerprint, uncached_fingerprint));

    let cached_per = cached_total as f64 / iterations as f64;
    let uncached_per = uncached_total as f64 / iterations as f64;
    Ok(ScopeBenchmark {
        mode: detected.mode,
        iterations,
        paths: paths.len(),
        cached_total_us: cached_total,
        uncached_total_us: uncached_total,
        cached_per_iteration_us: cached_per,
        uncached_per_iteration_us: uncached_per,
        speedup: (cached_per > 0.0).then_some(uncached_per / cached_per),
        equivalent: true,
    })
}
