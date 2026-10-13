# Issue 285: process-group cleanup and bounded source-risk certification

This change fixes early leader reap in Unix termination. It does not change the
Engineering Policy, required gates, metric definitions, thresholds, debt,
ratchet, baseline authority, or the Windows `taskkill /T /F` behavior. Processes
that deliberately leave the managed group are outside this contract.

## Termination ownership and errors

`process/command.rs` owns an isolated direct child whose PID is its PGID.
Termination requires exclusive wait/reap ownership for the entire operation.
`waitid(P_PID, WEXITED | WNOHANG | WNOWAIT)` first checks that the child remains
waitable without reaping it. A zombie leader retains its identity through TERM's
two-second grace and the final group KILL, preventing reuse before that signal.
There is no `try_wait` in that interval. An external concurrent reaper violates
the ownership assumption; WNOWAIT does not solve that separate case.

An already externally reaped child, including one whose status Rust may have
cached, produces ECHILD before any signal. Public `Child` exposes no cached-status
getter. Calling `try_wait` after ECHILD would not prove a returned status belongs
to the original child, so this path conservatively returns the error.

TERM failure does not short circuit the last group KILL attempt. The first
signal error is retained while cleanup tries to reap the direct child. One
deadline starts after the initial WNOWAIT check: `start + grace + 2 seconds`
(normally four seconds). A TERM error skips grace and only shortens that deadline
to `start + 2 seconds`. Grace sleeps at most its remaining budget. The final KILL
is attempted even if an earlier error or an expired budget exists, but never
restarts the deadline. After it, one immediate `try_wait` is allowed even when
the deadline expired; further polls use only the remaining budget. There are no
subsequent group signals. This bounds this component's waiting, not a blocked OS
system call; it cannot promise to cancel kernel calls. A failed KILL with a live leader cannot
enter an unbounded `wait`. A cleanup timeout returns the first error, or
`TimedOut` when there was no earlier error. That error does **not** assert the
leader was reaped or the group was cleaned. ESRCH from group signaling continues
to mean the group is absent. No unrelated task/capture/adapter code or its
critical-path binding changes.

## Darwin's zombie-only process-group error

Darwin's group signal can return EPERM when its group has no eligible live
member: XNU filters SZOMB before determining whether any member could be
signaled. This is also a real permissions error, so EPERM alone is never success.
See Apple's [signal implementation](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_sig.c).

Only the real Darwin `send_signal` path may examine this EPERM. Its non-reaping
observer must prove the exclusively owned leader exited. The libproc group query
uses selector `PROC_PGRP_ONLY` (2) and a fixed two-PID buffer. Success requires
exactly one PID's byte count and that PID equals the leader. Apple enumerates
live and zombie members under the same lock used to change group membership:
[proc_info.c](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/proc_info.c)
and [kern_proc.c](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_proc.c).
This establishes that the complete group contains only its anchored zombie at
the enumeration point. It makes no claim about an escaped member later rejoining.

Zero (including libproc's failure result), negative/short/nonintegral counts,
full two-slot buffers, other PIDs, any other member including another zombie,
a live leader, ECHILD and unknown observations preserve the **original** EPERM.
The guard performs one query, without retry waits or per-PID status inferences.
No query error replaces the first signal error. Injected signal functions bypass
this guard, including injected EPERM for a proven solo zombie. Processes that
leave the isolated session/group and external concurrent reapers remain outside
this contract. Official source review is not evidence of a runner's actual kernel;
the macOS fixtures and old failing tests must establish native behavior.

## Real process fixtures

The independent integration target includes the actual `command.rs`, rather
than a copy of its algorithm. On Unix, a Python fixture leader forks a descendant
in the same group. The descendant installs its TERM disposition before atomically
publishing ready PIDs/PGID. Tests inspect the descendant's real process state
before emergency cleanup; an absent process or zombie is stopped, a running or
sleeping process is a failure. The host is not the orphan descendant's reaper.
Failed process queries, stderr, malformed output and unexpected status are
errors. An empty `ps` selection with status 1 additionally requires a kernel
`kill(pid, 0)` ESRCH result; empty stdout alone never establishes absence.

`GH285_PROCESS_EVIDENCE` selects a retained fixture directory. The default is
`target/gh285-process`. Source, readiness, process-state output and CLI
stdout/stderr/status/report remain available after failures. The guard publishes
an emergency cleanup request and only directly kills its still-owned `Child`.
The living descendant responds by signaling its **own current** group, with a
30-second watchdog as a final bound. No host guard signals a saved PGID after
leader reap. Assertions run before this cleanup, so it cannot make the test pass.

Run the integration target with:

```sh
cargo test --locked --manifest-path tools/harness-gate/Cargo.toml --test process_termination_test
```

Unix tests are:

- `leader_term_exit_still_kills_same_group_term_resistant_descendant`
- `normal_group_and_term_ignoring_leader_are_reaped`
- `waitable_exit_is_reaped_but_cached_or_external_reap_is_echild_without_signal`
- `first_signal_error_survives_cleanup_and_failed_kill_is_bounded`
- `signal_callbacks_do_not_restart_the_shared_cleanup_deadline`
- `independent_cli_timeout_and_cancellation_stop_term_resistant_descendant`

The last test launches separate real CLI processes for cancellation and timeout,
preserves the existing exit-1/report-FAIL contract and checks the corresponding
step flag. It never poisons the test runner's process-global cancellation state.
macOS additionally runs `darwin_guard_requires_a_complete_unique_waitable_leader`.
It checks real live-leader and exited-leader/live-descendant groups. A keeper in
a different group retains a zombie member of the original group, so a full
query remains a refusal rather than being guessed empty. Unknown, malformed,
truncated and wrong-PID query results and observer errors are separate synthetic
API negatives. The solo exited leader is a real positive, while an injected
EPERM still propagates and reaps it; the subsequent ECHILD path cannot query.
The existing waitable-exit test separately requires successful real termination.

Windows has `windows_taskkill_termination_preserves_direct_child_behavior`;
that test and the unchanged production bytes are a Windows-specific claim, not
Unix native execution evidence.

## Source selection and actual ownership

The certified inventory adds only `process/command.rs`. Selection identity is
`gh285-process-group/1`. Analyzer `harness-gate-rust-measure/0.3.1`, rule
`mccabe-rust-3/1`, instrumentation `closure-black-box/1`, mapping
`insertions-utf8/1` are unchanged. Configuration support identity is now
`compiler-target-production/3`, retaining complete Try-statement support and
adding only `target_os = "macos"` as a standalone predicate, evaluated against
the actual rustc configuration strings. Other target_os values, other key/value
predicates, nested not/all key/value forms and cfg_attr remain unsupported.
Metric definitions stay unchanged. The final
configuration source and binary hashes are separately bound in provenance;
neither identity change permits reuse or relabeling of an earlier series.

`test_actual_process_command_base_head_inventory_and_reparse` reads base
`3d887a460bb72064b29a6f9ea71293e2949a16d7` and actual working-head source
independently. It inventories Linux, macOS and Windows compiler configurations,
checks each active function and closure, instruments and reparses the exact
bytes, and maps both body and symbol UTF-8 spans back to their original ranges.
Test code and inactive target code are excluded. Cross-target syntax inventory
alone is not a platform execution certificate.

The test embeds the exact base bytes with SHA-256
`80118e08ebab122498013269a47a12c6b3dca80107ead21a3fbd81d4856aff61`.
It always verifies that hash and cross-checks the precise Git blob when present.
A shallow clone's explicit missing-object response uses those same frozen bytes,
never head as base. Git errors or snapshot tampering still fail;
`test_process_command_base_snapshot_missing_object_and_tamper` checks these paths.

Unix base has the isolation function, its setsid initializer closure,
`terminate` and `send_signal`. Unix head additionally has
`terminate_with_signal`, `reap_timeout`, `observe_exit` and its shared PID-level
`observe_pid`. Actual macOS head additionally owns `list_process_group` and
`darwin_group_is_finished`; other target inventories exclude their exact source
ranges. Windows base and head
have the unchanged isolation function, `terminate`, and its `is_ok_and` closure.
The pre-exec closure has CC 2; the Windows status closure has CC 1. Counters in
macro/string contents are not invented Rust decisions.

## Bounded macOS predicate and Try-statement support

`test_cfg_macos_predicate_is_bounded_and_ranges_reparse` inventories the real
Linux/macOS/Windows rustc configurations, checks exact inactive item/statement
bytes and reparses, and retains precise refusal diagnostics for the unsupported
key/value and nested predicate forms. Actual command base/head source inventories
include every new host-active function, with independent LLVM mapping and missing
owner negatives even when a counter is zero. New Darwin owners must also have
positive native counters in their own records. Cross-target AST parsing does not
certify libproc execution on a different host. The configuration source/binary
hashes and `/3` identity bind the final tool; `/1` and `/2` manifests are rejected.

## Bounded Try-statement support and analyzer build identity

The original Windows base/head source includes
`#[cfg(not(windows))] child.kill()?;`. Its outer expression is `syn::Expr::Try`.
The earlier configuration visitor did not read attributes at that position and
correctly rejected it instead of inventing a production inventory. The first
source suite's 23 tests/two Windows AST errors remain preserved under
`/mnt/dev-ssd/dev-tmp/gh285-push-20261008/source-measure-suite/`; its analyzer SHA
was `fba95ac8c5f61bd7e1e90073f675ca4f1e6b8227978643ea8eb750398c2b635c`.
That old helper explicitly built its shared checkout target despite the caller's
target override. The record/source hashes matched; this was a real unsupported
syntax boundary, not a stale-cache explanation.

The new configuration visitor consumes Try attributes only as a complete
statement in `visit_block_mut`, with the statement's `?` and semicolon included
in excluded ranges. A nested Try retaining cfg or cfg_attr is rejected **before**
`active()` can delete the attribute, for both true and false predicates. Unknown
predicates and cfg_attr statements still fail. No production statement or embedded
base bytes are wrapped, rewritten or skipped to avoid the refusal.

`test_cfg_try_statements_and_nested_positions_are_bounded` verifies active and
inactive statement inventories, exact excluded bytes/spans, CC and reparse for
Linux/macOS/Windows. Its five refusal cases per target record exact stderr and
empty stdout, including nested true/false predicates and cfg_attr. The real
`test_cfg_try_real_native_ok_err_and_excluded_denominators` compiles the same
Try-statement source on the actual host, executes both Ok and Err early-return
paths, retains separate and combined LLVM profiles, and checks independent owner
counts and complete line/region denominators. Each actual export must have no
region overlapping the complete inactive statement; raw coordinates and counts
are retained per mode. This is an absence assertion about the compiler's actual
layout, not a claim that measurement clips a real enclosing parent region or
reduces the native denominator. An
independent original-byte grid oracle selects the innermost actual LLVM interval
at each position, clips the precise excluded interval, and records expected
line/count and code-region sets for each Ok/Err/combined export. These sets and
their denominators are compared with measured output. Expected physical-line
sets are independently specified from this source for the active target.
The oracle and raw profiles remain available on failure.
The Ok(7) return region must have an independent zero Err count and positive Ok
count; this does not require its physical line to become uncovered when an
enclosing interval still contributes on that line.
Missing owners, omitted exclusions and the old `/1` and `/2`
configuration identities fail at their precise boundaries. Cross-target AST tests
cannot substitute for native Windows/macOS evidence.

`test_cfg_try_synthetic_interval_contract` separately checks interval clipping
against each target's actual full-statement AST exclusion. Its enclosing interval
`[4, 1, 15, 2]`, code kind 0 and counter 1 are explicitly synthetic. Hand-listed
expected physical-line sets require 12 lines before clipping and 10 after;
the two whole interior lines disappear while the partial first/last lines retain
unexcluded contributions. Its named `synthetic-contract.json` is marked as
synthetic, and is never inserted into an LLVM export, native-result or ordinary
risk evidence. This unit contract does not certify a native parent region.

The preserved `source-measure-suite-v2` run under the same push evidence root
ran 25 tests in 44.090 seconds: 24 passed and this native Try case failed at the
incorrect assertion that an enclosing LLVM code region must exist. Linux
rustc 1.97.1/LLVM 22.1.6 emitted eight separate configured-function code regions;
none overlapped `[10, 5, 13, 8]`. Its Ok-mode grid/raw-region/count comparisons
reached that assertion. Err, combined profiles and the subsequent refusal
assertions were **not executed** and have no PASS evidence from that run.
The original export, binary, profile and oracle remain retained in
`source-measure-suite-v2/native/cfg-try-native-30m3_je_/`.
The 24 passes apply to their original test bytes/tool identity and do not certify
the corrected suite. That test-contract correction produced a complete v3
suite of 26 tests, preserving all 25 previous cases, without changing parser,
product, measurement formula, support identity or gates at that stage. The v3
Linux passes remain bound to those original bytes and `/2` tool identity;
the current `/3` suite must run again in a fresh directory as described below.

The class helper now respects explicit `CARGO_TARGET_DIR` and passes the selected
rustc host as Cargo's explicit `--target`. It records build argv/environment,
compiler identity, checkout/source hashes, stdout/stderr/status, and Cargo's
reported executable path/freshness. That path must equal the requested
directory/host/debug binary, whose actual SHA is recorded after a successful
locked build and unchanged source/lock checks. A build failure never falls back
to a previous executable. Build records use a fresh retained evidence directory;
the native fixtures reference that record by path and hash.

## Native fork/exec persistence evidence

`test_process_command_real_fork_exec_native_ownership` builds independent
base/head crates from those exact source bytes. It pins the repository's actual
`libc` version, registry source and checksum through a dependency-subset lock.
It records explicit `cargo fetch --locked`, then `cargo build --locked --offline`,
and verifies lock bytes after both. Builds have independent target directories,
actual selected compiler/LLVM identities, original and instrumented source,
AST/configuration manifest, command logs, binary hashes and raw profiles.
Natural-exit Unix modes first establish bounded fixture-owned WNOWAIT readiness,
without reaping. The unmodified Darwin base's isolated mode records its actual
outcome: either legacy EPERM followed by a safe direct-child reap, or actual
success on that kernel. No old-base failure is relabeled as a successful cleanup.
Head must successfully return exit 7. A separate `live-anchor` mode installs its
TERM handler before publishing its real PID; both base and head must terminate
that live owned child with exit 7. This certifies callback persistence without
requiring the frozen base to fix its historical zombie-only behavior. API refusal
modes are labeled separately and never substitute for actual host process cases.

Plain and isolated modes emit an ordinary parent profile. The isolated Unix
mode registers a second **fixture-only** `pre_exec` after the unchanged original
setsid callback. It writes the LLVM profiler's current counters to an independent
child-PID file before exec. Parent and child filenames/PIDs must differ, the
child must have its own original-closure LLVM record and a positive count, and
its own line/region coverage must be nonzero. The plain parent record has an
independent zero count; zero means no execution observed by that profile, not
a proof the callback did not execute elsewhere.

This executable is single threaded at fork. The explicit profiler flush is a
test mechanism, not an async-signal-safe production callback. It establishes
independent counter ownership and persistence, and does not establish that an
ordinary fork/exec parent profile captures child executions. Production
`pre_exec` still only invokes setsid (plus the existing measurement
instrumentation when collecting risk). Fixture flush counters never enter
normal repository risk evidence. A parent-only zero coverage remains zero in
the ordinary metric calculation; applicable 80/80 gates must still pass without
an exemption. Missing records may not be replaced by zeros or parent counts.

Native evidence stores parent-only, child-only and combined exports separately,
plus every owner's instances/counts and complete line/region denominators.
Combined profiles include inherited pre-fork counters; their counts are not a
certificate of distinct invocation totals. Independent child closure execution
is established from the child-only export.
Additional head modes exercise ECHILD and bounded failed-KILL behavior. Each
mode removes each owner in turn, including zero-hit records, and requires the
precise missing-function diagnostic. A closure with its parent's mapping is
rejected. Removing the positive child closure record also fails. The success
result is written only after all native and negative assertions complete; all
earlier command output and raw profiles remain on a failure.

The source suite runs through the existing native measurement workflow on Windows
and macOS, and the Linux quality-script suite. Set
`RUST_MEASURE_NATIVE_EVIDENCE` to a new absolute directory to preserve evidence:

```sh
python3 -B -m unittest discover -s tools/quality/tests -p test_source_measure.py -v
python3 -B -m unittest discover -s tools/quality/tests -p test_ci_quality.py -v
```

Run both new cfg-Try tests and the exact Windows base/head native fixture with
the final analyzer as part of this source suite. Original Linux process 5/5 and
fork/exec evidence remain facts about their original bytes/tool identity; they
are not relabeled as new-configuration certification. Final base/head risk uses
one identical final tool per host, without changing thresholds or requiredness.

Existing negative coverage is reused, including
`test_instrumentation_distinguishes_unexecuted_closure_and_rejects_missing_evidence`
(original digest, instrumented bytes, missing independent records and unsupported
macro regions), `test_compare_rejects_mixed_target_configurations`,
`test_unsupported_configuration_fails_without_partial_inventory` and
`test_unknown_macro_fails_closed`. The CI scope assertion accepts command.rs but
still blocks unknown production sources before analyzer build.

## Acceptance boundary

Source implementation and fixture design are not completed certification. Push
must retain actual process and native fixture results by host, then measure
base and final head independently with the same final tools/configuration for
that host. Exact final commit SHA and all required CI checks remain necessary.
Do not mix platforms or reuse old-series evidence. A missing fork/exec closure
record, unsupported syntax, or a failed ordinary coverage/risk gate blocks this
extension; it does not authorize a measurement exemption or fixture-data fallback.
No toolchain/default/installed host change or baseline adoption is part of this
work. Issue 286 report locking remains separate.

## PR303 failure boundary and fresh verification

The exact 3c9e4c150ca8241912bf44bbab4bb086e3036503 CI failures remain in
`/mnt/dev-ssd/dev-tmp/gh285-push-20261008/pr303-failures-37733282364-70/`.
Windows's sole source-suite failure was a real instrumented-source byte mismatch:
`Path.write_text` translated LF to CRLF while the manifest bound UTF-8 LF bytes.
The fixture now writes precise source and instrumented bytes using `write_bytes`
and asserts their actual bytes. The measurement's strict hash check is unchanged;
original and instrumented retained evidence use exact bytes too.

The macOS source suite stopped on the unmodified base's isolated natural-exit
EPERM. Its product run had 416 passes and 9 failures, with 7 explicit EPERM
diagnostics and 2 only indirectly consistent timeout/status failures. Neither
indirect error is asserted to be a proven EPERM. The `Git whitespace check`
label belongs to a fixture changed to `sh -c "sleep 2"`; it is not evidence of
an independent whitespace failure. No task, capture, adapter, preset or old test
contract is changed. All 9 regressions and subcases not reached after the early
failures must pass on the actual final macOS binary.

The old Linux `/2` ordinary-risk evidence (1044 identities, zero failures) is
historical evidence for that exact tool and commit only, not `/3` certification.
Preserved v1/v2/v3 profiles and all failure directories must not be overwritten.
A fresh full source suite now contains 27 tests, and ci_quality contains 22.
Push must bind the new suite/tool hashes, execute actual Linux/macOS/Windows
native ownership, repeat the applicable Linux regressions and exact same-tool
base/head risk chain, and obtain final-SHA required CI. Dev's static checks and
audit's source review do not certify those runs; they are pending at freeze.
