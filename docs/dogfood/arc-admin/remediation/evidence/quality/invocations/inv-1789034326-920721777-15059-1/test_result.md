=== Verification report ===
Timestamp: 2026-09-10T10:03:08.609794879+00:00
Profile: full
Components: backend,frontend,workflow

- PASS: secret scan (14466 ms)
- PASS: architecture audit (479 ms) - 0 violation(s), 0 blocker(s), 0 error(s), 0 warning(s)
- PASS: backend format (237 ms)
- PASS: backend Clippy (559 ms)
- PASS: backend compile (399 ms)
- PASS: backend tests (165743 ms) - 31 result(s)
- PASS: frontend lint (3602 ms)
- PASS: frontend format (3442 ms)
- PASS: frontend tests (13714 ms) - 85 result(s)
- PASS: frontend end-to-end tests (23265 ms)
- PASS: frontend real full-stack smoke test (20031 ms)
- PASS: frontend production build (6808 ms)
- PASS: Git hook syntax (8 ms)
- PASS: project initializer tests (481 ms)
- PASS: framework release configuration gate (80 ms)
- PASS: framework upgrade tests (1122 ms)
- PASS: template quality gate (481 ms)
- PASS: template quality gate tests (646 ms)
- PASS: observability configuration gate (79 ms)
- PASS: optional tracing and compliance configuration gate (79 ms)
- PASS: production deployment configuration gate (81 ms)
- PASS: audit retention and archive configuration gate (82 ms)
- PASS: Rust OpenAPI and Angular client generation gate (721 ms)
- PASS: supply chain security configuration gate (79 ms)
- PASS: arc-flow format (243 ms)
- PASS: arc-flow Clippy (320 ms)
- PASS: arc-flow tests (645 ms) - 58 result(s)
Quality profile "full": blocked (configuration); full quality: blocked
quality workflow input is missing: /mnt/dev-ssd/workspaces/symphony/GH-207/target/quality/gh-207/parity/source/.harness-gate/runtime/full-state.json

TEST_SUMMARY: FAIL
