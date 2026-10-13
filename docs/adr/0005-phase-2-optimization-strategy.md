# ADR 0005: Phase 2 Optimization Strategy

## Status

**Accepted** - Implemented incrementally, merged to `main`, and verified by
cross-platform CI on 2026-08-27.

## Context

Phase 1 optimization successfully resolved all CI failures and established a clean baseline with zero warnings and full test coverage. However, several opportunities for improvement remain:

### Current State Analysis

**Performance Issues:**
- Test suite execution: 20-30 seconds
- Clean build time: 2-3 minutes
- Incremental build time: 30-45 seconds
- Release binary size: 6-8MB

**Code Quality Issues:**
- Generic error messages lacking context
- Some functions with high cyclomatic complexity (>15)
- Mixed concerns in verification workflow
- Limited error recovery guidance

**Impact on Users:**
- Slow local development feedback loop
- Large binary downloads
- Difficult to debug failures
- Hard to maintain and extend

### Why Now?

1. **Clean Foundation**: Phase 1 provides stable baseline for refactoring
2. **Development Velocity**: Current build/test times slow iteration
3. **User Experience**: Error messages need improvement
4. **Technical Debt**: Code complexity will make future features harder
5. **Binary Distribution**: Smaller binaries improve adoption

## Decision

We will implement **Phase 2: Performance and Code Quality Enhancement** focusing on:

## Implementation Notes

The implementation retained `anyhow` for internal context propagation and added
typed errors at public workflow boundaries. `AuditError`, `SecretsError`,
`ScopeError`, and `VerifyError` classify user-visible failures; `CliError`
renders them as stable `ERROR [E####]` output without changing CLI arguments or
TOML schemas. This gives callers a dependable failure category while preserving
the detailed file and command context from the existing implementation.

The scope and secrets modules now share NUL-delimited Git path and staged-file
handling, and audit/secrets/scope/verify share report-writing helpers. These
helpers are intentionally narrow: they remove duplication without introducing a
generic workflow abstraction.

Local verification is recorded in
[Phase 2 Results](../benchmarks/phase-2-results.md). PR #7 was merged to
`main` as `3e9c3bd`; its GitHub CI run passed build and test jobs on Ubuntu,
macOS, and Windows, plus format, Clippy, security audit, and code coverage.

### 1. Test Performance Optimization

**Decision**: Introduce shared test fixtures with workspace isolation

**Rationale**:
- Current approach creates ~50-60 temporary Git repositories per test run
- Each Git initialization takes 50-100ms
- Shared fixtures reduce setup overhead by 70-80%
- Maintains test isolation through workspace directories

**Trade-offs**:
- **Pro**: 60-70% faster test execution
- **Pro**: Reduced disk I/O and CI cache thrashing
- **Con**: More complex test infrastructure
- **Con**: Risk of test interdependencies

**Mitigation**: Incremental migration, thorough review of test isolation

### 2. Binary Size Optimization

**Decision**: Add size-optimized release profile and audit dependencies

**Rationale**:
- Default `opt-level = 3` optimizes for speed, not size
- `opt-level = "z"` + LTO can reduce binary by 20-30%
- Many dependencies include unused features
- Smaller binaries improve download speed and adoption

**Approach**:
```toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

**Trade-offs**:
- **Pro**: 20-30% smaller binary
- **Pro**: Faster downloads, better user experience
- **Con**: Slightly longer compilation time
- **Con**: May sacrifice some runtime performance

**Mitigation**: Benchmark runtime performance, keep separate profiles if needed

### 3. Compilation Time Reduction

**Decision**: Enable incremental compilation and optimize dev profile

**Rationale**:
- Development feedback loop is critical
- Dependencies don't change often, can be optimized once
- Dev builds don't need full optimization
- Incremental compilation is stable in modern Rust

**Approach**:
```toml
[profile.dev]
opt-level = 1
incremental = true

[profile.dev.package."*"]
opt-level = 2
```

**Trade-offs**:
- **Pro**: 30-40% faster incremental builds
- **Pro**: Better developer experience
- **Con**: Dev binaries slightly larger (not shipped)
- **Con**: First build still slow (acceptable)

**Mitigation**: None needed - pure win for development

### 4. Error Handling Enhancement

**Decision**: Add context to all errors and create domain-specific error types

**Rationale**:
- Current errors are generic: "Failed to load config"
- Missing context makes debugging difficult
- Users need guidance on how to fix errors
- `anyhow::Context` adds minimal overhead

**Approach**:
```rust
// Add context to all Result chains
load_config(path)
    .context(format!("Failed to load audit config from {}", path.display()))?;

// Create domain-specific errors for common cases
#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("Audit config not found at {path}\nRun 'harness-gate init' to create one")]
    ConfigNotFound { path: PathBuf },
}
```

**Trade-offs**:
- **Pro**: Much better user experience
- **Pro**: Easier debugging
- **Pro**: Self-documenting error handling
- **Con**: More verbose code
- **Con**: Small runtime overhead (negligible)

**Mitigation**: Use macros to reduce boilerplate if needed

### 5. Code Complexity Reduction

**Decision**: Refactor high-complexity functions using Extract Function pattern

**Rationale**:
- Functions with complexity >10 are hard to understand
- Mixed concerns make testing difficult
- Single Responsibility Principle improves maintainability
- Smaller functions easier to optimize

**Approach**:
- Identify functions with cyclomatic complexity >10
- Extract logical units into helper functions
- Use builder pattern for complex configurations
- Add unit tests for extracted functions

**Trade-offs**:
- **Pro**: Better maintainability
- **Pro**: Easier to add features
- **Pro**: Better test coverage
- **Con**: More functions to navigate
- **Con**: Risk of over-abstraction

**Mitigation**: Stop at single-level extraction, avoid deep call stacks

## Alternatives Considered

### Alternative 1: Async I/O for Test Parallelization

**Description**: Use `tokio` to run tests in parallel with async I/O

**Pros**:
- Potentially faster test execution
- Better resource utilization

**Cons**:
- Adds complex dependency (`tokio`)
- Tests don't need async - they're CPU/disk bound
- Shared fixtures achieve similar speedup with less complexity

**Decision**: **Rejected** - Shared fixtures simpler and sufficient

### Alternative 2: Aggressive Dependency Pruning

**Description**: Replace all heavy dependencies with minimal alternatives

**Pros**:
- Smallest possible binary
- Fastest compilation

**Cons**:
- High maintenance burden
- Lose ecosystem integration
- Risk of reinventing wheels poorly

**Decision**: **Rejected** - Audit first, prune only clear wins

### Alternative 3: Split Into Workspace

**Description**: Split into `harness-gate-core` and `harness-gate-cli` crates

**Pros**:
- Better compilation caching
- Clearer architecture boundaries

**Cons**:
- Adds workspace complexity
- Build system changes
- May not significantly improve compile times

**Decision**: **Deferred** - Try simpler optimizations first, revisit if needed

### Alternative 4: Skip Phase 2, Add Features

**Description**: Move directly to new feature development

**Pros**:
- Faster user-visible value delivery
- More exciting work

**Cons**:
- Technical debt accumulates
- Features built on weak foundation
- Future refactoring becomes harder

**Decision**: **Rejected** - Technical foundation must be solid

## Consequences

### Positive

1. **Faster Development**: 30-40% faster build and test cycles
2. **Better UX**: Clear error messages with actionable guidance
3. **Easier Maintenance**: Lower complexity, better testability
4. **Smaller Distribution**: 20-30% smaller binaries
5. **Solid Foundation**: Ready for feature development

### Negative

1. **Refactoring Risk**: Changes could introduce bugs
2. **Time Investment**: 3-5 days before new features
3. **Learning Curve**: New test infrastructure patterns
4. **Backward Compatibility**: Internal changes only, but still risky

### Neutral

1. **No API Changes**: This is internal optimization
2. **No New Features**: Users see indirect benefits only
3. **Documentation Updates**: Need to update contributor guide

## Implementation Plan

### Phase 2.1: Performance (Week 1)
1. Test infrastructure refactoring
2. Binary size optimization
3. Compilation time improvements
4. Benchmarking and validation

### Phase 2.2: Quality (Week 2)
1. Error handling enhancement
2. Code complexity reduction
3. Documentation updates
4. Final review and merge

### Rollout
1. Create `optimize/phase-2` branch
2. Implement incrementally with tests
3. PR with before/after benchmarks
4. Review and merge
5. Monitor for regressions

## Validation

### Success Metrics

**Must Have** (Blocking):
- ✅ All tests passing
- ✅ No new Clippy warnings
- ✅ Test time < 15s (from ~25s)
- ✅ Binary size < 6MB (from ~7MB)

**Should Have** (Desirable):
- ✅ Incremental build < 30s
- ✅ Error messages include file paths
- ✅ All functions complexity < 15

**Nice to Have** (Stretch):
- ✅ Binary size < 5MB
- ✅ Test time < 10s
- ✅ All functions complexity < 10

### Benchmarks

Before starting Phase 2:
```bash
# Baseline measurements
time cargo test                    # Record: XX.XXs
time cargo build --release         # Record: XX.XXs
ls -lh target/release/harness-gate # Record: X.XMB
cargo clippy -- -W clippy::cognitive_complexity
```

After completing Phase 2:
```bash
# Compare improvements
time cargo test                    # Target: < 15s
time cargo build --release         # Target: similar
ls -lh target/release/harness-gate # Target: < 6MB
cargo clippy -- -W clippy::cognitive_complexity # Target: no warnings
```

## References

- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Min-sized Rust](https://github.com/johnthagen/min-sized-rust)
- [The Cargo Book - Profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)
- [Error Handling in Rust](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
- Phase 1 Spec: `docs/specs/optimization-phase-1.md`
- Phase 2 Spec: `docs/specs/optimization-phase-2.md`

## Related Decisions

- **ADR 0002**: Optimize Release Builds - Extends this decision
- **ADR 0003**: Enhance CI Pipeline - Performance improvements help CI
- **ADR 0004**: Add Integration Tests - Phase 2 optimizes these tests

## Review & Approval

- **Author**: Claude Opus 5
- **Created**: 2026-08-26
- **Reviewers**: TBD
- **Status**: Accepted, merged to `main`, and cross-platform CI verified

## Notes

### Open Questions

1. **Q**: Should we parallelize tests with `cargo nextest`?
   **A**: Consider after shared fixtures implementation

2. **Q**: How aggressive should we be with dependency pruning?
   **A**: Run `cargo-bloat` first, only prune clear wins

3. **Q**: Should we add performance regression tests?
   **A**: Yes, add benchmark CI job in future PR

### Future Considerations

- Consider `cargo-nextest` for even faster tests
- Explore workspace split if compile times still problematic
- Add automated performance regression detection
- Consider compile-time feature flags for optional features

---

**Decision Made**: 2026-08-26
**Status**: Accepted, merged to `main`, and cross-platform CI verified
