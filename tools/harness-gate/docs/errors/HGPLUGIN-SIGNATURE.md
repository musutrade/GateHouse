# HGPLUGIN-SIGNATURE

## Why this fails

The detached signature is malformed, the lock signer is not approved by the host, the signature does not verify over the exact manifest bytes, or the host key input is invalid or duplicated.

## Diagnose and repair

Supply an operator-approved `--plugin-trusted-key` and a detached Ed25519 signature over the domain and exact manifest bytes. Project pins never authorize a key; nothing is downloaded or approved automatically.

## Example

`harness-gate explain HGPLUGIN-SIGNATURE`
