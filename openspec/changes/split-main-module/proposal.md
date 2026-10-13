# Proposal: Split the CLI Entry Module

## Why

`tools/harness-gate/src/main.rs` grew to 412 lines and combined CLI schema,
scope conversion, early commands, project discovery, command execution, and
output rendering. Separating those concerns improves reviewability while
keeping the user-facing CLI stable.

## What Changes

- Keep process startup and exit handling in `main.rs`.
- Move Clap models and scope conversion to `cli.rs`.
- Move early command handling and project discovery to `app/mod.rs`.
- Move discovered-project command dispatch to `app/commands.rs`.
- Move scope output rendering to `app/output.rs`.
- Preserve command names, aliases, flags, output, and behavior.

## Goals

- Give CLI schema, project lifecycle, command dispatch, and output clear
  ownership.
- Keep the entry module small and focused on startup concerns.
- Preserve all argument conflicts, command behavior, output text, exit codes,
  and error-code formatting.
- Keep extracted modules private and narrowly coupled.

## Non-goals

- Changing commands, aliases, options, defaults, or argument conflicts.
- Changing project discovery/preparation or command execution semantics.
- Changing human-readable or JSON output, exit codes, or error codes.
- Adding a command framework, public library API, or new dependency.

## Success Metrics

- `main.rs` only wires modules, installs signal handlers, and maps results to
  process exit codes.
- Existing unit, CLI, and integration tests pass without behavior changes.
- Format, Clippy with `-D warnings`, rustdoc, and strict OpenSpec validation
  pass.
- No public CLI symbol, argument contract, output text, or error formatting
  changes.

## Risk Assessment

Low to medium. The extraction is internal to a binary crate; the main risks
are visibility mistakes and accidental changes to Clap metadata or dispatch
ordering. Existing CLI tests and static checks provide fast feedback, and
reverting the refactoring commit restores the previous layout.
