# GH-230 official Core continuation

> Storage update (2026-10-13): historical raw logs/bundles are available through
> the [verified archive guide](archive.md). Dates, digests and outcomes below are unchanged.

Status: incomplete P6/P7. This record supersedes the released-Core prerequisite
in earlier GH-230 diagnostics; their original failures and archives remain intact.
The remaining clean-host prerequisite is blocked in this agent environment.
No supported delivery combination, task completion, acceptance PR, or controller
handoff is declared.

## Baseline and runtime

GitHub confirmed predecessor PR #235 merged as
`e24aeb351168610c00c14032a473fad61659ec2b`, and issue #229's controller acceptance.
Latest main is `ee544690662645806cda7dd4cd2c9192566f7929` (official Core v0.4.0).
The distributed binary staged at
`target/symphony-inputs/core-v0.4.0/harness-gate-linux-amd64` matches SHA-256
`8e3df8303ca8f650d4ef768b29cfefb60ca115122a47b116245cc19e4649bbdd`.
Fresh native tests use that binary; no checkout-built binary supplies release proof.

The assigned checkout's `.git` is read-only: `git fetch origin main` failed with
`error: cannot open '.git/FETCH_HEAD': Read-only file system` (exit 255).
A copy of this checkout's metadata under `target/gh-230/continued/git-metadata`
also could not fetch because its GitHub proxy connection failed (exit 128).
Authenticated GitHub API data supplied the main commit and changed blobs instead.
The integration script verified original Git object hashes, reconstructed the
exact signed commit, and fast-forwarded the workspace-local metadata. An initial
commit reconstruction failed its hash assertion because of a signature newline;
the correction and both logs are retained. The preserved tracked diff remained
byte-identical, and untracked GH-230 files were retained.

The working files now include latest main. The effective branch metadata is at
`target/gh-230/continued/git-metadata`, branch `symphony/GH-230`; the original
read-only `.git` remains at the predecessor. Subsequent delivery operations must
reconcile this distinction; no push or handoff was attempted for unfinished work.
No other checkout was accessed.

A fresh private runtime was inventoried and assembled from the provisioned native
driver, SHA-verified official rustc-dev input, pinned host sysroot, and local crate
cache. Command logs retain exact paths. All capture and build outputs are within
this assigned workspace. These are local diagnostics, not a clean-host install.

## Remaining environment failure

The early operator-wrapper probe was:

```bash
bash target/gh-230/operator-acceptance/run-container.sh \
  target/gh-230/operator-native-runtime target/gh-230/continued/probe \
  /released-core/harness-gate-linux-amd64 --version
```

It exited 1 with:

```text
permission denied while trying to connect to the docker API at unix:///var/run/docker.sock
```

Host Docker and ptrace successes in the operator recovery record do not establish
access from this sandbox. This run did not retry the blocked network download,
repeat the obsolete Core v0.3.7 probe, or bypass the Docker/ptrace restriction.
Clean-host capture, independently observed producer launches, complete acceptance
negatives, relocation, cold/warm setup and peak-disk measurements remain unproven.
The reviewed delivery matrix remains empty; installed `collect` still fails closed.

## Fresh validation

The native command recorder now gives every invocation a unique sequence number
and retains stdin, output, exit code, and elapsed time. Repeated certification
previously overwrote logs with the same name. These are direct command records,
not a count of descendant producers. A test description also no longer assumes
the supplied Core is checkout-built.

| Command | Actual result |
| --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | 392 passed, zero skipped; exit 0 |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | exit 0 |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | exit 0 |
| `python3 -m unittest discover -s tools/quality/tests -v` | initial run: 433 tests, 432 passed, one error, zero skipped; exit 1. Corrected full rerun: 433 passed, zero skipped; exit 0 |
| `python3 -m unittest test_rust_collector_runtime.StandaloneNativeTests.test_private_cargo_capture_preserves_incomplete_classification -v` | corrected-input rerun: one passed; exit 0 |
| `openspec validate package-official-rust-collector-for-independent-delivery --strict --no-interactive` | exit 0, including after documentation edits |
| `git --git-dir=target/gh-230/continued/git-metadata --work-tree=. diff --check` | exit 0 |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | exit 0; engineering policy, schemas, examples and links pass |

The Python discovery error was `KeyError: 'RUST_COLLECTOR_TEST_VENDOR'` in the
private Cargo test. Offline `cargo vendor --manifest-path
tools/quality/fixtures/rust-native/file-classification/Cargo.toml --locked --offline
target/gh-230/continued/vendor` succeeded. Supplying that explicit vendor directory
fixed the targeted test, which passed in 12.286 seconds including its fresh captures.
The original full-suite failure is retained; it is not reported as a green run.
A subsequent full discovery with the corrected vendor setting passed all 433 tests
with zero skips (278.824 seconds wall time). It used the same official Core v0.4.0
binary and assembled private runtime, and created fresh native captures. No
production source changed between the failed discovery and corrected full rerun.

The other seven standalone native tests passed in discovery, including complete
private re-export, exact low-coverage CRAP 56 facts and actual Core decisions,
all five unavailable capability states, stale anchors, malformed requests,
authenticated invocation rejection and nonce replay. Unavailable capability
mutations are synthetic negatives over fresh native measurements. They establish
neither successful installed generic collection nor producer counts.

Inventory and runtime assembly took 23.650 and 10.732 seconds respectively on
this existing host. The local runtime tar is 996,782,080 bytes with SHA-256
`8d0cab229443f1b86c8362c3037b37f08926be9d77ac99cd2e1eff9c1c5ed7ab`.
These timings and bytes are diagnostics, not clean-host cold/warm or peak-disk
acceptance measurements. The complete runtime tar remains workspace-local.

[Exact validation commands and environments](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/core-0.4.0-checks.json),
[original validation and failure logs](archive.md), and
[validation archive receipt](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/core-0.4.0-validation-archive.json) retain this run.
[Fresh native evidence and distributed Core bytes](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/core-0.4.0-native-evidence.tar.gz.parts/manifest.json)
contain 1,515 files, including original binaries and profiles for both standalone
test runs and the new native driver/classification fixtures. Every archived file
was read back and checked against its
[SHA-256 inventory](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/core-0.4.0-native-inventory.json); the
[native archive receipt](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/core-0.4.0-native-archive.json) records 90,491,981 compressed
bytes and its digest. This archive does not substitute for complete delivery
package retention or clean-host certification.

The corrected full discovery is retained separately in
[its evidence archive](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/core-0.4.0-python-final-evidence.tar.gz.parts/manifest.json), including exact
command/environment/stdout/stderr and the three fresh native test directories
with original binaries and profiles. Its
[inventory](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/core-0.4.0-python-final-inventory.json) and
[receipt](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/core-0.4.0-python-final-archive.json) record every member's SHA-256 and
successful read-back verification. Earlier evidence is unchanged. This closes
the local Python validation gap only; P6/P7 remain incomplete and the existing
container denial was not retried.

The [documentation consistency record](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/core-0.4.0-docs-consistency.json) retains
its command, output and report. Its automatic `commit` field reads the original
read-only `.git` at the predecessor, while the tested working files include main
as described above. This is uncommitted diagnostic evidence, not acceptance of
a pushed commit.

Project-local `harness-gate config check` and `harness-gate verify --profile ci --all`
are not applicable because `.harness-gate/flow.toml` is absent. Required hosted CI
has not run for this unfinished change.

All P6/P7 checkboxes remain open. Successful fixture diagnostics do not certify
Arc-Admin. No policy, baseline, business test, release, or #215 action was taken.
