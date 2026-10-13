# Quality setup

This preset enables `.harness-gate/quality.toml`. Review its component/source
boundaries, `.harness-gate/packs/*/policy.json`, and `capabilities.json` before
verification. Full and CI select required quality; hook is partial and never
certifies full quality. Missing required evidence fails closed.

The Rust reference pack preserves coverage >= 4/5 and CRAP <= 30 with the
accepted retained measurement series. TypeScript supports line/function coverage;
CRAP remains unsupported, with an optional diagnostic binding and no numeric
measurement. PostgreSQL is an execution service, not a quality component.
The API contract pack binds compatibility, breaking changes and generated-client
drift to a relationship in the same project aggregate.

Run `harness-gate config check` after adjusting paths. Configuration validation
does not certify a collector installation or create measurement evidence.
Provision host-owned `.harness-gate/runtime/<profile>-state.json`, trusted keys,
and fresh signed `<binding>-request.json` inputs for the chosen profile before
`harness-gate verify --profile ci --all`. Even partial hook verification needs
trusted state. Never copy retained reference evidence as evidence for your source.
The preset supplies no signing keys, signed requests, runtime state, or PASS.
Current host/pack tooling must prepare these inputs; automatic installation and
request generation are not provided by `init`.

`capabilities.json` records reference series and capability boundaries. The host
must pin the actual installed collector/toolchain/source-identity series; a
changed toolchain requires deliberate series/baseline migration, not relabeling.
Baseline defaults to `none`; there is no implicit debt waiver. To adopt debt or
no-regression policy, configure a required Git or retained-artifact baseline,
its workflow baseline request, and compatible lineage before adding ratchet rules.

Reference combinations are UX, not core architecture. Declarative ecosystem and
capability packs own these defaults. Future frontend=vue, backend=go,
database=postgres composition should use pack data with the same generic
configuration/compiler/evaluator. Database packs need quality participation only
when an explicit capability requires it. No registry/package manager is shipped.

Legacy flow-only repositories and `generic` remain opt-in. For migration,
generate a reference preset in a separate temporary directory, review and copy
quality.toml and its packs, align project identity, source roots, flow components
and profiles with the existing flow, and provision trusted inputs. `migrate` only
upgrades execution configuration; `init --force` is an explicit replacement,
not an automatic migration. Removing quality.toml restores flow-only behavior;
keep policy and retained evidence for audit and rollback review.

Details: https://github.com/musutrade/Harness-Gate/blob/main/docs/quality-presets.md
Governing policy: https://github.com/musutrade/Harness-Gate/blob/main/docs/engineering-policy.md
