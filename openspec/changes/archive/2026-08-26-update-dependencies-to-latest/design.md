# Design: Update Dependencies to Latest Versions

## Context

This is a routine dependency maintenance update. The project uses standard Rust dependency management via Cargo. Current dependencies are from early 2024-2025 and need updating to latest stable versions within semver constraints.

See proposal.md for motivation.

## Goals / Non-Goals

**Goals:**
- Update all dependencies to latest semver-compatible versions
- Maintain backward compatibility (no breaking changes to harness-gate)
- Ensure all tests pass on all platforms

**Non-Goals:**
- Major version upgrades requiring code changes
- Replacing dependencies with alternatives
- Adding new dependencies

## Decisions

### Decision 1: Use `cargo update` for Cargo.lock Updates

**Choice:** Run `cargo update` to update Cargo.lock within existing Cargo.toml constraints

**Rationale:**
- Safe: respects semver constraints in Cargo.toml
- Fast: automatic dependency resolution
- Standard practice in Rust ecosystem

**Alternatives considered:**
- Manual Cargo.lock editing: error-prone, not recommended
- `cargo upgrade` (cargo-edit): requires additional tool, not needed for patch/minor updates

### Decision 2: Manual Cargo.toml Version Bumps

**Choice:** Manually review and update version numbers in Cargo.toml for minor/patch updates

**Rationale:**
- Explicit control over version constraints
- Clear git diff showing intent
- Can review changelogs before bumping

**Alternatives considered:**
- `cargo upgrade`: automated but may introduce unexpected changes
- Keep existing constraints: misses important updates

### Decision 3: Incremental Update Strategy

**Choice:** Update all dependencies at once, but test thoroughly

**Rationale:**
- Simpler: one PR, one test cycle
- Clear: single commit shows all updates
- The project is small (~10k LOC), risk is manageable

**Alternatives considered:**
- Update one-by-one: slower, more PRs, same risk overall
- Update by category: arbitrary grouping, no real benefit

## Risks / Trade-offs

### Risk 1: API Deprecations or Breaking Changes
**Impact:** Code may use deprecated APIs that now warn or fail  
**Mitigation:** Run `cargo clippy` to catch warnings; review changelogs for major dependencies

### Risk 2: Test Failures on Specific Platforms
**Impact:** Updates may expose platform-specific issues  
**Mitigation:** CI tests on Linux, macOS, Windows; rollback if failures

### Risk 3: Binary Size Increase
**Impact:** New dependency versions may be larger  
**Mitigation:** Measure before/after; acceptable if <10% increase

### Risk 4: Performance Regression
**Impact:** New versions may be slower  
**Mitigation:** Run test suite timing comparison; rollback if >10% slower

## Migration Plan

1. **Update Cargo.lock:** `cargo update`
2. **Update Cargo.toml:** Review and bump version constraints
3. **Test locally:** `cargo test`, `cargo clippy`, `cargo build --release`
4. **Check changelogs:** Review major dependency changes
5. **Commit changes:** Clear commit message with before/after versions
6. **CI validation:** Wait for all platforms to pass
7. **Merge:** Once CI is green

**Rollback:** `git revert <commit>` - simple and safe

## Open Questions

None - this is a straightforward maintenance update.
