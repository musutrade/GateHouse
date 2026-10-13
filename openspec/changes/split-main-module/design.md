# Design: Split the CLI Entry Module

## Module Layout

```text
src/main.rs
  module wiring, signal installation, result-to-exit-code handling
src/cli.rs
  Clap command/config/scope argument models and conversions
src/app/mod.rs
  early command handling, project discovery/preparation, dispatch entry
src/app/commands.rs
  commands operating on a discovered Project
src/app/output.rs
  human-readable scope rendering
```

The existing binary-level command surface remains unchanged. Implementation
modules are private and communicate through crate-internal types.

## Behavior Preservation

The extraction must preserve:

- Command names, visible aliases, help text, defaults, global flags, and
  argument conflict rules.
- Early handling of `init`, `presets`, and config migration before project
  discovery.
- Project discovery/preparation order and all command dispatch branches.
- Human-readable output, JSON output, error-code formatting, and process exit
  codes.
- Existing public-in-practice module entry points and all current test behavior.

## Verification

Run the complete nextest suite, format check, Clippy with all targets and
features, rustdoc, `git diff --check`, and strict OpenSpec validation. Inspect
the diff for changed Clap attributes, branch ordering, output strings, and
exit/error handling.

## Rollback

Revert the single refactoring commit. No configuration or generated artifact
migration is required.
