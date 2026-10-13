# Design: Implement Phase 2 Optimization

## Context

Current state (baseline after Phase 1):
- Test execution time: ~20 seconds (18 tests)
- Binary size: 4.3MB (release build)
- Compilation time: ~2.5 minutes (clean build)
- Error handling: Uses `anyhow` for all errors (lacks typing)
- Code organization: Some duplication, opportunities for refactoring

Constraints:
- Must maintain 100% test pass rate on all platforms (Linux, macOS, Windows)
- Cannot break existing CLI interface or configuration format
- Must maintain zero clippy warnings standard
- Changes must be incremental and independently verifiable

See proposal.md for motivation and full context.

## Goals / Non-Goals

**Goals:**
- Reduce test execution time by 30-50% (target: <15s)
- Reduce binary size by 20-30% (target: <3.5MB)
- Improve compilation time by 10-20%
- Implement typed error handling with better user messages
- Reduce code duplication and improve maintainability

**Non-Goals:**
- Adding new features or changing existing functionality
- Changing public API or configuration schema
- Major architectural rewrites (incremental improvements only)
- Breaking changes to existing behavior

## Decisions

### Decision 1: Use cargo-nextest for Test Parallelization

**Choice:** Adopt `cargo-nextest` as the test runner instead of default `cargo test`.

**Rationale:**
- Better parallelization (per-test granularity vs per-file)
- Faster execution for our 18 integration tests
- Better CI integration and reporting
- Industry standard (used by major Rust projects)

**Alternatives Considered:**
- Keep `cargo test` with `--test-threads`: Limited parallelization, no per-test control
- Manual test splitting: Too much maintenance overhead

**Implementation:**
- Install via CI: `cargo install cargo-nextest`
- Update CI workflow: Replace `cargo test` with `cargo nextest run`
- Keep `cargo test` available for local development (backwards compatible)

**Trade-off:** Additional dependency, but significant performance gain (30-50% faster).

---

### Decision 2: Optimize Binary Size with Multi-Profile Strategy

**Choice:** Create separate optimization profiles for different use cases.

**Profiles:**
```toml
# Cargo.toml profiles
[profile.release]
strip = true
lto = true
codegen-units = 1
opt-level = 3          # Balanced size/speed

[profile.release-small]
inherits = "release"
opt-level = "z"        # Optimize for size
panic = "abort"        # No unwinding
```

**Rationale:**
- Default `release` balances size and performance
- `release-small` for size-critical deployments
- User can choose based on their needs

**Alternatives Considered:**
- Single aggressive profile: Might sacrifice too much performance
- No size optimization: Misses easy wins

**Implementation:**
- Add profiles to Cargo.toml
- Document trade-offs in README
- CI builds both profiles for comparison

---

### Decision 3: Dependency Audit and Replacement Strategy

**Choice:** Audit dependencies with `cargo-bloat` and `cargo-udeps`, replace heavy ones.

**Targets for optimization:**
1. **Feature flags**: Make optional features truly optional
   - `clap` features: Only enable what we use
   - `serde` features: Minimal feature set
   
2. **Dependency replacement** (if analysis shows benefit):
   - Consider lighter alternatives for non-critical deps
   - Remove unused transitive dependencies

**Process:**
1. Run `cargo bloat --release` to identify heavy dependencies
2. Run `cargo-udeps` to find unused dependencies
3. For each heavy dependency: analyze usage and consider alternatives
4. Make changes incrementally with before/after benchmarks

**Rationale:**
- Data-driven decisions (bloat analysis first)
- Incremental changes allow rollback if issues arise
- Each change independently verifiable

**Alternatives Considered:**
- Aggressive removal without analysis: Too risky
- Keep all dependencies: Misses optimization opportunities

---

### Decision 4: Migrate Error Handling from anyhow to thiserror

**Choice:** Implement typed error enums with `thiserror` for better error handling.

**Error Architecture:**
```rust
// Per-module error types
#[derive(thiserror::Error, Debug)]
pub enum AuditError {
    #[error("Failed to read audit config: {0}")]
    ConfigRead(#[from] std::io::Error),
    
    #[error("Invalid audit rule '{rule}': {reason}")]
    InvalidRule { rule: String, reason: String },
    
    #[error("Git operation failed: {0}")]
    GitError(String),
}

// Top-level CLI error
#[derive(thiserror::Error, Debug)]
pub enum CliError {
    #[error(transparent)]
    Audit(#[from] AuditError),
    
    #[error(transparent)]
    Secrets(#[from] SecretsError),
    
    // ... other module errors
}
```

**Rationale:**
- Typed errors enable better error handling and testing
- User-facing error messages with context
- Compiler-enforced error propagation
- Better error documentation

**Migration Strategy:**
1. Create error types per module (audit, secrets, verify, scope)
2. Migrate one module at a time
3. Update tests to check specific error types
4. Keep `anyhow` for main.rs entry point (converts from CliError)

**Alternatives Considered:**
- Keep `anyhow`: Simpler but loses type safety
- Use `Box<dyn Error>`: Less ergonomic, no automatic Display impl

---

### Decision 5: Code Refactoring Strategy

**Choice:** Incremental refactoring focused on high-impact areas.

**Priority Areas:**
1. **Extract common patterns** (DRY violations)
   - File reading/writing helpers
   - Git command execution patterns
   - Error context addition patterns

2. **Improve module boundaries**
   - Clearer separation of concerns
   - Reduce cross-module coupling

3. **Add inline documentation**
   - Complex algorithms (gitignore parsing, scope detection)
   - Public APIs

**Approach:**
- Refactor in small commits (one pattern at a time)
- Tests must pass after each refactor
- Use clippy suggestions as guidance

**Rationale:**
- Incremental changes are safer and easier to review
- Focus on measurable improvements (duplication reduction)
- Maintains test coverage throughout

---

## Risks / Trade-offs

### Risk 1: Performance Regressions
**Risk:** Optimization changes could introduce subtle bugs or performance regressions.

**Mitigation:**
- Benchmark before/after for each major change
- Comprehensive test suite runs after each commit
- Git bisect available if issues discovered later
- Each optimization is a separate commit for easy rollback

---

### Risk 2: Binary Size vs Performance Trade-off
**Risk:** Aggressive size optimization might hurt runtime performance.

**Mitigation:**
- Use separate profiles (`release` vs `release-small`)
- Benchmark both size and execution speed
- Document trade-offs for users to choose
- Default profile balances both concerns

---

### Risk 3: Error Handling Migration Complexity
**Risk:** Migrating error handling across entire codebase is error-prone.

**Mitigation:**
- Migrate one module at a time
- Keep both `anyhow` and `thiserror` during transition
- Each module migration is a separate commit
- Tests verify error behavior doesn't change

---

### Risk 4: cargo-nextest Compatibility
**Risk:** CI or platform-specific issues with cargo-nextest.

**Mitigation:**
- Test on all platforms (Linux, macOS, Windows) before merging
- Keep `cargo test` as fallback in CI
- Document installation requirements
- Gradual rollout (local first, then CI)

---

### Risk 5: Compilation Time Regressions
**Risk:** Some optimizations (LTO, single codegen-unit) increase compile time.

**Mitigation:**
- Only apply aggressive optimization to release builds
- Keep dev builds fast (no LTO, multiple codegen-units)
- Monitor CI build times
- Document expected compile time impact

---

## Implementation Strategy

### Phase 2A: Performance Wins (Low Risk)
1. Integrate cargo-nextest
2. Add optimization profiles to Cargo.toml
3. Run dependency audit (bloat, udeps)
4. Quick wins: Remove unused deps, optimize features

**Expected Impact:** 30-40% test time reduction, 10-15% binary size reduction  
**Timeline:** 1 day  
**Risk:** Low

---

### Phase 2B: Error Handling Migration (Medium Risk)
1. Create error type infrastructure
2. Migrate modules one by one (audit → secrets → verify → scope)
3. Update tests for typed errors
4. Improve error messages

**Expected Impact:** Better UX, improved maintainability  
**Timeline:** 1-2 days  
**Risk:** Medium (requires careful testing)

---

### Phase 2C: Code Quality Refactoring (Low Risk)
1. Extract common patterns
2. Improve module organization
3. Add inline documentation
4. Address remaining code duplication

**Expected Impact:** Improved maintainability, reduced duplication  
**Timeline:** 1-2 days  
**Risk:** Low (incremental, well-tested)

---

### Rollback Strategy

Each optimization track is independent:
- **Track 1 (Tests):** Revert cargo-nextest, return to `cargo test`
- **Track 2 (Binary):** Revert Cargo.toml profile changes
- **Track 3 (Deps):** Restore removed dependencies from git history
- **Track 4 (Errors):** Keep `anyhow`, don't merge error migration
- **Track 5 (Refactor):** Revert specific refactoring commits

All changes are behind git commits, making rollback straightforward.

---

## Verification Plan

### Performance Benchmarks
```bash
# Before Phase 2
time cargo test                           # Baseline: ~20s
time cargo build --release                # Baseline: ~2.5min
ls -lh target/release/harness-gate        # Baseline: 4.3MB

# After Phase 2
time cargo nextest run                    # Target: <15s
time cargo build --release                # Target: <2.5min (similar or better)
ls -lh target/release/harness-gate        # Target: <3.5MB
cargo build --release --profile release-small  # Target: <3MB
```

### Quality Checks
- Zero clippy warnings maintained
- All 18 tests passing on all platforms
- Code coverage ≥80% maintained
- No breaking changes to CLI interface

### Success Criteria
- ✅ Test time reduced by ≥30%
- ✅ Binary size reduced by ≥20%
- ✅ Error messages improved (qualitative review)
- ✅ Code duplication reduced (measurable via tooling)
- ✅ All tests passing, zero warnings
