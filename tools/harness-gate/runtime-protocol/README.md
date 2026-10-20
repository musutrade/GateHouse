# Runtime adapter data contract (first implementation slice)

This workspace library is linked into the Core runtime-probe host. It is not a
released adapter. It implements strict `gate-runtime/v1` data validation for capabilities,
inspect and force-remove by full lowercase 64-character object ID. It supports
neither container creation nor name-based lookup/removal. The existing service
runtime and lease v2 path are unchanged.

Messages are one UTF-8 JSON value, at most 65,536 bytes, with optional surrounding
JSON whitespace. Missing/duplicate struct fields, unknown fields, unknown enum
values and trailing JSON values are rejected. Labels are explicit key/value
records; duplicate keys and empty labels are rejected. Parse errors do not echo
untrusted values. Serde external enum tagging gives operations these shapes:

```json
{"inspect":{"object_id":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}}
```

`capabilities` is a string operation. Outcomes are `absent`, `unavailable`,
`completed`, `rejected-before-effect`, `unknown`, or externally tagged
`present`/`capabilities` objects. Absent/unavailable cannot carry object data.
Responses echo the exact request binding and operation. A present object must
have the exact requested ID and nonempty unique labels; Core must separately
supply its trusted required labels to `require_ownership`.

The binding includes protocol, adapter digest/version, invocation, operation
key, generation and an explicit engine namespace (engine, endpoint, context,
principal, rootless flag and scope). The library never reads environment or
configuration to fill these fields. Namespace strings are opaque exact pins,
not independently attested identities. Their acquisition and binding to the
actual engine remain a host/adapter implementation requirement. Digest fields
are syntax-checked here, not verified against executable bytes.

`require_same_intent` rejects any change from a recorded request. It does not
provide a persistent operation-key registry. `require_absence` accepts only a
separate exact-ID inspect response with the removal intent's same binding and
`absent` outcome. An adapter's `completed` mutation response is insufficient.
This predicate is not a state transition or permission to delete a lease.

## Explicit remaining work

- Capability negotiation, protected absolute artifact pinning and verification.
- Bounded subprocess framing, cancellation, output draining and process cleanup.
- Persisted prepared/unknown intents, lock/generation checks, owner checks and
  sticky-unknown recovery. No mutation of a lease is implemented by this crate.
- Engine namespace acquisition without ambient-context fallback; real engines.
- Start/partial-start protocol, installation/release and joint acceptance.

Legacy v2 leases do not match this request schema. They must stay on their
existing verified path; do not infer adapter/namespace/generation fields or
rewrite them to make the new schema parse. The Python experiment remains design
evidence, not an implementation dependency.

The Core fast-coverage archive includes this linked production package. Production
source files belong to the blocking process boundary; standalone crate tests
and clippy remain explicit CI gates in addition to Core production measurement.

```sh
cargo test --locked --manifest-path tools/harness-gate/Cargo.toml \
  -p gatehouse-runtime-protocol --target-dir /tmp/gatehouse-runtime-protocol-target
```

Protocol acceptance in this slice cannot establish zero subprocess dispatch,
bounded kill, persisted recovery or real Docker/Podman correctness. Those
require their own implementation and tests before a production switch.

## Removal intent transition model

`intent::Intent` is a pure model, not a durable coordinator or enabled removal
API. Preparation requires an authenticated (by the caller) present inspection
with the exact request binding/object ID and independently required ownership
labels. Its fields are private. `begin_dispatch` returns Unknown; the future
coordinator must durably commit that state under its generation lock before
spawning anything. Failure to commit must prevent dispatch.

Unknown is sticky for transport loss and all mutation outcomes, including
`completed` and `rejected-before-effect`. Invalid input leaves the original
value unchanged. Unknown cannot begin another dispatch. Only a separate exact-ID
Absent inspection under the same namespace/generation/operation key can return
Completed. This is not permission to delete a lease or proof that a real engine
returned the fact. Completed cannot dispatch or consume further transitions.

`restore` requires protected persisted bytes and independently trusted expected
request pins. It rejects legacy leases and changed bindings but does not verify
disk integrity, prevent rollback to an older Prepared record, authenticate the
adapter, lock a file, check owner liveness or migrate a lease. Those are blocking
requirements of the next durable coordinator, not guarantees of this model.
The current CLI continues to reject Remove before dispatch.

## Unix cooperative intent store

`store::Store` persists this model in an existing operator-provisioned canonical
absolute directory, one directory per intent. The directory must belong to the
current user with no group/other access bits. It takes a nonblocking exclusive
flock on the directory descriptor, retrying only within a caller-supplied
1ns–30s lock budget and honoring cancellation. Dropping the guard closes the
lock descriptor; no reusable lock pathname is removed. Filesystem calls/fsync
are not promised a hard wall-clock deadline.

The fixed `intent.json` is read without following its final symlink, with a size
bound; non-regular, multiply linked, foreign or non-private records reject.
Restore requires the independent full expected request, including generation and
operation key. There is no legacy lease conversion or record deletion.

Transitions compare the observed bytes under the held lock, write a private
temporary file, sync it, publish by atomic replacement (no-clobber on first
creation), then sync the directory. Unknown must be durably committed before a
future caller dispatches. Any commit error poisons the guard and prevents further
transitions until reopen/reconciliation; a failed directory sync may mean that
the new record is already visible, so failure is never evidence of no effect.
This crate still starts no process and the Core runtime probe still rejects
Remove. There is not yet a coordinator connecting owner checks, this store and
an authenticated runtime adapter.

All actors must use the same lock and preserve the private directory and its
ancestors. These are cooperative locks, not hostile same-account isolation or
cryptographic rollback protection. Replacing an earlier record with old bytes
can invalidate guarantees; the protected store is a trusted input. Rebinding the
directory or changing the record under a held guard is rejected when observed.
The implementation assumes a local filesystem with atomic rename and working
file/directory fsync. Real crash/power-loss guarantees require filesystem-specific
acceptance. Abandoned temporary files are retained, never swept automatically.


## Store-bound dispatch authorization

`Store::begin_dispatch` now returns a non-Clone `DispatchAuthorization` only
following a successful Unknown commit, including directory sync. Its private
mutable store borrow retains the lock and exposes only the stored request;
a future mutation host must consume this token rather than accept another
request. Dropping it leaves Unknown. It is not executable authentication or a
public mutation entry; no subprocess is started by this API.

After reopening Prepared, `resume_prepared` must validate a fresh exact-ID Present
inspection and independently supplied ownership labels before authorization.
A failed revalidation clears that gate. Unknown exposes only an inspect
`recovery_request`; only existing exact-ID Absent reconciliation completes it.
`phase()` is informational, including after poisoning, and never dispatch authority.
The coordinator still must enforce cancellation before begin, after commit and
before spawn, and after receiving output. A test-only per-store seam can fail before file sync, before publication, or
after publication before directory sync. It has no production environment knob
and simulates error ordering, not power loss. Independent failure-ordering tests
and the consuming authenticated mutation host remain the next acceptance work.
