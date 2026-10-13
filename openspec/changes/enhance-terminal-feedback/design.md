# Design: Enhance Terminal Feedback

## Context

Harness-Gate has human-readable and JSON modes. Its configured commands write to log files, while the CLI presents gate and step results. Presentation must therefore be separate from command execution and must not alter report contents.

## Decisions

### Terminal-aware presentation module

`src/ui.rs` centralizes color selection, semantic status formatting, and progress rendering. It detects TTY capability through the standard library. `auto` enables color only for a terminal and honors `NO_COLOR`; `always` is an explicit override; `never` disables color.

### Verification progress lifecycle

Before execution, `verify` calculates its two gates plus the selected configured steps. A progress object redraws one stderr line before each stage and advances after every result. It is hidden when stderr is not interactive, so pipes and test captures retain the existing line-oriented output.

### Compatibility

JSON-producing commands do not call presentation helpers. Progress uses stderr, leaving stdout reports untouched. Existing status text and exit behavior stay unchanged after stripping styling.

## Alternatives Considered

- A third-party renderer was considered but is unnecessary for one progress line and would increase binary surface.
- Printing a line per percent was rejected because it makes CI logs noisy.

## Rollback Plan

Remove the UI module and its call sites. No persisted data or configuration migration is involved.
