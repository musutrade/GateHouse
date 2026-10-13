=== Verification report ===
Timestamp: 2026-09-10T09:01:09.020656477+00:00
Profile: full
Components: backend,frontend,workflow

- PASS: secret scan (15532 ms)
- PASS: architecture audit (448 ms) - 0 violation(s), 0 blocker(s), 0 error(s), 0 warning(s)
- PASS: backend format (201 ms)
- PASS: backend Clippy (19623 ms)
- PASS: backend compile (2003 ms)
- PASS: backend tests (172783 ms) - 31 result(s)
- PASS: frontend lint (3404 ms)
- PASS: frontend format (3404 ms)
- PASS: frontend tests (12122 ms) - 85 result(s)
- PASS: frontend end-to-end tests (24243 ms)
- PASS: frontend real full-stack smoke test (21626 ms)
- PASS: frontend production build (9519 ms)
- PASS: Git hook syntax (101 ms)
- PASS: project initializer tests (501 ms)
- PASS: framework release configuration gate (101 ms)
- PASS: framework upgrade tests (1002 ms)
- PASS: template quality gate (501 ms)
- PASS: template quality gate tests (602 ms)
- PASS: observability configuration gate (101 ms)
- PASS: optional tracing and compliance configuration gate (101 ms)
- PASS: production deployment configuration gate (101 ms)
- PASS: audit retention and archive configuration gate (101 ms)
- PASS: Rust OpenAPI and Angular client generation gate (702 ms)
- PASS: supply chain security configuration gate (101 ms)
- PASS: arc-flow format (202 ms)
- PASS: arc-flow Clippy (302 ms)
- PASS: arc-flow tests (602 ms) - 58 result(s)

TEST_SUMMARY: PASS
