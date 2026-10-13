=== Verification report ===
Timestamp: 2026-09-10T08:33:15.198600762+00:00
Profile: full
Components: backend,frontend,workflow

- PASS: secret scan (15588 ms)
- PASS: architecture audit (442 ms) - 0 violation(s), 0 blocker(s), 0 error(s), 0 warning(s)
- PASS: backend format (301 ms)
- PASS: backend Clippy (52793 ms)
- PASS: backend compile (8411 ms)
- PASS: backend tests (217089 ms) - 31 result(s)
- PASS: frontend lint (4004 ms)
- PASS: frontend format (3604 ms)
- PASS: frontend tests (14344 ms) - 85 result(s)
- PASS: frontend end-to-end tests (25343 ms)
- PASS: frontend real full-stack smoke test (23028 ms)
- PASS: frontend production build (9614 ms)
- PASS: Git hook syntax (101 ms)
- PASS: project initializer tests (502 ms)
- PASS: framework release configuration gate (101 ms)
- PASS: framework upgrade tests (1103 ms)
- PASS: template quality gate (501 ms)
- PASS: template quality gate tests (602 ms)
- PASS: observability configuration gate (101 ms)
- PASS: optional tracing and compliance configuration gate (101 ms)
- PASS: production deployment configuration gate (101 ms)
- PASS: audit retention and archive configuration gate (101 ms)
- PASS: Rust OpenAPI and Angular client generation gate (702 ms)
- PASS: supply chain security configuration gate (101 ms)
- PASS: arc-flow format (201 ms)
- PASS: arc-flow Clippy (8110 ms)
- PASS: arc-flow tests (7707 ms) - 58 result(s)

TEST_SUMMARY: PASS
