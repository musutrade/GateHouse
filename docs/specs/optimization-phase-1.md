# Phase 1 Optimization Specification

**Version**: 1.0  
**Status**: Historical / superseded
**Created**: 2024-08-26  
**Owner**: Harness-Gate Team  

## Overview

> This is the original 2024 planning document. Its implementation checklist
> is retained for historical context and is superseded by
> [ADR-0025](../adr/0025-phase-1-quality-baseline-gates.md) and the Phase 1
> quality-baseline OpenSpec change. Current acceptance status is tracked there.

This specification details the Phase 1 optimization work for Harness-Gate, focusing on quick wins that significantly improve project quality with minimal effort.

## Goals

1. **Reduce binary size** by 30-50% through release optimizations
2. **Enhance security** by adding dependency vulnerability scanning
3. **Improve reliability** by testing on all platforms
4. **Increase visibility** through code coverage reporting
5. **Prevent regressions** by adding integration tests

## Scope

### In Scope
- Release build optimizations (LTO, strip, codegen-units)
- CI enhancements (security audit, multi-platform testing, coverage)
- Integration tests (CLI parsing, init/verify workflows)
- Documentation (ADRs, this spec)

### Out of Scope
- Code refactoring (Phase 2)
- Performance optimizations beyond build configuration (Phase 2)
- User experience improvements (Phase 2)
- Dependency updates (Phase 3)

## Technical Specifications

### 1. Release Build Optimization

**File**: `tools/harness-gate/Cargo.toml`

**Change**:
```toml
[profile.release]
strip = true          # Remove debug symbols
lto = true            # Link-time optimization
codegen-units = 1     # Better optimization at cost of compile time
```

**Rationale**: See [ADR-0002](../adr/0002-optimize-release-builds.md)

**Expected Impact**:
- Binary size: 30-50% reduction
- Startup time: 5-15% improvement
- Build time: 2-5x slower (acceptable for releases)

**Testing**:
```bash
# Before
cargo build --release
ls -lh target/release/harness-gate

# After (measure improvement)
cargo build --release
ls -lh target/release/harness-gate
```

**Acceptance Criteria**:
- ✅ Binary size reduced by at least 30%
- ✅ Release builds complete successfully on CI
- ✅ All tests pass with optimized binary

---

### 2. Security Audit

**File**: `.github/workflows/ci.yml`

**Change**: Add new job
```yaml
security-audit:
  name: Security Audit
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    
    - name: Install Rust
      uses: dtolnay/rust-toolchain@stable
    
    - name: Cache cargo-audit
      uses: actions/cache@v3
      with:
        path: ~/.cargo/bin/cargo-audit
        key: ${{ runner.os }}-cargo-audit
    
    - name: Install cargo-audit
      run: cargo install cargo-audit --locked
    
    - name: Run security audit
      run: cargo audit --deny warnings
      working-directory: tools/harness-gate
```

**Rationale**: See [ADR-0003](../adr/0003-enhance-ci-pipeline.md)

**Expected Impact**:
- Catch known vulnerabilities before production
- CI time increase: +30-60 seconds

**Testing**:
```bash
# Local testing
cargo install cargo-audit
cargo audit --deny warnings
```

**Acceptance Criteria**:
- ✅ Security audit runs on every PR
- ✅ Build fails if vulnerabilities found
- ✅ Caching works (subsequent runs <10s)

---

### 3. Multi-Platform Testing

**File**: `.github/workflows/ci.yml`

**Change**: Modify test job
```yaml
test:
  name: Test
  strategy:
    fail-fast: false
    matrix:
      os: [ubuntu-latest, macos-latest, windows-latest]
  runs-on: ${{ matrix.os }}
  steps:
    - uses: actions/checkout@v4
    
    - name: Install Rust
      uses: dtolnay/rust-toolchain@stable
    
    - name: Cache cargo
      uses: actions/cache@v3
      with:
        path: |
          ~/.cargo/bin/
          ~/.cargo/registry/index/
          ~/.cargo/registry/cache/
          ~/.cargo/git/db/
          tools/harness-gate/target/
        key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
    
    - name: Run tests
      run: cargo test --locked
      working-directory: tools/harness-gate
```

**Rationale**: See [ADR-0003](../adr/0003-enhance-ci-pipeline.md)

**Expected Impact**:
- Catch platform-specific bugs before release
- CI time: Same (parallel execution)

**Testing**:
- Monitor CI runs across all platforms
- Verify tests pass on each platform

**Acceptance Criteria**:
- ✅ Tests run on Ubuntu, macOS, and Windows
- ✅ All 58 existing tests pass on all platforms
- ✅ Caching reduces test time on repeated runs

---

### 4. Code Coverage

**File**: `.github/workflows/ci.yml`

**Change**: Add coverage job
```yaml
coverage:
  name: Code Coverage
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    
    - name: Install Rust
      uses: dtolnay/rust-toolchain@stable
    
    - name: Install tarpaulin
      run: cargo install cargo-tarpaulin --locked
    
    - name: Generate coverage
      run: cargo tarpaulin --out Xml --output-dir coverage
      working-directory: tools/harness-gate
    
    - name: Upload to Codecov
      uses: codecov/codecov-action@v3
      with:
        files: tools/harness-gate/coverage/cobertura.xml
        fail_ci_if_error: false  # Non-blocking initially
```

**Rationale**: See [ADR-0003](../adr/0003-enhance-ci-pipeline.md)

**Expected Impact**:
- Visibility into test coverage
- CI time increase: +2-3 minutes

**Configuration**:
- Create `codecov.yml` in repository root:
```yaml
coverage:
  status:
    project:
      default:
        target: 70%
        threshold: 5%
    patch:
      default:
        target: 50%
```

**Testing**:
```bash
# Local testing
cargo install cargo-tarpaulin
cargo tarpaulin --out Html
open tarpaulin-report.html
```

**Acceptance Criteria**:
- ✅ Coverage report generated on every PR
- ✅ Coverage badge added to README
- ✅ Baseline coverage established (expect 60-70%)

---

### 5. Integration Tests

**Files**: New directory structure
```
tools/harness-gate/tests/
├── common/
│   └── mod.rs
├── cli_test.rs
└── integration_test.rs
```

**Implementation**:

#### `tests/common/mod.rs`
```rust
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

pub struct TestContext {
    pub temp_dir: TempDir,
    pub project_root: PathBuf,
}

impl TestContext {
    pub fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();
        Self { temp_dir, project_root }
    }
    
    pub fn run_harness_gate(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_harness-gate"))
            .args(args)
            .arg("--project-root")
            .arg(&self.project_root)
            .output()
            .expect("Failed to execute harness-gate")
    }
    
    pub fn write_file(&self, path: impl AsRef<Path>, content: &str) {
        let full_path = self.project_root.join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(full_path, content).unwrap();
    }
}
```

#### `tests/cli_test.rs` (Priority tests)
```rust
mod common;
use common::TestContext;

#[test]
fn test_help_command() {
    // Test --help produces output
}

#[test]
fn test_version_command() {
    // Test --version produces output
}

#[test]
fn test_invalid_subcommand() {
    // Test error on unknown subcommand
}

#[test]
fn test_verify_without_project() {
    // Test error when not in project
}
```

#### `tests/integration_test.rs` (Priority tests)
```rust
mod common;
use common::TestContext;

#[test]
fn test_init_workflow() {
    // Test: init --preset rust-api
    // Verify: config files created and valid
}

#[test]
fn test_config_check() {
    // Test: config check with valid config
}

#[test]
fn test_presets_command() {
    // Test: presets lists available presets
}
```

**Dependencies**: Add to `Cargo.toml`
```toml
[dev-dependencies]
tempfile = "3.8"
assert_cmd = "2.0"
predicates = "3.0"
```

**Rationale**: See [ADR-0004](../adr/0004-add-integration-tests.md)

**Expected Impact**:
- Prevent CLI regressions
- Validate end-to-end workflows
- Test time increase: +2-5 seconds

**Acceptance Criteria**:
- ✅ At least 5 integration tests added
- ✅ CLI parsing tested for all subcommands
- ✅ Init and verify workflows tested
- ✅ Tests pass on all platforms

---

## Implementation Plan

### Task Breakdown

| # | Task | Effort | Dependencies | Assignee |
|---|------|--------|--------------|----------|
| 1 | Create ADR documents | 2h | None | ✅ Done |
| 2 | Add release profile config | 10m | #1 | Pending |
| 3 | Add security audit to CI | 30m | #1 | Pending |
| 4 | Add multi-platform testing | 1h | #1 | Pending |
| 5 | Add code coverage | 2h | #1 | Pending |
| 6 | Create test utilities | 1h | #1 | Pending |
| 7 | Write CLI integration tests | 2h | #6 | Pending |
| 8 | Write workflow integration tests | 2h | #6 | Pending |
| 9 | Measure and document results | 1h | #2-8 | Pending |
| 10 | Update README with badges | 30m | #5 | Pending |

**Total Estimated Effort**: 12 hours

### Timeline

- **Day 1**: Tasks #1-3 (ADRs, release config, security audit)
- **Day 2**: Tasks #4-5 (multi-platform testing, coverage)
- **Day 3**: Tasks #6-8 (integration tests)
- **Day 4**: Tasks #9-10 (measurement, documentation)

### Rollout Strategy

1. **Create feature branch**: `optimize/phase-1`
2. **Incremental PRs**: One PR per major change
3. **Monitor CI**: Watch for any instability
4. **Measure impact**: Document before/after metrics
5. **Update docs**: Reflect changes in README and CONTRIBUTING

---

## Success Metrics

### Quantitative

| Metric | Before | Target | How to Measure |
|--------|--------|--------|----------------|
| Binary size | TBD | -30% to -50% | `ls -lh target/release/harness-gate` |
| CI security checks | 0 | 1 | Pipeline includes cargo-audit |
| Platforms tested | 1 | 3 | Tests run on Ubuntu, macOS, Windows |
| Code coverage | Unknown | 60-70% | Codecov report |
| Integration tests | 0 | 5+ | `cargo test --test '*'` |
| CI duration | ~5min | <10min | GitHub Actions timing |

### Qualitative

- ✅ Security vulnerabilities detected automatically
- ✅ Platform-specific bugs caught before release
- ✅ Coverage trends visible on PRs
- ✅ Confidence in end-to-end workflows
- ✅ Clear documentation of architectural decisions

---

## Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| CI too slow | Medium | Medium | Cache aggressively, make coverage optional |
| Flaky tests | Low | High | Use proper isolation, add retries |
| Coverage noise | Medium | Low | Set reasonable thresholds, non-blocking initially |
| Build failures | Low | High | Test locally first, incremental rollout |

---

## References

- [ADR-0001: Use Rust for CLI](../adr/0001-use-rust-for-cli.md)
- [ADR-0002: Optimize Release Builds](../adr/0002-optimize-release-builds.md)
- [ADR-0003: Enhance CI Pipeline](../adr/0003-enhance-ci-pipeline.md)
- [ADR-0004: Add Integration Tests](../adr/0004-add-integration-tests.md)
- [Project overview](../../ABOUT.md)

---

## Sign-off

This historical plan is superseded. Technical, security, and documentation
acceptance are recorded in ADR-0025 and its OpenSpec evidence package.

---

**Next Steps**: See the current Phase 1 and Phase 2 OpenSpec records.
