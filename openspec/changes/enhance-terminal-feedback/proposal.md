# Proposal: Enhance Terminal Feedback

## Why

Verification can run several gates and configured commands without showing overall progress, and its plain status markers are easy to miss. Developers need clearer interactive feedback without damaging CI logs, pipes, or existing JSON output.

## What Changes

- Add terminal-aware colored status output and global `--color auto|always|never` control.
- Add a dynamic progress bar for interactive `verify`, including gates and selected steps.
- Keep redirected and JSON output plain and stable.

## Goals

- Make pass, warning, failure, and error states visually distinct in a terminal.
- Show completed and total verification stages while verification runs.
- Preserve non-interactive and JSON compatibility.

## Non-goals

- Changing verification ordering, exit codes, report schemas, or configuration format.
- Streaming child-command output or defining a machine-readable progress protocol.

## Success Metrics

- `verify` renders progress on an interactive terminal and a final summary on completion.
- `--color always` emits ANSI styling; `--color never` and `NO_COLOR` emit none.
- Existing tests, formatting, and Clippy remain clean.

## Impact and Risk

Affected code is limited to CLI argument parsing and human-facing output paths. Risk is **Low**: workflow execution and report generation are unchanged. The rollback is a revert of this change; plain output remains the fallback behavior.

Related decision: [ADR-0022](../../../docs/adr/0022-terminal-feedback.md).
