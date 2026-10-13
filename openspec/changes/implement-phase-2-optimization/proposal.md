# Proposal: Implement Phase 2 Optimization

## Why

Phase 1 successfully established a clean baseline (all tests passing, zero warnings, stable CI). Phase 2 addresses performance bottlenecks and code quality issues that were deferred: test execution is slow (~20s), binary size is large (4.3MB), and compilation takes significant time. These issues impact developer productivity and deployment efficiency. Now is the right time because we have a stable foundation and comprehensive test coverage to validate optimizations.

## What Changes

This change implements the 5 optimization tracks defined in Phase 2 spec:

1. **Test Performance Optimization**
   - Parallelize test execution with `cargo nextest`
   - Reduce test setup overhead and fixture complexity
   - Optimize slow integration tests
   - Target: 30-50% reduction in test time (from ~20s to <15s)

2. **Binary Size Reduction**
   - Enable aggressive optimization flags (opt-level, LTO, codegen-units)
   - Audit and remove unused dependencies
   - Replace heavy dependencies with lighter alternatives
   - Use feature flags to make dependencies optional
   - Target: 20-30% size reduction (from 4.3MB to <3.5MB)

3. **Compilation Time Improvement**
   - Optimize dependency build times
   - Reduce monomorphization bloat
   - Consider incremental compilation improvements
   - Target: 10-20% faster builds

4. **Error Handling Enhancement**
   - Migrate from `anyhow` to `thiserror` for typed errors
   - Add error context with user-actionable messages
   - Implement error codes for documentation
   - Improve error display formatting

5. **Code Quality & Maintainability**
   - Reduce code duplication (DRY violations)
   - Improve module organization and boundaries
   - Add inline documentation for complex logic
   - Refactor for better testability

## Capabilities

### New Capabilities

This is a pure optimization and refactoring change that improves existing behavior without adding new functional requirements.

### Modified Capabilities

This change does not modify existing capability requirements - all existing features and behaviors remain unchanged. The optimizations are implementation-level improvements (performance, size, maintainability) that don't alter external contracts or functionality.

**Note:** Setting `skip_specs: true` in `.openspec.yaml` because this is pure refactoring and optimization work with no spec-level behavior changes.

## Impact

**Affected Code:**
- `Cargo.toml` - dependency updates, feature flags, optimization profiles
- `tools/harness-gate/src/*.rs` - error handling refactoring across all modules
- Test files - test parallelization and optimization
- CI configuration - integration of new testing tools

**Performance Impact:**
- ✅ Faster test execution (30-50% improvement)
- ✅ Smaller binary size (20-30% reduction)
- ✅ Faster compilation (10-20% improvement)
- ✅ Better developer experience with clearer error messages

**Dependencies:**
- May replace or remove some dependencies (e.g., lighter alternatives)
- Add `cargo-nextest` for test execution
- Add `cargo-bloat`, `cargo-udeps` as dev tools
- Migrate error handling from `anyhow` to `thiserror`

**Risks:**
- Medium risk: Optimization changes could introduce regressions
- Mitigation: Comprehensive testing at each step, benchmarking before/after
- Rollback: Each optimization is a separate commit for easy revert

**Timeline:** 3-5 days across 5 parallel tracks (can be implemented incrementally)

**Success Metrics:**
- Test time: < 15 seconds (baseline: ~20s)
- Binary size: < 3.5MB (baseline: 4.3MB)
- Build time: < 2 minutes (baseline: ~2.5 minutes)
- Zero test failures or regressions
- Improved error message clarity (qualitative assessment)
