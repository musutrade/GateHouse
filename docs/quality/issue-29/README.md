# Issue #29: read-only verification plans

The canonical interface is verify --plan, with --json for verification-plan/v1.
See the [English contract](../../verification-plan.md), [中文](../../verification-plan.zh-CN.md)
and [JSON Schema](../../../schema/verification-plan.schema.json).
The [valid example](plan-example.json) and [invalid example](plan-invalid-example.json)
were produced by the CLI and validated against the schema. Neither contains a
verification PASS; verification_result is null.

Execution and inspection share scope detection, compiled rule matchers,
explicit-component validation, VerificationPlan::build and dependency closure.
The preview bypasses project preparation, invocation/report allocation,
services, command execution, baseline resolution, quality preparation/collection,
signed-request authentication and replay ledgers. A staged private configuration
view reads only necessary index blobs without write-tree or checkout-index.
Checkout-filtered configuration is explicitly rejected to avoid claiming a
selection based on different configuration; source filters are never executed.

| Validation | Result |
| --- | --- |
| Complete nextest suite | 520 passed, zero skipped |
| New preview integration tests | Five passed: 36 mode/policy/change scenarios, explicit/all execution-set parity, errors, deterministic/no-write behavior, indexed configuration and omitted/selected quality inputs |
| Actual plan/execution comparison | 35 dispatch/skipped-set comparisons: 33 valid scope-matrix cases plus explicit components and all; the other three unmatched=fail cases return invalid plans |
| Existing #23/#25 integration regressions | Four tests passed, retaining both rename endpoints and conditional unmatched=all fallback without forcing all steps |
| Clippy all targets, warnings denied | Passed |
| Production inventory tests | 14 passed; new production modules belong to blocking app/verify boundaries |
| Critical-path inventory tests | 18 passed; bindings refreshed, new isolated plan.readonly scenario added, mandatory policy unchanged |
| Valid/invalid CLI JSON against Draft 2020-12 schema | Passed; no reports created |
| Pure documentation consistency | Passed |

The initial broad run stopped because the task's temporary parent had mode 0775,
which the existing host replay-ledger boundary correctly rejects. The task
temporary directory was changed to 0700 and the complete suite passed. No trust
check, test, required gate, threshold, waiver or baseline was relaxed.

The final configuration-transformation checks (filter, encoding and ident) and
explicit/all parity assertions were verified again in all five preview integration tests. Staged preview checks cover configured filesystem-monitor and
source filters not being invoked, configuration filters being rejected,
and unchanged repository/index/object/request bytes. Separate parser regressions
reject unmerged entries, links, submodules, path escapes and invalid encodings.

Remaining runtime inputs are deliberately unvalidated; planning never certifies
executable availability, signatures, nonce freshness, service readiness or
measurement evidence. The CLI/configuration versions, existing scope/result
formats and Engineering Policy semantics are unchanged. Interface rollback
removes the flag/schema/projection together without resetting evidence/history.

Hosted CI first exposed two fixture portability issues: detached Git maintenance raced the unchanged-byte assertion, and a parser test used a non-canonical temporary path and Unix-only filename on Windows. Fixtures now disable their own automatic maintenance, use canonical roots and portable filenames, and retain the Unix newline/tab case. The complete byte comparison is retained. Additional partial-clone tests reject a missing config object without invoking a configured remote helper or changing Git files.
