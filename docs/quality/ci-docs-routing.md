# Documentation-only CI routing

This note records how the core repository routes pure documentation changes and
how the fixed aggregate evaluates the selection. It accompanies the frozen
[CI topology contract](../../tools/quality/fixtures/ci-topology.json) and does
not change gate names, thresholds, measurement series or release authority.

## Classification

The always-started `classify` job runs
`python3 tools/quality/ci_quality.py classify --base-sha <before> --head-sha
<after>` and writes `run_full`, `docs_only` and `files` outputs.

A path is a pure document only when it is prose markdown:

- a root-level `*.md` file, or
- a `docs/**/*.md` file **except** `docs/dogfood/**`, `docs/benchmarks/**` and
  `docs/quality/**`.

Those excluded trees are test fixtures, retained records, specifications and
ownership manifests — not documentation. Protocol files (`schema/**`), presets,
`tools/**`, `.github/**`, install entries and every non-markdown file also keep
the full change class. Renames classify both the source and target path.

`docs_only` is true only when the change set is non-empty and every path is a
pure document. Unknown paths, an empty diff, an unresolvable base or any
classifier error select the conservative full set; a classifier failure fails
the aggregate.

## Execution

- The `docs-consistency` job always runs. With `run_full=true` it installs Rust
  and runs the complete checker; with `run_full=false` it runs
  `docs_consistency.py --pure`, which performs link, heading, policy-anchor and
  sandbox-wording checks without invoking Cargo.
- Every other required job is gated by
  `needs.classify.outputs.run_full == 'true'` (push-only jobs additionally keep
  their `github.event_name == 'push'` condition). No required workflow uses
  `paths`/`paths-ignore`; job-level selection always creates a check run, so a
  required check can never stay Pending.
- The aggregate receives `RUN_FULL`. Full mode requires every event-applicable
  child to succeed. Documentation-only mode requires `classify` and
  `docs-consistency` to succeed and accepts a skipped heavy child, but a heavy
  child that ran and failed still fails the aggregate. A missing or invalid
  `RUN_FULL` falls back to full mode, so a failed classifier cannot pass.

## Independent workflows

`native-collector-release.yml` was retired from active workflows on 2026-10-12
(archived at `retired/workflows/native-collector-release.yml`, restore commit
`6aaa2342`, ownership record in `release-ownership.json`): GateProbes publishes
the native collector, so no PR or `main` push starts that four-platform build
from current main anymore. This does not prohibit creating tags, and a tag that
points at a historical commit still runs the workflow as it existed at that
commit. Historical releases, signatures and tags are retained. `rust-generic-shadow.yml` keeps its opt-in condition.
`rust-measure-native.yml` was retired from active workflows on 2026-10-12 (archived at
`retired/workflows/rust-measure-native.yml`, restore commit `4ec65a76`): core acceptance uses the pinned Linux
amd64 measurer release, so no PR or `main` push starts the Windows/macOS analyzer build from core source.

## Measured routing

The documentation-only and full paths are measured on the hosted runs of the
change that introduced this routing; timings are observations, not guarantees.
Full code changes continue to require the complete set of Linux, macOS and
Windows jobs, coverage, risk, contracts and release governance checks.
