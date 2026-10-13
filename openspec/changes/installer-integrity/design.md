# Design: Installer Artifact Verification

`install.sh` parses explicit arguments before creating temporary state. It
accepts only strict `vX.Y.Z`, optional prerelease, and build metadata; numeric
prerelease identifiers with leading zeroes are rejected. Binary URLs are
constructed from the immutable tag and fixed repository identity.

The download set is binary, `SHA256SUMS`, its `.sig`/`.crt`, and the binary's
`.sig`/`.crt`. Every file is non-empty and remains under a private temporary
root. The manifest must contain exactly one simple-name entry for the selected
binary. `sha256sum`/`shasum` validates that entry, then `cosign verify-blob`
validates both subjects with the Actions issuer and exact release workflow/tag
identity.

The destination validator rejects symlink path components, traversal, newline
characters, non-directories, unsafe permissions, and symlink/non-regular
targets. A create-new sibling is copied, made executable, and renamed into the
destination only after all checks pass. Failure cleanup removes only the
installer's private temporary root and sibling.

Source mode clones the exact tag and builds with `cargo install --locked` into
a temporary Cargo root before using the same destination publisher.

The test fixture replaces curl, cosign, git, cargo, uname, and cp with
deterministic local shims. It proves successful installation, checksum
tampering, signature and identity failure, target and parent symlink
rejection, executable mode, Windows naming, atomic replacement, signal
cleanup, and temporary file cleanup without network access.

## Compatibility and Rollback

The existing manual `cargo install` path remains unchanged. Users of the old
no-argument binary installer must pass a release version. A fix is rolled out
through a new immutable tag; an existing installed executable is never removed
on verification failure.
