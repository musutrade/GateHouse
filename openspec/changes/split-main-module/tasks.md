# Implementation Tasks: Split the CLI Entry Module

## 1. Design and Boundaries

- [x] 1.1 Record the decomposition decision in ADR-0015 (Priority: P1, Effort: S)
- [x] 1.2 Define private CLI schema, application, dispatch, and output modules (Priority: P1, Effort: S)

## 2. Extraction

- [x] 2.1 Keep startup wiring and exit handling in `main.rs` (Priority: P0, Effort: S)
- [x] 2.2 Extract Clap models and scope conversion into `cli.rs` (Priority: P0, Effort: M)
- [x] 2.3 Extract early commands and project lifecycle into `app/mod.rs` (Priority: P0, Effort: M)
- [x] 2.4 Extract discovered-project dispatch into `app/commands.rs` (Priority: P0, Effort: L)
- [x] 2.5 Extract scope output into `app/output.rs` (Priority: P1, Effort: S)

## 3. Verification

- [x] 3.1 Run the complete nextest suite: 81/81 tests passed (Priority: P0, Effort: M)
- [x] 3.2 Run format, Clippy, and rustdoc (Priority: P0, Effort: S)
- [x] 3.3 Run `git diff --check` and strict OpenSpec validation (Priority: P0, Effort: S)
- [x] 3.4 Confirm no CLI, output, exit-code, or error-format contract changed (Priority: P1, Effort: S)
