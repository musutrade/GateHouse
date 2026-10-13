# ADR 0015: Decompose the CLI Entry Module by Responsibility

## Status

**Accepted** - 2026-08-27

## Context

`tools/harness-gate/src/main.rs` combined Clap argument definitions, scope
argument conversion, early commands, project discovery, command dispatch,
configuration output, scope rendering, signal setup, and exit handling in one
412-line module. This made changes to the CLI surface and application behavior
harder to review independently.

## Decision

Split CLI implementation into private submodules:

- `main`: module wiring, signal installation, and process exit handling.
- `cli`: Clap command, config action, and scope argument models.
- `app/mod`: early command handling, project discovery, and dispatch entry.
- `app/commands`: commands that operate on a discovered project.
- `app/output`: human-readable scope output.

Keep all existing command names, aliases, flags, argument conflicts, output
text, exit codes, error-code formatting, project preparation, and command
behavior unchanged. Child modules remain private implementation details.

## Consequences

- CLI schema changes are isolated in `cli`.
- Project lifecycle and command behavior are separated from process startup.
- Output formatting has a focused ownership boundary.
- Cross-command behavior changes may touch `app/commands` and a domain module.

## Alternatives Considered

- Leave the entry module monolithic: rejected because CLI schema and command
  execution have distinct ownership and review risk.
- Create one module per CLI subcommand: rejected because it would add many
  small files without reducing the central dispatch concerns.
- Introduce a command trait or framework: rejected because the binary crate
  does not need a public extension API.

## Related

- [ADR-0014](0014-preset-module-decomposition.md)
- [OpenSpec: split-main-module](../../openspec/changes/split-main-module/proposal.md)
