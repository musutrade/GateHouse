# Design: Harden Gate Boundaries and Delivery Contracts

## Scanner Boundary

Working-tree scanners inspect Git-listed paths only after rejecting symlink
file entries. Audit traversal also verifies that a candidate is a regular file
below the repository root before reading it. Staged secret scanning continues
to read Git blobs, which do not follow filesystem links.

## Cancellation and Process Ownership

Unix keeps the existing session/process-group implementation. Windows uses
`taskkill /PID <pid> /T /F` to terminate the complete descendant tree, with a
direct-kill fallback when the command is unavailable. Secret and audit loops
check the shared cancellation flag between files and return a typed
cancellation error. Platform-specific tests cover descendant cleanup where
the host supports it.

## Compiled Configuration

Scope rules expose a compiled matcher cache built when the flow configuration is
loaded. Audit allowlist regexes are compiled once per rule and passed into the
scanner. Rule violations carry the originating rule name separately from the
pattern label used for display.

## Bounded Inputs and Initialization

Audit sources and working-tree or staged secret-scan inputs fail closed above
16 MiB per file. JSON Lines error extraction uses two streaming passes and
bounded context buffers instead of retaining the complete log. Preset
initialization stages all generated files, including its local `.gitignore`,
before replacing destinations; a failed commit restores existing files.

Verification explicitly drains service cleanup errors after the scheduler has
joined its workers. A cleanup failure is written as a failed report entry and
keeps the command result failed.

## User-Facing Contracts

All remediation text and headings use `harness-gate`; the installer points to
the `musutrade/Harness-Gate` release repository. The Linux textual contract is
regenerated for the current package version. Doctor file checks resolve
repository-relative paths against `Project::root`.

## Rollback

Revert the implementation and this change record together. No persistent data
format or report schema migration is required. The 16 MiB limit is an
operational boundary; repositories with larger generated inputs must exclude
or split those files rather than increasing memory use silently.
