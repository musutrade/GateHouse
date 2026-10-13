# Tasks: Signed Out-of-Process Adapter Protocol

- [x] 1.1 Record the protocol decision in ADR-0033 (P2, S).
- [x] 1.2 Define JSON transport, capability, signature, compatibility, and
  rollback requirements (P2, M).
- [x] 1.3 Add OpenSpec requirements and bounded implementation acceptance
  criteria (P2, S).
- [x] 2.1 Implement the adapter host and signed fixture (P2, L).
- [x] 2.2 Add cross-platform crash, cancellation, and capability tests (P2, M).

The host is opt-in and does not alter built-in step behavior. Release signing
and DevRail required-check ownership remain external governance decisions.

## Evidence Review

`cargo test --manifest-path tools/harness-gate/Cargo.toml
process::adapter::tests::` covers signed execution, capability and signature
rejection, crash, timeout, cancellation, malformed response, and artifact
escape. The same tests run in the Linux, macOS, and Windows CI matrix.
