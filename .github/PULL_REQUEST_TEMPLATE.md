## Summary

Describe the user-visible behavior and the scope of this change.

## Verification

- [ ] `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check`
- [ ] `cargo test --manifest-path tools/harness-gate/Cargo.toml --locked`
- [ ] `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --locked --all-targets --all-features -- -D warnings`
- [ ] Relevant documentation, schema, and OpenSpec records are updated

## Safety and rollback

Describe trust-boundary changes, compatibility impact, and the reviewed
rollback path. Do not include credentials, private keys, or customer data.
