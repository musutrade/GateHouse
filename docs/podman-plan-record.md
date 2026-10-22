# Podman plan evidence (unpublished)

This is internally consistent evidence for one protected context. It is not
proof of freshness, signature, current ownership, restart continuity,
installation truth, or authorization. Docker v1 is unchanged.

## Construction boundary

`CapturedImage` has no public constructor or deserializer. Only protected
info/image/info produces it, retaining exact bounded config and image bytes,
image facts and config-derived operator epoch. The injected GET callback is
still trusted to supply responses: callers can fabricate replies. This type
proves execution of validation, not actual connected transfer. A later installed
host must fix the exchange to the verified transport before making any stronger
claim. Public `decode_image` and `ImageFacts` cannot construct this witness.

## Evidence codec

Schema `gate-runtime/podman-plan-evidence-v1`. Fixed struct field order:
`schema`, `profile`, `asserted_installation_sha256`,
`asserted_executable_sha256`, `config_json`, `config_sha256`, `operator_epoch`,
`image_id`, `image_json`, `image_sha256`, `inputs`,
`plan_json`, `plan_sha256`. Raw fields use JSON strings preserving exact valid UTF-8 bytes (including inner whitespace). This deliberately replaces the proposed base64 format; no binary/non-UTF-8 JSON is accepted. Exposures are rederived from the raw image rather than stored a second time. Digests use
64 lowercase hexadecimal characters. Labels use sorted keys; exposures are rederived from image JSON. No trailing newline, whitespace or alternate key ordering.
Profile is the existing exact Podman tuple. Inputs contain only name, operator
labels and optional fixed port. Existing bounds on name/labels/port apply;
config and image are each at most 1 MiB, whole record at most 16 MiB (accounting for JSON escaping).

Typed decoding must reject unknown/missing/duplicate fields and noncanonical
bytes, independently bound decoded fields, recompute all digests, re-decode the
image, rederive the plan and exposures, and compare exact current context bytes.
Installation and executable hashes remain assertions. No lease, dispatch or
freshness fields exist. Reopen does not mint a new CapturedImage.

## Storage implementation and limits

Create a new 0600 single-link current-UID regular file with O_EXCL/O_NOFOLLOW in
a protected canonical parent. Retain held file/parent identities and revalidate
before and after I/O. Write all bytes, sync file, sync parent, explicitly check
close before reporting success. Refuse any existing file without truncation,
replacement or automatic repair. Empty, partial and torn records remain for
operator diagnosis. Any write, sync, close or identity failure is unsuccessful;
a complete file may nevertheless remain after an unknown durability outcome.
Concurrent creators have one winner. Open validates owner/mode/link/type,
bounds, path identity, canonical bytes and context, then revalidates after read.

Crash before creation leaves no record. Crash after creation may leave empty or
partial bytes. File sync without parent sync does not establish directory-entry
durability. Crash after both syncs can leave a valid record despite caller not
observing success; reopen validates evidence only, never retries mutation.

Exact config includes socket inode/device/path: socket recreation and restart
are not adopted. Same UID/root can forge, replay, remove or roll back a complete
canonical record. Metadata checks are race detection, not containment.

## Required validation before storage acceptance

Re-derivation and every independent field substitution; duplicate/unknown and
noncanonical encoding; raw/decoded limits; another config/socket/epoch;
short/partial writes, file sync, parent sync and close failures; collision and
concurrent creators; symlink/hardlink/mode/metadata replacement. A successful
capture test alone does not establish any persistence property.

Storage now uses held-parent /proc/self/fd anchoring, create-new, file/parent sync and explicit close. Boundary interruption tests are not simulated kernel fsync/close errors or power-loss proof. Full fault injection and independent review remain required.

Creation reopens and compares the complete canonical record to the intended bytes
before reporting success. This detects substitution at that boundary; it cannot
prevent a same-UID writer changing bytes after the check. Tests inject short-write
ENOSPC, file/parent-sync EIO and close failure through the private I/O adapter,
and replace valid evidence at the reopen boundary. These are deterministic
software fault simulations, not kernel fault injection or power-loss proof.

Boundary regression tests explicitly show that replacing only EXPOSE without
updating the response hash is rejected, while recomputing an unkeyed hash can
produce valid different evidence with the same create body. Likewise, valid
replacement after create_new returns can be read successfully. Neither behavior
is authentication or a guarantee against same-UID writers. Consumers needing a
specific saved record must keep their own trusted expected identity; this API
alone supplies no such authority.

## Versioned installation-context prerequisite

`ProtectedContext::load_versioned` accepts only the canonical object with fields
`schema` then `context`, schema `gate-runtime/podman-context-v1`, and the existing
typed ExpectedContext field order. It opens config once and retains its exact
wrapper bytes. Both `snapshot` and `config_sha256` refer to that wrapper, not a
reserialized inner context. Duplicate/unknown fields, alternate whitespace/order
and raw-context input are rejected. The legacy development `load` remains an
explicit separate raw loader and rejects the wrapper. Neither loader adopts old
records or rewrites config. Future installation verification must require the
versioned loader; `is_versioned` only reports format, never installation truth.
Executable/host/installation held verification and RecordInstallationMatch are
not implemented by this prerequisite.

`from_held_versioned(path, File)` now consumes the original read-only CLOEXEC
regular config descriptor, checks protected path identity against it, and reads
with explicit offsets (the caller's cursor is ignored and unchanged). No config
path reopen or caller-supplied verified bytes is accepted. Wrong inode, replaced
path, writable/O_PATH descriptors and invalid wrappers fail. Socket opening is
still a separate protected socket operation, not a connected transfer.
Versioned capture -> PlanRecord -> create-new -> reopen is covered; this remains
a context prerequisite, not held host/installation/executable verification.

## Held installation checkpoint

`VerifiedPodmanInstallation::load` holds host, installation, versioned config and
executable snapshots. Canonical host keys: schema, installation_path,
installation_sha256, config_path, config_sha256. Installation keys: schema,
product, version, protocol, executable_path, executable_sha256, config_schema.
Schemas are gate-runtime/podman-host-v1 and gate-runtime/podman-installation-v1;
product/version/protocol/config_schema match the explicit Podman tuple/context.
Descriptors are euid-owned, single-link 0400/0600, at most 1MiB. Executable is a
single-link regular file at most 128MiB: nonroot euid owner 0500/0700 or root owner
0555/0755; ELF magic required. ELF magic does not validate architecture, ABI or
entrypoint. No file is executed. Protected ancestors and same held/path identity
are rechecked; every snapshot is reread positionally for exact byte equality.
Config is passed using a cloned original descriptor, never reopened by pathname.

A live `RecordInstallationMatch` borrows the installation and record, compares
profile, config bytes and asserted digests, and revalidates on each use. It cannot
be deserialized. It does not change installation_verified(), grant a lease, or
protect saved assertions against same-UID forgery/replay. Host path selection is
still an explicit caller input; fixed Core-owned mapping and installed process
launch are future integration work. Same-UID/root changes after checks, daemon
identity, signature trust and real Podman remain outside this checkpoint.
