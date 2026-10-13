=== Verification report ===
Timestamp: 2026-09-10T05:41:52.169891023+00:00
Profile: full
Components: backend,frontend,workflow

- PASS: secret scan (31243 ms)
- PASS: architecture audit (765 ms) - 0 violation(s), 0 blocker(s), 0 error(s), 0 warning(s)
- PASS: backend format (401 ms)
- PASS: backend Clippy (84442 ms)
- PASS: backend compile (8612 ms)
- PASS: backend tests (223407 ms) - 31 result(s)
- PASS: frontend lint (3905 ms)
- PASS: frontend format (3404 ms)
- PASS: frontend tests (14233 ms) - 85 result(s)
- PASS: frontend end-to-end tests (26544 ms)
- PASS: frontend real full-stack smoke test (20125 ms)
- PASS: frontend production build (10118 ms)
- PASS: Git hook syntax (101 ms)
- PASS: project initializer tests (501 ms)
- PASS: framework release configuration gate (101 ms)
- PASS: framework upgrade tests (901 ms)
- PASS: template quality gate (501 ms)
- PASS: template quality gate tests (601 ms)
- PASS: observability configuration gate (101 ms)
- PASS: optional tracing and compliance configuration gate (100 ms)
- PASS: production deployment configuration gate (101 ms)
- PASS: audit retention and archive configuration gate (101 ms)
- PASS: Rust OpenAPI and Angular client generation gate (701 ms)
- PASS: supply chain security configuration gate (101 ms)
- PASS: arc-flow format (201 ms)
- PASS: arc-flow Clippy (8515 ms)
- PASS: arc-flow tests (7413 ms) - 58 result(s)

TEST_SUMMARY: PASS
