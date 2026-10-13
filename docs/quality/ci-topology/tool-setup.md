# Pinned CI tool setup

OpenSpec `optimize-ci-execution-topology` tasks 2.1–2.3 implement design D3,
under the unchanged [Engineering Policy](../../engineering-policy.md) and
[frozen assurance contract](baseline.md). No new policy delta or ADR is needed.

## Installation contract

The transparent [composite action](../../../.github/actions/install-ci-tool/action.yml)
uses [one version map and verifier](../../../.github/actions/install-ci-tool/tool.sh):

| Tool | Version | Consumers |
| --- | --- | --- |
| cargo-nextest | 0.9.143 | Linux/macOS/Windows full tests, quality collection, push benchmarks, scheduled/manual baseline refresh |
| cargo-llvm-cov | 0.9.0 | Quality collection; existing coverage version retained |
| cargo-audit | 0.22.2 | Required security audit and release governance audit |
| cargo-tarpaulin | 0.37.5 | Push-only Code Coverage; unchanged LLVM engine and Cobertura XML output |

Nextest and audit previously floated. These pins match the available local
validation tools; downloaded Linux releases independently passed the verifier.
No coverage/CRAP series identifiers, collector commands, thresholds, supported
platforms, event conditions, release authority, or aggregate behavior change.
Both audit commands retain `--deny warnings` and their original lockfile scope.

The installer is pinned to
[`taiki-e/install-action` 183e4297cca2404691e9380e1307288dced5c82a](https://github.com/taiki-e/install-action/tree/183e4297cca2404691e9380e1307288dced5c82a).
Its immutable manifests supply release URLs and SHA-256 checksums. Checksums
are explicitly enabled. Supported releases are downloaded on every invocation,
overwriting any old executable restored by the existing Cargo cache. The
ineffective standalone audit cache was removed. No new binary cache or mutable
artifact authority is introduced; broader Cargo cache changes remain task 3.

If a version/platform is absent from that installer's manifests, explicit
`fallback: cargo-install` invokes `cargo install --locked tool@version` from the
crate registry. Download or checksum failures stop installation; they do not
silently select another binary. Installation failure or failed/missing/wrong
`cargo <subcommand> --version` evidence fails the job before its gate can run.
There is no `continue-on-error` or cache-hit condition. Output retains the full
effective version, including nextest build metadata. Audit 0.22.2 reports
`cargo-audit-audit`, and tarpaulin 0.37.5 reports `cargo-tarpaulin-tarpaulin`;
the verifier accepts those releases' names explicitly without relaxing version
matching or allowing those aliases for other tools.

To diagnose failures, expand the named resolve/install/verify steps. The
installer reports the selected platform, release, checksum verification and
fallback reason. A version mismatch reports expected and actual output.
When updating pins, review the installer commit and platform manifest hashes,
update the version contract tests, and require native hosted tests. Nextest's
macOS asset at this pin is x86_64; the installer supports its existing macOS
architecture fallback. Native macOS/Windows job execution remains mandatory.

## GH-291: tarpaulin setup extension

The preceding GH-165 optimization intentionally left tarpaulin outside its
scope. GH-291 removes that remaining unconditional source rebuild by adopting
the same installation and verification contract. The successful pre-change
[Code Coverage job](https://github.com/musutrade/Harness-Gate/actions/runs/37411168218/job/112099699928)
installed `cargo-tarpaulin v0.37.5` from source on 2026-10-06, spending about
118 seconds on that installation. Its retained result was 87.99% coverage
(10248/11647 lines). Pinning 0.37.5 preserves that effective release; this is
not a tool upgrade or a coverage-series/baseline reset.

The previous installer commit `d438492cf8a250514fa2d34b30bc3c0dc37c65ff`
only included tarpaulin through 0.37.2. The new immutable installer revision
(v2.87.25) adds support for the required release. Comparing both revisions
confirmed byte-identical `main.sh` and `action.yml`, plus identical version
entries and URL templates for the existing nextest 0.9.143, llvm-cov 0.9.0,
and audit 0.22.2 pins. Their versions, binaries, platform selection, checksum
behavior and locked source fallback are unchanged.

The official [tarpaulin 0.37.5 release](https://github.com/xd009642/tarpaulin/releases/tag/0.37.5)
and the installer's
[immutable tarpaulin manifest](https://github.com/taiki-e/install-action/blob/183e4297cca2404691e9380e1307288dced5c82a/manifests/cargo-tarpaulin.json)
agree on the Linux x86_64 musl archive SHA-256:
`edd214e46aedd1692bf8601f9754da5c6ade83da78f73d291f5dd611c940b590`.
The actual pinned installer downloaded and checked that asset locally, and
the installed binary passed the repository's Cargo-dispatched version verifier.
No new cache is introduced. The tarpaulin job retains its separate instrumented
target, disabled target cache, push-only condition, exact `--engine llvm`
coverage command, timeout, XML path and existing Codecov transport behavior.
All required checks, matrices, warm samples, thresholds, coverage/CRAP semantics,
evidence requirements and release authority are unchanged; no policy delta is
proposed.

Local regression coverage freezes the ordered resolve/install/verify steps,
exact installer identity, checksum setting and locked fallback selection, and
checks tarpaulin's effective name/version, error status and consuming command.
An isolated smoke test additionally executed the actual pinned upstream
installer with controlled Cargo/download boundaries:

| Scenario | Observed outcome |
| --- | --- |
| Unsupported platform | Invoked exactly `cargo install --locked cargo-tarpaulin@0.37.5`; successful fixture version then passed the required verifier |
| Source fallback failure | Installer returned 42; verification was not run |
| Source fallback reports 0.37.4 | Installation returned 0; required verifier failed |
| Corrupt archive | SHA-256 verification failed; no source fallback or gate execution |
| Failed download | Installer failed; no source fallback or gate execution |

The fallback smoke exercises control flow with a Cargo stub, not a second
actual source compilation. The earlier documented
`target/quality/issue291-tool-setup/` raw directory could not be located.
The new 2026-10-08 five-case raw evidence is retained under
`/mnt/dev-ssd/dev-tmp/gh291-push-20261008/a5-installer-20261008T074405950771Z/`;
its independent audit verified 311 file hashes and 54 checks. This new evidence
does not assert consumption of the missing old logs.

Hosted Code Coverage now retains three successful post-change observations
with the same effective 0.37.5 version and LLVM/Cobertura command.
Verified prebuilt installation took 0.775141/0.443176/0.412105s, compared
with 117.790059s for the retained source installation. All 92 per-file
covered/total rows agree in the first before/after log pair; later source
changes have different denominators. This establishes same-version observed
counter parity, without controlled same-SHA equivalence or raw XML byte parity.
Codecov tokenless upload was rejected, so these logs do not prove ingestion.
See [the exact cohorts and limitations](issue291-natural-main-appendix.md).

## GH-165 hosted evidence and remaining acceptance

The retained [hosted baseline](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/hosted-baseline.json) contains three successful
pre-change runs (34303413318, 34301967575, 34291282719). Audit installs took
169/183/172 seconds; combined nextest/llvm-cov source installs took
279/236/215 seconds. Existing prebuilt nextest test setup took 1 second on
Linux/macOS and 2–3 seconds on Windows. That already-cheap prebuilt path is
retained, now with explicit version and installer identity. The unconditional
source installs in quality, audit, benchmarks, refresh and release are replaced.
Tarpaulin setup was outside tasks 2.1–2.3; its later extension is recorded above.

These are before-state measurements, not a claimed speedup. The submitting
agent must not poll CI. The controller must confirm the submitted commit's
`Required Quality Aggregate` is green and inspect hosted install steps for
setup reduction (or record a platform fallback's reason). Comparable after-state
capture remains tasks 6.1–6.2; this change does not claim whole-proposal acceptance.

## GH-165 local validation (historical)

Before editing, all 3 frozen topology tests passed, confirming the inherited
GH-164 event contract. Source inspection reproduced the ineffective audit cache:
restoring it was unconditionally followed by `cargo install --force`.

Validation logs and downloaded manifest/binary smoke evidence are retained under
`target/quality/gh-165/`. The retained [prebuilt smoke evidence](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/tool-setup-smoke.json) records that
Linux prebuilt nextest, llvm-cov and audit were fetched
from the immutable installer's manifest URLs, checked against its SHA-256 hashes,
and executed through the same Cargo version verifier (all exit 0). This verifies
the Linux assets locally; hosted action execution and other OSes remain CI work.
Regression tests cover blank, missing, failing, wrong-name, wrong-version and
prerelease evidence, unknown tool selection, explicit pins, installer integrity
settings and workflow audit semantics.

The required Rust and documentation commands use
`CARGO_TARGET_DIR="$PWD/target/gh-165-build"`, because the inherited target
`/home/gem/cargo-target` is outside the permitted workspace. No other checkout
or project configuration is used. Command results are recorded below.

`harness-gate config check` and `harness-gate verify --profile ci --all` are
**not applicable**: this checkout contains no `.harness-gate/flow.toml` declaring
a `ci` profile. No generic config was invented.

Submission uses the GitHub Git database API with the controller-prepared HEAD
as parent because `git add` returned exit 128:
`fatal: Unable to create '/home/gem/symphony-workspaces/GH-165/.git/index.lock': Read-only file system`.
The handoff records the actual pushed SHA; local Git metadata remains unchanged.

| Command | Result |
| --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Exit 0; 332 passed, 0 skipped; workspace target override above. |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Exit 0. |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Exit 0; workspace target override above. |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Exit 0; report status pass; workspace target override above. |
| `python3 -m unittest discover -s tools/quality/tests -v` | Exit 0; 307 tests passed. |
| `python3 -m unittest discover -s tools/release/tests -v` | Exit 0; 23 tests passed. |
| `python3 -m unittest discover -s tools/quality/tests -p test_ci_tool_setup.py -v` | Exit 0; 4 tests passed. |
| `openspec validate optimize-ci-execution-topology --strict` | Exit 0. |
| `bash -n .github/actions/install-ci-tool/tool.sh` | Exit 0. |
| `git diff --check` | Exit 0. |

PyYAML parsed the action and all workflows successfully. A structural comparison
of all three edited workflows against the prepared HEAD, excluding only the
scoped tool setup steps, found identical remaining definitions (including all
job commands, conditions, matrices and aggregate dependencies).
