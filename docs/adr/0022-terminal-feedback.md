# ADR-0022: Provide Terminal-Aware CLI Feedback

## Status

**Accepted** (2026-08-27)

## Context

Long-running verification emits scattered status lines. Users cannot see overall progress while a configured command runs, and outcomes require visual scanning. Harness-Gate also runs in CI and through pipes, where escape sequences and redraws would corrupt logs and machine-readable output.

## Decision

Introduce a small internal `ui` module that owns terminal presentation:

- `verify` displays one dynamic progress bar on an interactive stderr terminal, covering its two gates and selected configured steps.
- Status markers use semantic colors: green for pass, yellow for warning, red for failure/error, and cyan for headings.
- Color defaults to `auto`, respects `NO_COLOR`, and can be overridden with global `--color auto|always|never`.
- Non-interactive streams retain stable plain-text output. JSON output remains uncolored and has no progress rendering.

The implementation uses ANSI escape sequences and the Rust standard library rather than a presentation dependency, keeping the distributed binary small and the output policy explicit.

## Consequences

- Interactive verification makes forward progress and outcomes immediately visible.
- CI logs, pipes, snapshots, and JSON contracts remain parseable.
- A single presentation layer prevents terminal-detection logic from spreading through workflow code.
