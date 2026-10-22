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

## Required durable format (not implemented yet)

Schema `gate-runtime/podman-plan-evidence-v1`. Fixed struct field order:
`schema`, `profile`, `asserted_installation_sha256`,
`asserted_executable_sha256`, `config_base64`, `config_sha256`, `operator_epoch`,
`image_id`, `image_base64`, `image_sha256`, `inputs`, `exposed_ports`,
`plan_base64`, `plan_sha256`. Raw fields use padded standard base64. Digests use
64 lowercase hexadecimal characters. Labels use sorted keys; exposures use a
sorted unique array. No trailing newline, whitespace or alternate key ordering.
Profile is the existing exact Podman tuple. Inputs contain only name, operator
labels and optional fixed port. Existing bounds on name/labels/port apply;
config and image are each at most 1 MiB, whole record at most 4 MiB.

Typed decoding must reject unknown/missing/duplicate fields and noncanonical
bytes, independently bound decoded fields, recompute all digests, re-decode the
image, rederive the plan and exposures, and compare exact current context bytes.
Installation and executable hashes remain assertions. No lease, dispatch or
freshness fields exist. Reopen does not mint a new CapturedImage.

## Storage requirements (not implemented yet)

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

## Remaining tests before storage acceptance

Re-derivation and every independent field substitution; duplicate/unknown and
noncanonical encoding; raw/decoded limits; another config/socket/epoch;
short/partial writes, file sync, parent sync and close failures; collision and
concurrent creators; symlink/hardlink/mode/metadata replacement. A successful
capture test alone does not establish any persistence property.
