# Native Angular reference fixture

Implements only OpenSpec `typescript-angular-reference-adapter` tasks 1.1–1.2
([tasks](../../../../openspec/changes/typescript-angular-reference-adapter/tasks.md),
[design](../../../../openspec/changes/typescript-angular-reference-adapter/design.md)).
This is development evidence, not a collector adapter or certification. Required
checks and generic core behavior are unchanged. ADR-0040 remains the governing
[architecture decision](../../../../docs/adr/0040-language-agnostic-evidence-policy.md).

## Frozen tools and reproduction

[toolchain.json](toolchain.json) records exact tools and measurement boundaries.
All direct npm dependencies are exact; `app/package-lock.json` (format 3) locks
transitive versions, registry URLs and integrity hashes. Node 24.18.0 and npm
11.16.0 are enforced by `.npmrc`, `package.json` and the collector. Rust/Cargo
1.97.1 and Python 3.14.4 are the recorded collection environment. The Rust
provider has no external dependencies and its own format-4 Cargo lockfile.

Angular 22.0.x supports Node 24.15+ and TypeScript 6.0.x according to the
[Angular compatibility table](https://angular.dev/reference/versions). The actual
locked build and tests establish fixture compatibility. Coverage uses the
[CLI coverage options](https://angular.dev/guide/testing/code-coverage), with an
explicit Istanbul provider in `app/vitest.config.ts`.

From the repository root, with the recorded runtimes available:

```sh
python3 tools/quality/fixtures/typescript-angular/collect.py \
  --output target/quality/angular-native-run
```

The output directory must be new and outside the fixture. The script runs
`npm ci --no-audit --no-fund` twice, compares installed package inventories and
checks the original lockfile digest. It then checks generated-client drift,
runs locked Rust tests/build, builds Angular production output and runs Angular
tests with coverage against a live loopback Rust provider. Each subprocess has
an argv array, working directory, exit status and combined stdout/stderr log.
Writable caches are isolated under the output directory and removed on success.
Each collection has a 600-second timeout per command.

The manifest records the checkout revision, dirty working tree and source
snapshot digests: a dirty fixture is identified by its retained bytes, not
misrepresented as the recorded Git commit. It is a single native run, not an
accepted base/head comparison or a `harness-evidence/v1` envelope.

## Fixture cases

| Case | Source |
| --- | --- |
| External template, tested rendered output | `app/src/app/app.html`, `app.spec.ts` |
| Hit and deliberately unhit branches | `app/src/app/pricing.ts`, `pricing.spec.ts` |
| Same `quote` method on distinct classes | `standard-quote.ts`, `priority-quote.ts` |
| Lazy route and real navigation test | `app.routes.ts`, `details/`, `app.spec.ts` |
| Generated fetch client | `app/src/app/generated/`, `quote-client.ts` |
| Live provider consumption | `quote-client.spec.ts`, `provider/src/main.rs` |
| Generator input served by provider | `provider/openapi.json` |

The app was created by Angular CLI 22.0.8:

```sh
ng new reference-app --directory tools/quality/fixtures/typescript-angular/app \
  --routing --style css --ssr=false --skip-git --skip-install \
  --interactive=false --ai-config=none
```

The service, details component and quote classes were also CLI-generated, then
edited to introduce the cases above. To intentionally update the client, run
`npm run generate:client` from `app/` after editing the provider contract, review
the resulting bytes and collect into a new directory. Collection rejects drift.
The fixture proves a real client/provider exchange; contract compatibility,
breaking-change policy and project aggregation remain task 3.3.

## Retained evidence and limits

The [evidence record](evidence/README.md) identifies retained archives and their
SHA-256 digests. The native manifest hashes every retained artifact and records
configuration digests separately. Archives contain original source/configuration
bytes, locked dependency inventories, tool logs, build bundles and source maps,
emitted test JavaScript/maps, test JSON, native Istanbul `coverage-final.json`,
summary counters, LCOV and the served OpenAPI bytes.

Generated client code is tested but excluded from coverage measurement. Native
coverage is limited to `src/app/**/*.ts` under jsdom. Template execution does not
establish template branch coverage. Browser E2E, SSR, complexity, CRAP, canonical
subject identity, source-map integrity semantics and generic policy are unproven.
Absolute filenames in raw tool output are retained without rewriting. Timestamps,
ephemeral provider ports and absolute paths can differ on replay; reproducibility
here means the exact locked package inventory and reproducible native operations,
not byte-identical timestamp-bearing reports.

The manifest starts in `failed` state and changes to `complete` only after every
required command succeeds and required native outputs exist, including both hit
and unhit pricing branches. Missing tools, timeout, nonzero exits, stale output
directory reuse, generated-client drift and missing artifacts block completion.
`complete` describes collection only; it never grants a policy pass. Collector
failure-boundary tests live in `tools/quality/tests/test_angular_fixture_collection.py`.
