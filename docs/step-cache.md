# Local deterministic step cache (step-cache/v1)

Issue #32 extends retained collector evidence with opt-in reuse of ordinary
external commands. Engineering Policy semantics are unchanged: Core still owns
requiredness, thresholds, baseline/debt/ratchet, final project decisions and
release authority. Quality preparation, signed requests, nonce ledgers,
collection and current policy evaluation retain their existing path.

## Enable and disable

The first implementation supports Unix hosts. Windows executes commands normally
and reports `CACHE_PLATFORM_UNSUPPORTED`. Existing flow version 2 configurations
without `steps.cache` keep their environment, behavior and report shape.

Add this declaration to a reviewed external snapshot step. Replace the example
toolchain paths with canonical files on your host. The first toolchain entry
pins the executable and its filename must match the bare `program` name; the
command uses this absolute pin even when host reuse is disabled:

```toml
[[steps]]
id = "project.static-check"
label = "Deterministic local check"
component = "project"
profiles = ["full"]
program = "checker"
args = ["{root}/src/input.txt"]
cwd = "{root}"
log = "static-check.log"
timeout_secs = 60

[steps.cache]
version = 1
deterministic = true
side_effect_free = true
inputs = ["generated/settings.json"]
outputs = []
toolchain = ["/opt/checker/bin/checker", "/opt/checker/lib/rules.dat", "/opt/checker/lib/runtime.so"]
environment = ["LANG"]
```

The declarations assert complete knowledge of the command's inputs. A command
that reads a database, network response, clock, randomness, Git metadata,
undeclared home files or mutable external state must not opt in. Include every
interpreter, library, compiler resource, plugin and ignored input the command
uses. This contract does not provide an OS sandbox or infer determinism.

Cache-configured commands always receive a cleared environment containing only
listed inherited variables, minus `remove_env`. This also applies when host
caching is disabled, so execution and reuse have the same declared semantics.
Services, built-in gates, repository-input commands, runner contracts, retries,
shards and invocation report arguments cannot declare caching. An ordinary
dependency must declare nonempty output files before its consumer can reuse a
result; otherwise the consumer executes normally.

Choose a private host directory **outside the checkout** whose existing parents
are protected by the host. Only the final directory may be created automatically.
The cache directory must be mode 0700; its files must be private, singly linked,
owned regular files. Do not mount it into untrusted command sandboxes. Processes
with the same uid and unrestricted host access share the host trust boundary.

```console
harness-gate --step-cache-dir /private/host/project-cache verify --profile full --all
harness-gate verify --profile full --all
harness-gate --step-cache-dir /private/host/project-cache cache-clear --json
```

Omitting `--step-cache-dir` disables reuse and writes. Removing `steps.cache`
restores legacy environment inheritance. `cache-clear` authenticates ownership
before unlinking individual entries and retains unknown, partial, modified,
linked or unverifiable files. It retains the private key and lock file. A nonzero
exit with `retained > 0` asks the operator to inspect remaining material; it is
not permission to recursively delete that directory.

## Identity and trust

A cache key binds the actual file bytes and mode, effective cwd and expanded
arguments, command executable, explicit toolchain files, Core binary and version,
OS/architecture, project, input mode, HEAD context, selected profile, complete
effective flow configuration, audit/secrets/quality configuration, plugin lock,
referenced quality policies, declared ignored inputs and dependency output
hashes. A working-tree identity enumerates tracked and nonignored untracked
files, including explicit absence for deleted tracked files. Staged identities
hash materialized source bytes; they never use a tree ID as a substitute for
checkout-filtered bytes. Input identity is rechecked after execution and again
before accepting a hit.

Effective snapshot paths are deliberately bound. Independently materialized
staged roots generally miss even when content is equal. Version 1 makes no
relocation-equivalence claim. Working-tree sources must remain stable while
verification runs, as required for the uncached command path too.

Input and output declarations are exact regular files, not globs/directories.
Links, unsupported names, missing explicit inputs, missing toolchain files,
non-Unicode environment values, excessive input size or unresolved identity
cause normal execution. The input scan is capped at 100,000 source files and
256 MiB per identity; Core binary hashing has a separate 512 MiB cap and is
memoized only within the running process. Tracked source cannot be declared as
generated output. Invocation evidence is excluded from source scans and cannot
be a declared input or output. If report paths contain tracked files, caching
is bypassed rather than excluding tracked source from the key.

The host generates a private Ed25519 seed once per cache namespace. Separate
domain prefixes protect deterministic key derivation and record signatures.
Only an opaque, keyed 256-bit digest is written as the key. The sensitive input
identity, including environment values and configuration arguments, stays in
memory. Records contain provenance, output hashes, a log digest and authenticated
log bytes. Logs requiring redaction or truncation are not stored. Copying entries
to a namespace with another host key does not establish trust.

On a hit Core verifies the signature, version, key, step/provenance, age, log
hash, every existing output hash and current inputs. It copies the authenticated
log into the **new invocation**, applies the **current parser**, and returns only
that external command's successful fact. Missing/changed outputs cause execution;
version 1 never restores outputs. Mandatory prerequisites, cancellation,
requiredness, waivers, service cleanup, current quality policy, aggregation and
fresh report sealing continue normally. No cached project PASS or signed adapter
request is loaded. Failed, timed-out, cancelled, flaky, waived or partial results
are never published as success-cache records.

## Capacity, concurrency and failure

A nonblocking host file lock serializes namespace initialization, lookup,
publication, eviction and clearing. Contention takes the normal execution path;
concurrent misses may both execute. Signed records are published by same-directory
atomic rename after syncing the complete staging file. There is one staging
filename and no recursive cache directories.

Managed entries are bounded by 128 records, 64 MiB aggregate record bytes,
8 MiB per entry, 2 MiB original log bytes and seven days of age. Oldest signed
records are evicted first; replacement and expired entries are removed under the
same lock. An interrupted partial staging file is retained and blocks further
writes until an operator inspects it. A fully authenticated staging record can be
reclaimed. Invalid ownership/signatures never justify deletion. Unrelated files
are outside the managed capacity and retained. Cache failure changes reuse into
execution and does not change the command's result.

## Explanation and compatibility

`verify --plan --json` uses the #29 `verification-plan/v1` reason structure. A
cache-configured selected step adds `CACHE_RUNTIME_NOT_VALIDATED`; preview never
opens the host cache, hashes runtime inputs, creates state or predicts a hit.

Executed steps optionally expose `cache` in `test_result.json`. Existing report
schema version 1 remains readable by the updated schema; the nested extension is
explicitly `step-cache/v1`. Strict older schema validators must upgrade before
consuming this optional extension. Legacy reports omit it and remain valid.

| Status | Reason | Meaning |
| --- | --- | --- |
| HIT | CACHE_INPUTS_AND_OUTPUTS_VERIFIED | Authenticated evidence and current inputs/outputs/parser verified. |
| MISS | CACHE_ENTRY_ABSENT | This complete keyed identity has no entry. |
| MISS | CACHE_ENTRY_UNTRUSTED | Entry cannot be authenticated, is expired, or lookup is busy. |
| MISS | CACHE_OUTPUT_MISSING_OR_CHANGED | Existing output content/mode differs or is absent. |
| MISS | CACHE_REVALIDATION_FAILED | Current parser, log/provenance or second input check rejected reuse. |
| MISS | CACHE_STORE_REJECTED | Normal execution ran, but its evidence could not safely be stored. |
| BYPASS | CACHE_HOST_DISABLED | No host directory was supplied. |
| BYPASS | CACHE_PLATFORM_UNSUPPORTED | Host storage implementation is unavailable. |
| BYPASS | CACHE_DECLARATION_UNBOUND | Execution-time declaration validation rejected caching. |
| BYPASS | CACHE_INPUT_UNBOUND | Actual source, tools, environment or dependency outputs could not be bound. |
| BYPASS | CACHE_HOST_UNTRUSTED | Directory ownership, permissions, location or initialization/lock could not be trusted. |

`key` and `source_invocation` are nullable. `lookup_ms` includes key construction
and hit validation; `store_ms` includes post-execution binding and publication;
`saved_execution_ms` records the original command duration for a hit, not a
promised wall-clock gain. Compare repeated end-to-end timings with matching
source/toolchain/profile and report cold initialization separately. Very cheap
commands or large input trees can cost more to hash than to execute. The optional
[measurement record](quality/issue-32/README.md) reports the measured boundary.

Rollback: omit the host flag first, then remove cache declarations and update
strict validators with the matching report/schema revision. No evidence,
signature request, retained collector baseline or nonce history is migrated,
reset or reinterpreted. Remote sharing, Windows storage, artifact restoration and
relocation-equivalent staged reuse require separate trust/compatibility review.
