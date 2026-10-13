# ADR-0003: Enhance CI Pipeline with Security and Coverage Checks

## Status

**Accepted** (2024-08-26)

## Context

The current CI pipeline (`.github/workflows/ci.yml`) provides basic quality checks:
- ✅ Tests on Ubuntu
- ✅ Code formatting (`cargo fmt`)
- ✅ Linting (`cargo clippy`)
- ✅ Cross-platform builds (Ubuntu, macOS, Windows)

However, critical checks are missing:

### Security Gaps
- No dependency vulnerability scanning
- No license compliance checking
- Dependencies could have known CVEs

### Test Coverage Gaps
- Tests only run on Ubuntu
- Platform-specific bugs could slip through
- No code coverage metrics
- Unknown which code paths are tested

### Quality Gaps
- No measurement of test coverage percentage
- No tracking of coverage trends
- Difficult to identify untested code

Industry best practices for Rust CI include:
1. **Security auditing** with `cargo-audit`
2. **Multi-platform testing** (not just building)
3. **Code coverage reporting** with tools like `cargo-tarpaulin`
4. **Dependency checking** with `cargo-deny`

## Decision

We will enhance the CI pipeline with three new checks:

### 1. Security Audit
Add `cargo-audit` to check for known security vulnerabilities in dependencies.

```yaml
- name: Security audit
  run: |
    cargo install cargo-audit
    cargo audit --deny warnings
```

### 2. Multi-Platform Testing
Run full test suite on all three platforms (Ubuntu, macOS, Windows), not just Ubuntu.

```yaml
test:
  strategy:
    matrix:
      os: [ubuntu-latest, macos-latest, windows-latest]
```

### 3. Code Coverage
Integrate `cargo-tarpaulin` and Codecov for coverage reporting.

```yaml
- name: Coverage
  run: |
    cargo install cargo-tarpaulin
    cargo tarpaulin --out Xml
- uses: codecov/codecov-action@v3
```

### Phased Implementation

**Phase 1 (Immediate)**:
- Security audit (blocking)
- Multi-platform testing (blocking)

**Phase 2 (Next week)**:
- Code coverage (non-blocking initially)
- Set coverage threshold after baseline established

**Phase 3 (Future)**:
- `cargo-deny` for license and dependency policy
- Performance regression testing
- Benchmark tracking

## Consequences

### Positive

✅ **Security**: Automatic detection of vulnerable dependencies before they reach production.

✅ **Platform Reliability**: Catch platform-specific bugs (e.g., Windows path handling, macOS signal handling) before release.

✅ **Test Visibility**: Coverage metrics help identify:
- Untested code paths
- Areas needing more tests
- Coverage trends over time

✅ **Confidence**: Higher confidence in cross-platform reliability.

✅ **Developer Experience**: Coverage reports guide where to add tests.

### Negative

⚠️ **Longer CI Runtime**:
- Security audit: +30-60 seconds
- Multi-platform testing: 3x test duration (parallel, so ~same wall time)
- Coverage: +2-3 minutes
- Total CI time: ~5 minutes → ~8 minutes

⚠️ **CI Cost**: More compute minutes (GitHub Actions free tier: 2000 min/month should be sufficient).

⚠️ **Maintenance**: Additional tools to keep updated.

⚠️ **Potential Noise**: Coverage changes might create PR noise if not configured properly.

### Mitigation Strategies

1. **Cache cargo installs** to speed up tool installation
2. **Make coverage non-blocking initially** until we establish baseline
3. **Set reasonable coverage thresholds** (e.g., 70% initially, not 100%)
4. **Use matrix builds** for parallel execution

### Configuration Decisions

**cargo-audit**:
- Fail on warnings (`--deny warnings`)
- Run on every PR and main branch
- Cache audit database

**Multi-platform testing**:
- Run full test suite on all platforms
- Fail fast: false (get results from all platforms)
- Cache cargo artifacts per platform

**Code coverage**:
- Tool: `cargo-tarpaulin` (Rust-native, good accuracy)
- Report format: XML (Codecov compatible)
- Initially non-blocking (informational)
- Target threshold: 70% after baseline established

### Alternatives Considered

1. **`cargo-llvm-cov` instead of `tarpaulin`**
   - Pros: More accurate, LLVM-based
   - Cons: Requires nightly Rust, more complex setup
   - Decision: Stick with tarpaulin for simplicity

2. **Coveralls instead of Codecov**
   - Both are good options
   - Codecov chosen for better GitHub integration and free for open source

3. **`cargo-deny` in Phase 1**
   - Deferred to Phase 3 to avoid overwhelming changes
   - Will add after coverage is stable

### Success Metrics

After 2 weeks, we should see:
- ✅ Zero security vulnerabilities in dependencies
- ✅ Tests passing on all three platforms
- ✅ Coverage baseline established (expect 60-70%)
- ✅ CI time remains under 10 minutes

### Rollback Plan

If CI becomes too slow or unstable:
1. Make coverage optional (manual trigger)
2. Reduce platform matrix to Ubuntu + one other
3. Run security audit weekly instead of per-PR

## References

- [cargo-audit](https://github.com/rustsec/rustsec/tree/main/cargo-audit)
- [cargo-tarpaulin](https://github.com/xd009642/tarpaulin)
- [Codecov](https://about.codecov.io/)
- [GitHub Actions: Matrix Builds](https://docs.github.com/en/actions/using-jobs/using-a-matrix-for-your-jobs)
