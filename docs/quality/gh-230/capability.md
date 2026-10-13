# GH-230 Core capability diagnostic

> Storage update (2026-10-13): historical raw logs/bundles are available through
> the [verified archive guide](archive.md). Dates, digests and outcomes below are unchanged.

Status: incomplete P6/P7; no supported combination, acceptance PR or handoff.
This continues the [trusted invocation diagnostic](trusted-invocation.md).
All earlier diagnostics, including genuine failures, remain retained unchanged.

The opt-in `test_private_generic_projection_is_decided_by_actual_core` now tests
all five unavailable capability states through actual Rust Core evaluation.
It creates two fresh native fixture captures, certifies them through the existing
private runtime, and projects typed evidence with the installed projection code.
The supported low-coverage case fails its Core threshold with a retained head
value of 56. No Core policy or dispatch code changed.

For each negative, the test explicitly injects one capability state into a copy
of the head binding. It checks that projected evidence retains that state with
an empty metrics list, then evaluates it against the same required Core policy
and supported baseline. These are synthetic capability outcomes over fresh
native evidence, not successful native measurements of unsupported combinations.

| Injected capability | Every Core gate | Core aggregate |
| --- | --- | --- |
| `unsupported` | `unsupported` | `blocked` |
| `not_configured` | `blocked` | `blocked` |
| `not_collected` | `skipped` | `blocked` |
| `not_applicable` | `not_applicable` | `blocked` |
| `measurement_error` | `measurement_error` | `measurement_error` |

All six Core evaluations exit 1 as expected. Each input, command, output and
decision is retained. The public collector still rejects the unknown delivery
combination with matching invocation ID and empty evidence/artifacts; the test
does not infer producer counts from that empty output.

The explicit native test passed: one test, 28.419 seconds wall time. Its command
is `python3 -m unittest test_rust_collector_runtime.StandaloneNativeTests.test_private_generic_projection_is_decided_by_actual_core -v`;
the exact workspace-local runtime, Core, build and temporary paths are recorded
in the command receipt. This run used the unchanged diagnostic private runtime
and checkout-built Core, with their identities checked against the prior
inventory. Original fresh binaries, profiles, sources and capture records are
retained in the evidence archive. The full runtime package remains local and
is not included in this diagnostic archive.

Only the opt-in native test and documentation changed in this follow-up.
The prior Rust nextest/fmt/clippy and 408-test Python discovery results remain
applicable; the changed native test was run explicitly. Documentation consistency,
strict OpenSpec and whitespace results are recorded with exact commands.
The first documentation check exited 1 in generated CLI examples and schemas;
its runner omitted the explicit workspace-local Cargo and temporary-directory
environment. The checker suppresses its child-command errors, so that report
does not establish the underlying cause. The failed report is preserved alongside
the subsequent check with those paths explicitly configured.
Project-local `harness-gate config check` and `harness-gate verify --profile ci --all`
remain not applicable because `.harness-gate/flow.toml` is absent.

The [released Core prerequisite](recovery.md#released-core-prerequisite), clean
supported-host execution, successful public generic collection, observed producer
counts, complete negative acceptance matrix, cold/warm cost measurements and
durable complete package retention remain unresolved. The original release
download, namespace and tracing denials have not been retried or bypassed.
No P6/P7 checkbox is completed. Fixture evidence does not certify Arc-Admin;
no release, baseline or #215 action was taken.

[Exact commands and results](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/capability-checks.json),
[retained diagnostic bytes](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/capability-evidence.tar.gz.parts/manifest.json),
[file inventory](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/capability-inventory.json) and
[archive receipt](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/capability-archive.json) describe this follow-up.
