# Tasks: Enhance Terminal Feedback

Parent: [proposal.md](proposal.md)

## 1. Specification and Architecture

- [x] 1.1 Record terminal-output policy in ADR-0006 (Priority: P0, Effort: S)
- [x] 1.2 Define compatibility, color selection, and rollback behavior (Priority: P0, Effort: S)

## 2. Implementation

- [x] 2.1 Add a terminal presentation module with color modes and semantic styles (Priority: P0, Effort: M)
- [x] 2.2 Add global `--color` option and configure presentation before command execution (Priority: P0, Effort: S)
- [x] 2.3 Add interactive verification progress and colored gate/step summaries (Priority: P0, Effort: M)
- [x] 2.4 Apply semantic styling to doctor and top-level errors (Priority: P1, Effort: S)

## 3. Verification and Documentation

- [x] 3.1 Add CLI tests for forced and disabled color output (Priority: P0, Effort: S)
- [x] 3.2 Run cargo fmt, cargo clippy, and cargo test (Priority: P0, Effort: M) - Verified with `TMPDIR=/tmp`; 81 tests pass and Clippy has zero warnings.
- [x] 3.3 Update the ADR index and user documentation (Priority: P1, Effort: S)
