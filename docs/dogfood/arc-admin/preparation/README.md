# GH-215 integration preparation

This is a preparation receipt and implementation handoff for GH-215, not its
acceptance. Existing Arc-Admin gates, thresholds, debt/ratchet rules and Angular
CRAP unsupported status remain unchanged. GH-208 must not infer permission to
transfer authority from this preparation.

## Observed environment

The [preflight receipt](preflight.json) pins the tested source, Harness-Gate
revision, binary digest, exact command and staged configuration hashes.
Arc-Admin was checked out independently at
`9982ed556eaf997910824d7b682946147c81a16a`, matching both the frozen dogfood source
and the remote main observed during preparation. The user's original checkout
was clean at a different revision and was not switched or updated.

`cargo flow scope --all` succeeded in the isolated checkout. The imported flow,
quality configuration and packs were copied unchanged into `.harness-gate/`.
No protected arc-flow implementation or hooks were modified.

The host has Node 24.18.0, Rust 1.97.1, cargo-llvm-cov 0.9.0 and a responding
Docker daemon (29.6.2). This inventory does not certify compatibility with a
reference measurement series. The SSD had approximately 341 GiB available.
Both repositories returned zero registered self-hosted Actions runners. This
does not disable GitHub-hosted runners, but the existing Arc-Admin CI declares
`runs-on: self-hosted` for all five jobs and therefore requires an online
self-hosted runner. GitHub-hosted execution requires a workflow/environment
change, not just enabling Actions. Arc-Admin Actions
is enabled. Repository permissions report push/admin, but listing repository
secrets returns HTTP 403 with the current credential. Existing secret names or
values therefore remain unknown; this is not evidence that no secrets exist.

The default executable `/home/gem/.cargo/bin/harness-gate` reports 0.1.0 and
rejects the `quality` command. Preparation built the pinned Harness-Gate source
with `cargo build --locked --manifest-path tools/harness-gate/Cargo.toml
--bin harness-gate` and used `/home/gem/cargo-target/debug/harness-gate`
explicitly. Production jobs must build/copy their own pinned executable rather
than rely on either this mutable shared target or the old PATH installation.

## Minimal collection result

The real `quality collect` entry point was invoked; its [raw log](collection-probe.log)
and exit code 1 are retained. It stopped at the missing trusted key file, before
launching a producer. All seven configured runtime input files are absent.
This is a failed provisioning probe, **not a successful native measurement**.
No full test suite or repeated cost trial was launched after that failure.

## Implementation configuration

Use `/mnt/dev-ssd/workspaces/gh215-arc-input` as the prepared disposable input,
not the user's active checkout. Keep all temporary databases, build outputs and
collector artifacts on the SSD. For the first CI collection, a GitHub-hosted `ubuntu-latest`
runner is an available option; explicitly pin the required toolchain and use
an isolated PostgreSQL service. Do not compare its timings directly with the
previous self-hosted samples. For repeatable self-hosted trials, use a
dedicated runner label such as
`arc-admin-quality-shadow`, one job initially, read-only repository contents
permission, and no production database credentials. Register the ephemeral
runner only when the producer/provisioner is executable; unregister it after
the job. Never expose signing credentials to untrusted pull-request code.

The host provisioner owns trust. Collectors must never mint their own trusted
keys or accept baseline digests from the downloaded bundle itself. A local
pilot can use a host-owned private key outside the repository, with restrictive
permissions, and an explicit shadow-only trust store. Such a key is not
automatically a production trust anchor. For CI, choose either an existing
approved host keystore or repository/environment secrets. The latter route
requires the current GitHub credential to have Secrets read/write access for
Arc-Admin (or a maintainer to provision the secret through GitHub); repository
admin role alone did not grant this token access. Do not request broad machine
administrator permissions or place private key material in issues/logs.

Prepare these inputs using the real [collector](../../../quality-collectors.md)
and [baseline](../../../quality-baselines.md) contracts:

| Input | Owner and required contents |
| --- | --- |
| `trusted-keys.json` | Host-controlled Ed25519 public keys and key IDs; private key remains outside the workspace. |
| `backend-request.json` | Fresh signed executable-bound request for native Rust line/region coverage and CRAP. |
| `frontend-request.json` | Fresh signed request for native frontend coverage; preserve explicit unsupported CRAP. |
| `frontend-api-request.json` | Fresh signed request for native API compatibility/client drift evidence. |
| `full-state.json` | Host-discovered subjects, actual source/config hashes, tool/series identities and run context for full collection. |
| `hook-state.json` | Separate fresh hook context and scope; do not reuse full state for staged changes. |
| `full-baseline-request.json` | Independently authenticated accepted base state and manifest digest for the unique Git merge base. |

Implement the producers in the Arc-Admin integration layer using generic
command/result/evidence interfaces. The existing test commands do not yet supply
these signed envelopes; successful tests alone cannot populate metric values.
First discover real tool/selection/series identity. If it differs from the
reference series, document and approve a baseline/series migration rather than
copy reference hashes or fabricate compatible observations. A head at main with
the same merge base cannot serve as a distinct head/base acceptance pair: select
an actual integration change and measure its exact base separately.

## Ordered continuation

1. Implement native producers and host provisioning against the staged config;
   retain raw native outputs and source/selection/tool identities. Do not create
   empty runtime JSON merely to get past the missing-file error.
2. Run one complete native collection. Only after its signed transport and
   provenance validate, construct and authenticate the exact base bundle and
   evaluate the head. Collection PASS is not policy PASS.
3. Exercise missing, tampered and stale evidence plus invalid baseline cases
   against that same real integration. Retain failures; never substitute GH-205
   synthetic fixtures for these receipts.
4. Validate staged/working-tree hook behavior, environment isolation, service
   failure/cleanup and scope/hook/CI routing while keeping all 25 original gates.
5. Register the bounded runner and measure complete-quality paired workloads
   and the original multijob topology, retaining setup/cache/order overhead.
   Preserve evidence before cleaning only task-owned build/cache directories.

No production trust anchor, accepted baseline or CI secret was created during
this preparation. GH-215 remains open; no checklist item is accepted by this
receipt. GH-207 generic fixes may proceed independently, but GH-208's authority
recommendation requires GH-215's actual evidence.
