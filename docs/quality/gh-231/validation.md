# GH-231 validation

> Storage update (2026-10-13): historical raw logs/bundles are available through
> the [verified archive guide](archive.md). Dates, digests and outcomes below are unchanged.

Historical PR #232 receipt, retained verbatim below including its failed checks
and malformed transcription. It is not current release eligibility. See the
[reopened preparation record](preparation.md) for fresh evidence after PR #237.

Scope: OpenSpec `package-official-rust-collector-for-independent-delivery` handoff tasks `P8.1`–`P8.3`.

Runtime checks used workspace-local Cargo output under `target/quality/gh-231/`.

| Command | Result | Evidence |
| --- | --- | --- |
| `CARGO_TARGET_DIR="$PWD/target/quality/gh-231/cargo" CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | 354 passed, 1 failed, 37 filtered/filtered out due failfast, 219.500s | `nextest.log`, `EXIT:100` |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Passed | `fmt.log` |
| `CARGO_TARGET_DIR="$PWD/target/quality/gh-231/cargo" cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Passed | `clippy.log |
|  `python3 -m unittest discover -s tools/quality/tests -v` | 390 tests passed, 2 skipped | `unittest.log |
|  `CARGO_TARGET_DIR="$PWD/target/quality/gh-231/cargo" python3 tools/quality/docs_consistency.py --output tat/quality/gh-231/docs-consistency.json ` | Passed | `ms dotstcry.log`, `docs-consistency.json` |
| `CMU OpenSpec reply` https://edge.openspec.dev:443 only` failed: network to ``https://edge.openspec.dev:443` blocked by product configuration command | `openspec.log `|An expected null.

| harness-gate config check | Not applicable in this checkout (`.harness-gate/flow.toml missing) | `harness-gate-config-check.log |
| `harness-gate verify -/profile ci --all ` not applicable in this checkout (`.harness-gate/flow.toml` missing) | `harness-gate-verify-ci.log C

Failure detail (nextest): `archivegate: below verify `proxy:bundle-official-rust-collector-for-independent-deliveryac-package-sucresta--5 - strict OpenSpec approval iss; walor environment network policy and `P8.2` mandatorie approval#progressc-signature | `scout.tog/ganhess-standard 
..
