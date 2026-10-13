# Trusted quality baselines

`harness-gate quality baseline` resolves configured baseline providers independently
of [verification composition](quality-verification.md). It exports the existing evaluator's five base
inputs without changing policy, thresholds, debt or ratchet calculations.

```bash
harness-gate quality baseline --repository-root "$PWD" \
  --state /trusted/head-state.json --request /trusted/base-request.json \
  --output /tmp/new-quality-baseline
```

The output must be a new directory outside the working tree. A successful
resolution writes `project.json`, `expected.json`, `evidence.json`, `inputs.json`
and `resolution.json`, plus an isolated `source` snapshot. Supply the first three
files through `quality evaluate`'s `--base-project`, `--base-expected` and
`--base-evidence`, and the resolution's roots through `--base-source-root` and
`--base-artifact-root`. These arguments work with both direct and compiled head
evaluation. Failed materializations are removed; existing destinations are refused.

## Host and artifact contracts

The trusted host supplies head state using the [compiler contract](quality-compilation.md).
It separately authenticates the retained CI producer/run and obtains the accepted
base state and manifest SHA-256 through a trusted channel. An artifact must never
supply its own expected state or expected digest. This command validates the
local bundle; downloading artifacts, authenticating CI and scheduling collection
remain responsibilities of the host.

The strict request object is:

```json
{
  "schema": "quality-baseline-request/v1",
  "state": { "schema": "quality-trusted-state/v1" },
  "manifest": "retained/manifest.json",
  "manifest_sha256": "<externally authenticated SHA-256 of manifest bytes>"
}
```

`state` above is abbreviated: it must contain the complete accepted base compiler
state, including context, config hashes, subjects, capabilities, series and artifact
pins. The manifest path is relative to the repository; for `retained_artifact` it
must exactly match configuration. The manifest contains:

```json
{
  "schema": "quality-baseline-manifest/v1",
  "state": { "schema": "quality-trusted-state/v1" },
  "evidence": [],
  "files": { "<bundle-relative path>": "<SHA-256>" }
}
```

Here too, state and evidence are abbreviated. Manifest state must equal the
host's full expected state. `files` must be exactly the union of config files,
subject source files and artifacts (prefixed by `artifact_root`). File paths are
relative to the manifest's directory and keep their repository layout. Extra,
missing, corrupt, traversing, aliased or symlinked files fail validation. Evidence
must match its source/context, declared subject/capability/series bindings and
artifact provenance. The manifest digest also pins evidence bytes.

Base commit must equal the head context's `base_commit`, and must differ from
head commit. Target, profile and complete tool/measurement series must be compatible
with head. Base configuration is pinned separately so intentional configuration
and path changes can retain history; an artifact cannot change those accepted
base pins. Run identity must match the expected base run, never the current run.

## Providers and lineage

The Git provider resolves `reference^{commit}`, optionally computes the unique
merge base with the trusted head, and checks the resulting commit against the
host's expected base. It reads immutable tree/blob objects into the output without
checkout, index updates or worktree registration. Dirty and untracked head files
are preserved. Replacement objects are disabled. SHA-1 commit identities and
UTF-8 regular files (including executable mode) are supported; symlinks, submodules,
unsafe paths and ambiguous merge bases fail closed. The retained bundle supplies
measurements for that exact commit; its config/source bytes must match Git objects.

The retained-artifact provider reconstructs only the exact pinned file inventory.
It needs no Git checkout at the base commit. Neither provider generates favorable
replacement measurements or evaluates policy. Both preserve the core's explicit
rename/move mappings and reject invalid mappings before exposing a base.

Required missing baselines fail. Optional missing requests, manifests or unresolved
Git refs produce only `resolution.json` with `status: "unavailable"`, never head
evidence. Present corrupt/incompatible bundles fail even when optional. Existing
configuration validation requires a baseline for debt/ratchet policy, so absence
cannot silently erase historical debt.

Ecosystem names are opaque metadata. Transport validates canonical series and
typed evidence for arbitrary declared capabilities without certifying metric
semantics. Evaluation still uses the unchanged supported metric registry and fails
closed on unsupported metrics. The [baseline corpus](../tools/harness-gate/fixtures/workflow/baseline/README.md)
tests both providers with an unregistered ecosystem and custom capabilities, plus
existing Rust debt and generic rename/move lineage. See [ADR-0044](adr/0044-trusted-quality-baselines.md).
