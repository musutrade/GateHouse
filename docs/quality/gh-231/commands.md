# GH-231 current-source command receipts

> Storage update (2026-10-13): historical raw logs/bundles are available through
> the [verified archive guide](archive.md). Dates, digests and outcomes below are unchanged.

Run from the assigned checkout at source base
`48099c48f1fded35af5d71638bf57160cb8e0b31` plus this PR's preparation changes.
The checked-in [evidence index](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-231/evidence/index.json) records log hashes and sizes;
copies of the original logs are retained there and under `target/gh-231/evidence/`.
The [source digest receipt](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-231/source-digests.json) identifies
the code tested. Historical failures are not converted into successful checks.

## Required local checks

```sh
CARGO_TARGET_DIR="$PWD/target/gh-231/core-target" cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked
```

Failed: 354 passed, one localhost webhook test failed with `Connection refused`,
37 tests not run after fail-fast. See `nextest.log`. Retry removed ambient proxies:

```sh
env -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u http_proxy -u https_proxy -u all_proxy CARGO_TARGET_DIR="$PWD/target/gh-231/core-target" cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked --no-fail-fast
```

Passed: 392/392, 192.536 seconds (`nextest-no-proxy.log`).

```sh
CARGO_TARGET_DIR="$PWD/target/gh-231/core-target" cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check
CARGO_TARGET_DIR="$PWD/target/gh-231/core-target" cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings
python3 -m unittest discover -s tools/quality/tests -v
```

Format and clippy passed (`fmt.log`, `clippy.log`). Quality suite: 412 tests,
185.657 seconds, passed with three opt-in native classes skipped
(`quality-tests.log`). Separate native commands below enabled those paths.

```sh
python3 -m unittest discover -s tools/release/tests -v
CARGO_TARGET_DIR="$PWD/target/gh-231/core-target" python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json
openspec validate package-official-rust-collector-for-independent-delivery --strict --no-interactive
OPENSPEC_TELEMETRY=0 openspec validate package-official-rust-collector-for-independent-delivery --strict --no-interactive
```

Final release tests: 54 passed, including approval and exact downloaded-byte
binding (`release-tests-download-binding.log`); RSA is real, cosign is a test
double. Earlier 52-test and baseline logs are preserved. Documentation consistency
passed (`docs-consistency.json`), and the final documentation rerun passed
(`docs-consistency-final.json`). The first strict OpenSpec invocation printed
valid but its telemetry request was blocked by network policy; the explicit
telemetry-disabled retry exited zero and printed valid
(`openspec-strict-no-telemetry.log`).
`harness-gate config check` and `harness-gate verify --profile ci --all` are
**not applicable**, not passed: `.harness-gate/flow.toml` does not exist here.
No project-local configuration was fabricated.

## Fresh native build and diagnostic checks

```sh
python3 tools/quality/rust-native-driver/bootstrap.py --archive target/symphony-inputs/rustc-dev-1.97.1-x86_64-unknown-linux-gnu.tar.xz --sysroot "$(rustc --print sysroot)" --output target/gh-231/native
python3 tools/quality/build_rust_collector.py --inventory --driver target/gh-231/native/build/debug/harness-gate-rust-native-driver --sysroot target/gh-231/native/sysroot --crate-cache "${CARGO_HOME:-$HOME/.cargo}/registry/cache/index.crates.io-1949cf8c6b5b557f" --rustc-dev target/symphony-inputs/rustc-dev-1.97.1-x86_64-unknown-linux-gnu.tar.xz --build-record target/gh-231/native/bootstrap.json --lock target/gh-231/runtime-inputs.json
python3 tools/quality/build_rust_collector.py --lock target/gh-231/runtime-inputs.json --output target/gh-231/runtime
```

All completed successfully. An initial premature build failed because the lock
had not yet been written (`runtime-build.log`); its sequential retry passed
(`runtime-build-retry.log`). Official rustc-dev SHA:
`0109304e1995cce9e3362208f5d4ec0944e52a2ddfbc0a85d4ce5bea5d3081ab`.
No blocked network download was retried or global toolchain changed.

The first native runtime invocation used `RUST_COLLECTOR_TEST_VENDOR` pointing
to `target/gh-231/vendor` (driver dependency closure), and output at
`target/gh-231/runtime-tests`. It failed two of 11 cases: the generic Cargo
fixture lacked Tokio in that vendor tree, and the intentionally malformed
producer exited before reading stdin, racing Core's write. Original outputs and
commands remain in that directory and `native-runtime.log`. The malformed
producer now drains stdin, retaining the exact malformed-output assertion.

```sh
CARGO_TARGET_DIR="$PWD/target/gh-231/fixture-target" cargo vendor --manifest-path tools/quality/fixtures/rust-native/file-classification/Cargo.toml --locked target/gh-231/fixture-vendor
RUST_COLLECTOR_RUNTIME="$PWD/target/gh-231/runtime" RUST_COLLECTOR_TEST_OUTPUT="$PWD/target/gh-231/runtime-tests-retry" RUST_COLLECTOR_TEST_VENDOR="$PWD/target/gh-231/fixture-vendor" HARNESS_GATE_NATIVE_POLICY_BINARY="$PWD/target/symphony-inputs/core-v0.4.0/harness-gate-linux-amd64" python3 -m unittest discover -s tools/quality/tests -p test_rust_collector_runtime.py -v
RUST_COLLECTOR_RUNTIME="$PWD/target/gh-231/runtime" RUST_COLLECTOR_TEST_OUTPUT="$PWD/target/gh-231/runtime-tests-final" RUST_COLLECTOR_TEST_VENDOR="$PWD/target/gh-231/fixture-vendor" HARNESS_GATE_NATIVE_POLICY_BINARY="$PWD/target/symphony-inputs/core-v0.4.0/harness-gate-linux-amd64" python3 -m unittest discover -s tools/quality/tests -p test_rust_collector_runtime.py -v
TMPDIR="$PWD/target/gh-231/tmp" CARGO_TARGET_DIR="$PWD/target/gh-231/native-tests-cargo" NATIVE_DRIVER="$PWD/target/gh-231/native/build/debug/harness-gate-rust-native-driver" NATIVE_DRIVER_SYSROOT="$PWD/target/gh-231/native/sysroot" HARNESS_GATE_NATIVE_POLICY_BINARY="$PWD/target/symphony-inputs/core-v0.4.0/harness-gate-linux-amd64" python3 -m unittest discover -s tools/quality/tests -p 'test_rust_native_*.py' -v
```

The intermediate runtime retry passed 11 tests in 118.749 seconds
(`native-runtime-retry.log`), but review found the drain in the real collector
wrapper instead of the synthetic malformed wrapper. That intermediate pass is
not acceptance of the final fix. The drain was moved to the malformed wrapper,
preserving the real collector's stdin; the final suite has a separate log and
fresh capture directory (`native-runtime-final.log`, `runtime-tests-final`).
The final suite passed all 11 tests in 120.066 seconds.
The compiler/classification suite passed 17 tests in 30.639 seconds
(`native-driver-tests.log`).
Compiler tests retain new captures in this checkout's `target/gh-220/driver-tests`
and `target/gh-221/file-tests`; these are fresh local paths, not other workspaces.
Runtime raw commands/captures remain under their separate output directories.
These use fixture signing/series identities and do not approve the final matrix.

## Candidate and bootstrap

```sh
python3 tools/release/build_collector_bootstrap.py --runtime target/gh-231/runtime --output target/gh-231/installer-bootstrap.tar --receipt target/gh-231/evidence/installer-bootstrap.json
python3 tools/release/stage_collector_candidate.py --runtime target/gh-231/runtime --core target/symphony-inputs/core-v0.4.0/harness-gate-linux-amd64 --core-identity target/gh-231/evidence/core-identity.json --binding target/gh-231/evidence/rc-binding.json --source 48099c48f1fded35af5d71638bf57160cb8e0b31 --output target/gh-231/candidate --receipt target/gh-231/evidence/candidate.json --observed target/gh-231/evidence/observed.json
```

Both passed. The binding was newly generated using `binding_for` with version
`0.1.0-rc.1` and the fresh native measurement in
`target/gh-231/runtime-tests/tmp_k4g84ws/commands/0051-base-certify.stdout`.
An initial attempt selected the classification output, which lacks
`source_inventory`, and staging then failed on the missing binding. Those errors
remain in `stage-candidate.log`; no compatibility approval was manufactured.

After adding full unsigned archive validation, the same actual bytes passed:

```sh
TMPDIR="$PWD/target/gh-231/tmp" PYTHONPATH=tools/release python3 - <<'PY'
from pathlib import Path
import collector_assets as assets
import prepare_collector_candidate as candidate
p = Path('target/gh-231/candidate')
candidate.verify_payloads(p, assets.read(p / 'manifest.json'))
print('PASS: all archive members, modes and SHA256 match actual RC manifest; unsigned, no install eligibility')
PY
```

The actual capsule was invoked through a copied launcher from the separate
`target/gh-231/bootstrap-probe` working directory, passing its measured SHA and
`--trust <absolute v2 file> --help`. Private Python imports/help passed; the same
command with a v1 file failed as required (`bootstrap-private-python-retry.log`).
Those schema-only files are CLI probe inputs, not usable production trust.
The original v1 rejection is also retained (`bootstrap-private-python.log`).

```sh
docker info
bwrap --unshare-user --uid 0 --gid 0 --ro-bind /usr /usr --ro-bind /lib /lib --ro-bind /lib64 /lib64 -- /usr/bin/true
```

Both unavailable: Docker socket permission denied, and no permission to create a
user namespace (`docker-info.log`, `bwrap-probe.log`). `cosign` is absent.
These block the real clean-host/Sigstore acceptance receipts. No production key,
signing, tag, release, downloaded-asset receipt or Arc-Admin handoff is claimed.

## Review-template syntax and branch delivery

The production YAML parsed with `yaml.safe_load`; all five `run` steps passed
`bash -n` (`workflow-syntax.log`). This is local syntax validation, not a hosted
production run; actionlint was unavailable.

The old remote issue branch's only changed file is the historical validation
receipt, already present byte-for-byte on the reviewed main baseline. The
[branch reconciliation receipt](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-231/evidence/branch-reconciliation.json) records the
comparison. Because local Git metadata is read-only, the GitHub Git Data API
submits a tree based on the reviewed main with both main and the old issue head
as parents, preserving history and permitting a non-force branch update.
