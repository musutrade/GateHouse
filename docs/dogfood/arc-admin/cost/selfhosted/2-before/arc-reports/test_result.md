=== Verification report ===
Timestamp: 2026-09-10T08:47:15.359768553+00:00
Profile: full
Components: backend,frontend,workflow

- PASS: secret scan (15443 ms)
- PASS: architecture audit (442 ms) - 0 violation(s), 0 blocker(s), 0 error(s), 0 warning(s)
- PASS: backend format (201 ms)
- PASS: backend Clippy (19722 ms)
- PASS: backend compile (2204 ms)
- PASS: backend tests (171881 ms) - 31 result(s)
- PASS: frontend lint (3705 ms)
- PASS: frontend format (3404 ms)
- PASS: frontend tests (12024 ms) - 85 result(s)
- PASS: frontend end-to-end tests (24352 ms)
- PASS: frontend real full-stack smoke test (21425 ms)
- PASS: frontend production build (9633 ms)
- PASS: Git hook syntax (101 ms)
- PASS: project initializer tests (502 ms)
- PASS: framework release configuration gate (102 ms)
- PASS: framework upgrade tests (1002 ms)
- PASS: template quality gate (501 ms)
- PASS: template quality gate tests (602 ms)
- PASS: observability configuration gate (102 ms)
- PASS: optional tracing and compliance configuration gate (101 ms)
- PASS: production deployment configuration gate (101 ms)
- PASS: audit retention and archive configuration gate (102 ms)
- PASS: Rust OpenAPI and Angular client generation gate (702 ms)
- PASS: supply chain security configuration gate (102 ms)
- PASS: arc-flow format (201 ms)
- PASS: arc-flow Clippy (201 ms)
- PASS: arc-flow tests (601 ms) - 58 result(s)

TEST_SUMMARY: PASS
