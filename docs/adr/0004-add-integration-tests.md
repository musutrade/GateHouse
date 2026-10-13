# ADR-0004: Add Integration Tests for End-to-End Scenarios

## Status

**Accepted** (2024-08-26)

## Context

Current test coverage consists of 58 unit tests across 10 modules:
- ✅ Configuration validation (config.rs: 16 tests)
- ✅ Audit scanning (audit.rs: 18 tests)
- ✅ Secret detection (secrets.rs: 9 tests)
- ✅ Signal handling (process.rs: 4 tests)
- ✅ Other modules have unit tests

However, critical gaps exist:

### Missing Test Types

**Integration Tests**: None
- No `tests/` directory
- No end-to-end workflow testing
- No CLI argument parsing tests

**Untested Scenarios**:
- Complete `init -> verify` workflow
- Cross-module interactions
- CLI subcommand behavior
- Real Git repository scenarios
- Docker service lifecycle
- Error recovery paths

**Risks**:
- Unit tests pass but full workflow fails
- Breaking changes in module interactions
- CLI regressions
- Platform-specific issues in integration

### Current Test Structure

```
tools/harness-gate/
├── src/
│   ├── main.rs (0 tests - CLI parsing untested)
│   ├── config.rs (16 tests)
│   ├── audit.rs (18 tests)
│   └── ... (other unit tests)
└── tests/ (missing)
```

Industry best practices recommend:
- **70/20/10 split**: 70% unit, 20% integration, 10% e2e
- Current: ~100% unit, 0% integration, 0% e2e

## Decision

We will add integration tests in a new `tests/` directory:

### Test Structure

```
tools/harness-gate/
└── tests/
    ├── common/
    │   └── mod.rs          # Shared test utilities
    ├── integration_test.rs # End-to-end workflows
    ├── cli_test.rs         # CLI argument parsing
    └── git_integration_test.rs # Git scenarios
```

### Test Scenarios to Cover

**Priority 1 (This Phase)**:

1. **CLI Parsing** (`cli_test.rs`)
   - All subcommands parse correctly
   - Invalid arguments produce clear errors
   - Global flags work across subcommands

2. **Init Workflow** (`integration_test.rs`)
   - `harness-gate init --preset rust-api`
   - Generated config files are valid
   - Config validation passes

3. **Verify Workflow** (`integration_test.rs`)
   - `harness-gate verify --all`
   - Steps execute in order
   - Reports are generated correctly

**Priority 2 (Next Phase)**:

4. **Git Integration** (`git_integration_test.rs`)
   - Scope detection with real Git repo
   - Changed file detection
   - Staged vs unstaged differences

5. **Error Scenarios**
   - Invalid configuration
   - Missing dependencies
   - Docker unavailable

### Test Infrastructure

**Utilities** (`common/mod.rs`):
```rust
pub struct TestContext {
    temp_dir: TempDir,
    git_repo: Option<Repository>,
}

impl TestContext {
    pub fn new() -> Self { /* ... */ }
    pub fn init_git(&mut self) { /* ... */ }
    pub fn write_config(&self, content: &str) { /* ... */ }
    pub fn run_harness_gate(&self, args: &[&str]) -> Output { /* ... */ }
}
```

**Isolation**:
- Each test gets a temporary directory
- Cleaned up automatically via RAII
- No shared state between tests

## Consequences

### Positive

✅ **Workflow Confidence**: Verify complete user journeys work end-to-end.

✅ **Regression Prevention**: Breaking changes caught before release.

✅ **CLI Testing**: Argument parsing and error messages tested.

✅ **Documentation**: Integration tests serve as executable documentation.

✅ **Refactoring Safety**: Can refactor internals with confidence.

✅ **Platform Testing**: Integration tests run on all platforms in CI (per ADR-0003).

### Negative

⚠️ **Slower Test Suite**: Integration tests are slower than unit tests:
- Unit tests: ~0.3s
- Integration tests: Expected ~2-5s
- Still acceptable for CI

⚠️ **More Complex Setup**: Require temporary directories, Git repos, config files.

⚠️ **Maintenance**: Need to update when CLI changes.

⚠️ **Flakiness Risk**: External dependencies (Git, filesystem) can introduce flakiness.

### Mitigation Strategies

1. **Keep Integration Tests Focused**: Test major paths, not every edge case
2. **Use Test Utilities**: Shared helpers reduce duplication
3. **Proper Cleanup**: Use RAII pattern for temp directories
4. **Timeout Protection**: Add reasonable timeouts
5. **Retry Flaky Tests**: CI can retry on failure

### Test Coverage Goals

| Test Type | Current | Target | Purpose |
|-----------|---------|--------|---------|
| Unit | 58 tests | 70-80 tests | Module logic, edge cases |
| Integration | 0 tests | 10-15 tests | Workflows, CLI, Git |
| E2E | 0 tests | 2-5 tests | Full scenarios |
| **Total** | **58** | **85-100** | Comprehensive coverage |

### Success Criteria

After implementation:
- ✅ CLI parsing has tests for all subcommands
- ✅ `init` workflow tested end-to-end
- ✅ `verify` workflow tested end-to-end
- ✅ Tests pass on all platforms (Ubuntu, macOS, Windows)
- ✅ Test suite completes in <10 seconds
- ✅ No flaky tests in 10 consecutive CI runs

### Examples

**CLI Test**:
```rust
#[test]
fn test_verify_requires_project_root() {
    let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("verify")
        .arg("--all")
        .current_dir("/tmp/non-project")
        .output()
        .unwrap();
    
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("not a harness-gate project"));
}
```

**Integration Test**:
```rust
#[test]
fn test_init_and_verify_workflow() {
    let ctx = TestContext::new();
    
    // Init with preset
    let output = ctx.run_harness_gate(&["init", "--preset", "rust-api"]);
    assert!(output.status.success());
    
    // Verify config is valid
    let output = ctx.run_harness_gate(&["config", "check"]);
    assert!(output.status.success());
    
    // Verify can run
    let output = ctx.run_harness_gate(&["verify", "--all"]);
    assert!(output.status.success());
}
```

### Alternatives Considered

1. **System-level tests with shell scripts**
   - Pros: Language-agnostic, easy to write
   - Cons: Platform-dependent, harder to maintain
   - Decision: Use Rust integration tests for consistency

2. **Separate test binary**
   - Pros: Complete isolation
   - Cons: More complex setup, slower
   - Decision: Use standard `tests/` directory

3. **Only add E2E tests**
   - Pros: Fewer tests to write
   - Cons: Too coarse-grained, slow feedback
   - Decision: Add both integration and E2E tests

## References

- [Rust Book: Integration Tests](https://doc.rust-lang.org/book/ch11-03-test-organization.html)
- [cargo test documentation](https://doc.rust-lang.org/cargo/commands/cargo-test.html)
- [Testing Pyramid](https://martinfowler.com/articles/practical-test-pyramid.html)
