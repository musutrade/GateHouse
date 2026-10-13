# Design: Post-Remediation Security, Reliability, and Maintenance Hardening

This design is the proposed implementation contract and evidence boundary.
Selected implementation tracks may be present in the source tree while the
umbrella remains Proposed; the status and task evidence must stay explicit.

## 1. R-07: Isolation wording and future sandbox boundary

The current adapter contract remains limited to signed request validation,
capability declarations, environment policy, bounded readers, process-group
cleanup, and evidence publication. Documentation, CLI help, ADRs, and OpenSpec
records must use the same vocabulary. `capability` means a protocol-level
allowlist; it does not mean a kernel or operating-system policy.

The future sandbox decision must be a separate record. It must select a
platform primitive for each supported OS, define network/filesystem/resource
scope, establish process identity and descendant behavior, specify a fail-
closed result when the primitive is missing, and provide cross-platform
fixtures. Until then, the compatibility contract rejects any claim of complete
descendant isolation.

## 2. R-13: Webhook egress policy

Webhook configuration is treated as an outbound data path. The eventual
validator and sender must apply one policy at both configuration and connect
boundaries:

1. Parse only supported HTTP(S) URL forms and normalize the host without
   logging credentials or query values.
2. Require an explicit host allowlist for configured destinations. A hostname
   allowlist is not a substitute for address classification.
3. Reject loopback, RFC1918/private, link-local, unspecified, multicast, and
   other local-only address classes by default for every resolved address.
4. Resolve immediately before connection and re-check every returned address
   against the policy. A destination that changes from public to local is a
   failure, not a retryable success.
5. Disable redirects or apply the same host and address policy to every hop;
   the implementation PR must make this choice explicit.

The policy must expose a structured rejection code and a redacted destination
summary. It must not include URL userinfo, bearer tokens, webhook bodies, or
resolved secrets in reports.

Proposed configuration shape (illustrative only; it is not accepted runtime
syntax until a compatibility review):

```toml
[webhook]
url = "https://hooks.example.test/events"
allowed_hosts = ["hooks.example.test"]
```

The acceptance matrix includes public IPv4/IPv6, loopback, RFC1918,
link-local, unspecified, a hostname with mixed results, and a DNS-rebinding
fixture that changes between validation and connection.

## 3. R-14: Cross-platform lease liveness and ownership

Lease liveness covers allocation, execution, cancellation, and cleanup. A
background renewal mechanism or equivalent host-owned loop must renew before
the TTL throughout that lifecycle and stop only after cleanup evidence is
written. Renewal records include the lease identity, invocation, resource
labels, immutable runtime identity, and monotonic timing evidence.

The implementation must use a reliable identity source per platform. Linux
may use its existing process-start identity; macOS and Windows need an
equivalent handle or start-time contract. If a platform cannot prove identity
or renew liveness, cleanup retains the resource and emits a structured
ownership-uncertain failure. It must never infer ownership from a reused PID,
name, or stale filename.

Implementation record (2026-09-03, v0.3.6): Linux reads the process start
time from `/proc/<pid>/stat`; macOS reads `pbi_start_tvsec`/`pbi_start_tvusec`
through `proc_pidinfo(PROC_PIDTBSDINFO)`; Windows reads the process creation
`FILETIME` through `GetProcessTimes`. Lease records store `heartbeat_at` and
`expires_at`; renewal runs on a 15-second heartbeat interval with a 30-second
renewal threshold and a 15-minute TTL. When identity or renewal cannot be
proven, cleanup retains the resource and emits structured
`LEASE_OWNERSHIP_UNCERTAIN` evidence.

Fixtures cover a step longer than the TTL, renewal failure, process restart,
identity reuse, cancellation during renewal, and cleanup after a successful
heartbeat on every supported CI platform.

## 4. R-15: Typed failures and structured diagnostics

Runtime results use one stable failure-code registry and a typed retry class.
Human messages are presentation fields derived from these values. Configuration
validation returns structured diagnostics with at least a code, severity,
canonical path, safe message, repair help, and optional source/related
locations. No scheduler, retry decision, or machine-result field may branch
on `contains`, prefix matching, or equality against display text.

Illustrative Rust shape (not an implementation commitment):

```rust
enum FailureCode {
    WebhookDestinationDenied,
    LeaseOwnershipUncertain,
    ResultParseFailure,
    SchedulerFailure,
}

struct Diagnostic {
    code: FailureCode,
    retry: RetryClass,
    path: Option<String>,
    message: String,
}
```

The exact registry, serialized spelling, compatibility policy, and migration
for existing string fields must be reviewed before code is written. Unknown
codes from a future producer fail closed at a machine-contract boundary while
remaining safely renderable for humans.

## 5. R-16: Wait and scheduler performance

The task runner should prefer a platform wait primitive when one exists. Where
polling is unavoidable, it should use a short bounded initial delay and
exponential backoff with a maximum, while observing cancellation and reader
deadlines. The scheduler should maintain deterministic node indexes and
remaining-dependency counts instead of repeatedly scanning every node.

The optimization must preserve stable dependency ordering, failure propagation,
cleanup, evidence completeness, and the current serial compatibility path.
Benchmarks compare the fixed baseline with representative fast commands,
long-running commands, wide DAGs, deep DAGs, cancellation, and failed nodes.

Illustrative algorithm shape:

```text
ready = indexed nodes whose remaining_dependencies == 0
while ready is not empty:
    node = ready.pop_deterministically()
    wait_for_process_or_deadline(node)
    decrement indexed dependents(node)
```

This is a design sketch only. The implementation must not use a ready index to
relax a rejected resource relation from configuration preflight.

## 6. R-17: Configuration and diagnostic maintainability

Validation should pass a borrowed validation context or focused sub-context
instead of cloning the complete `FlowConfig` for every service, parser, check,
or rule. Fields documented as closed vocabularies should deserialize into
closed enums and reject unknown values before execution. Environment variables
should use one `HARNESS_GATE_*` namespace; any compatibility alias needs an
explicit deprecation period and diagnostic.

The release profile's `panic = "abort"` and runtime `catch_unwind` behavior
must be documented as distinct contracts. Lock poisoning, panic boundaries,
and profile-specific tests should make the chosen behavior observable rather
than relying on an implicit build-profile interaction.

## 7. R-18: Repository and community governance

The repository hygiene track removes or archives one-off artifacts, ignores
Python cache files, and verifies that no generated cache is tracked. It adds a
security policy, MSRV declaration, issue and pull-request templates, and
quality-script lint/test coverage. Documentation must say that the built-in
secret scan is a quick content scan and does not replace dedicated secret
scanners.

The track also records product capabilities that are intentionally out of this
implementation: configuration include/inheritance, webhook authentication and
retry policy, and other future features must not be silently inferred from
metadata cleanup.

## Rollout and Evidence

The planned rollout is:

1. Approve this ADR/OpenSpec and freeze the current contracts.
2. Implement and verify R-13 and R-14 as the security/lifecycle tracks.
3. Introduce the typed failure registry and migrate machine consumers.
4. Run performance baseline and implement R-16 without semantic changes.
5. Complete R-17 and R-18 maintenance changes and documentation.
6. Run focused tests, locked tests, formatter, strict Clippy, audit, docs
   consistency, and the applicable Linux/macOS/Windows matrix.
7. Review rollback procedures and only then update task and status fields.

No production, DevRail staging, shadow, canary, or rollback-authority evidence
is part of this change.

## Alternatives and Rollback

The alternatives rejected by this design are: a simple webhook IP denylist,
PID/name-based lease ownership, display-string error parsing, unmeasured
performance rewrites, and advertising process groups as an OS sandbox. Each
fails to establish the required trust boundary.

If an implementation track causes a regression, revert that track through a
protected pull request and retain its failure evidence. Do not roll back by
silently restoring an unsafe webhook destination, deleting an uncertain
resource, accepting an unknown failure code as a pass, or reintroducing
unbounded polling. The current R-07 wording remains the fallback until a
separate sandbox decision is accepted.
