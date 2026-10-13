# Fixture ownership

This note records the reviewed owner of every fixture used for quality
decisions. It implements decision D5 of
[the core/plugin split](../../openspec/changes/split-core-and-plugins/design.md):
generic decision, invalid-evidence and timeout/replay fixtures stay with core;
only collector-specific fixtures go to GateProbes.

| Fixture | Owner | Rationale |
| --- | --- | --- |
| `tools/quality/fixtures/adapter` | core | Host adapter protocol failure modes |
| `tools/quality/fixtures/benchmark` | core | Core CI benchmark worker |
| `tools/quality/fixtures/capability` | core | Generic capability request/availability |
| `tools/quality/fixtures/ci-topology.json` | core | Core CI topology evidence |
| `tools/quality/fixtures/collectors` | core | Synthetic generic transport cases (no ecosystem toolchain) |
| `tools/quality/fixtures/complexity` | core | Complexity analyzer control subset |
| `tools/quality/fixtures/contracts` | core | Synthetic cross-component contract gates |
| `tools/quality/fixtures/core-typescript-policy.json` | core | Exact normalized policy/preset facts proven against the original Angular/V8 producer; source/artifact bytes come from verified immutable generic evidence data |
| `tools/quality/fixtures/generic-core` | core | Frozen decision-engine compatibility corpus |
| `tools/quality/fixtures/harness-evidence` | core | Generic normalized evidence |
| `tools/quality/fixtures/json-results` | core | Strict JSON result helpers |
| `tools/quality/fixtures/policy` | core | Generic policy examples |
| `tools/quality/fixtures/project-model` | core | Synthetic project identity fixtures |
| `tools/quality/fixtures/risk` | core | Bounded function-risk mapping |
| `tools/quality/fixtures/rust-macro-observation` | plugin | Rust macro collection measurement |
| `tools/quality/fixtures/rust-native` | plugin | Rust native collector inputs |
| `tools/quality/fixtures/rust-reference` | core | Frozen historical equivalence window and accepted baseline (D11) |
| `tools/quality/fixtures/typescript-angular` | plugin | TypeScript/Angular collector inputs |

The `rust-reference` directory stays in core under D11. Its
`equivalence-window.json` and `history.json` are inputs to
`rust_equivalence.py` and `tests/test_rust_reference.py`; the history manifest
also references `phase-1-accepted-b92755f.json` in that directory. These replay
existing evidence and preserve historical decision compatibility without
collecting new measurements. The candidate archives under `docs/quality/gh-96`
and `docs/quality/gh-97` also remain in core. Engineering-policy semantics are
unchanged.

The decision fixtures that core embeds at compile time were relocated out of
`tools/quality/fixtures/` into the core crate so core no longer reaches into the
plugin fixture tree:

| Relocated fixture | Owner |
| --- | --- |
| `tools/harness-gate/fixtures/workflow` | core |
| `tools/harness-gate/fixtures/v1-flow.toml` | core |

The machine-readable manifest is
[`fixture-ownership.json`](fixture-ownership.json). It is enforced by
`tools/quality/tests/test_fixture_ownership.py`, which checks that every
fixture entry is classified exactly once and that no core Rust source or test
references a plugin fixture path. It also checks the literal fixture inputs of
the two frozen Python equivalence consumers above, requiring each input to
exist and belong to core.

The ecosystem preset snapshots under
`tools/harness-gate/fixtures/preset-compat` are also Core-owned compatibility
inputs. Current templates live in GateProbes; the snapshots have an exact
inventory, original commit/path mapping and digest checks. See the
[preset consumer review](preset-externalization.md).
