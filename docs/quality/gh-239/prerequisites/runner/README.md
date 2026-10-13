# Runner image preparation

This is an operator-owned local image preparation, not an Actions job receipt or
final source-bound RC certification. The existing acceptance container/service
is unchanged. Production registration waits for the reviewed publication job and
approved inputs; registration must use organization runner group 4.

Inputs are retained under `/mnt/dev-ssd/symphony-inputs/gh239/runner-image`.
The Dockerfile's local base alias must resolve to exactly
`sha256:41459feb2f9970a7a22fa2b3789dccc286b1749a300300d97c3210ec56513db3`
before building. The first build incorrectly used that local image ID as a FROM
reference; BuildKit interpreted it as a registry name and failed. The corrected
build uses an operator-created alias after checking the actual local image ID.
Original failure remains in `build-original-failure.log`.

The runner archive is independently SHA-256 checked against GitHub's download
API metadata. OS dependency installation uses Ubuntu's authenticated apt metadata;
it resolves current packages, so this Dockerfile alone is not a reproducible
version lock. Retain the resulting image by exact ID, package inventory and image
archive digest before using it for production. Rebuilding requires a new observed
image receipt and complete revalidation. Exact ABI library bytes are copied from
the pinned acceptance-image inputs after dependency installation and rechecked.

The local probe runs without network, as UID 1000, with all capabilities dropped,
no-new-privileges and a read-only image. Only the public probe directory is mounted
read-only; work, temporary state and the runner diagnostic directory use disposable tmpfs. It verifies the host
library hashes, runner/Node/tool startup, denied writes to trusted paths, and
absence of Docker socket and developer home mounts. This demonstrates the tested
container boundary; it is not a fresh VM or broad host-escape certification.

A future production invocation needs a fresh writable runner working copy and
external diagnostic retention, separately staged read-only public trust and
approved packet/build inputs, bounded resources and controlled GitHub/Sigstore
network access. Never mount developer home, Docker socket or unrestricted host
paths. Inject only a short-lived registration token at registration; private RSA
trust remains in the protected environment. No token, secret, registration,
workflow dispatch or production signing is part of this preparation probe.

Relevant Engineering Policy semantics are unchanged.

## Observed result

Image `sha256:65b960d0cb906275c54d2576b4488f8ba6387d858dd79780e149738d069ebf04`
passed the local probe: runner 2.337.0, bundled Node 24.19.0, Python 3.14.4,
Git/OpenSSL/gh all start. All seven pinned shell/library hashes match, with kernel
7.0.0-31-generic. `probe-command.json` and `probe-result.json` retain the actual
invocation and observation. The first probe failed because Runner.Listener writes
_diag even for --version. The corrected invocation grants only that diagnostic
directory an additional bounded tmpfs; the runner binaries remain read-only.
Original failure is retained. These results do not establish online job execution.
