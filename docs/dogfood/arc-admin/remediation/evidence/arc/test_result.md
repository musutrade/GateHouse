=== Verification report ===
Timestamp: 2026-09-10T09:53:26.805868105+00:00
Profile: full
Components: backend,frontend,workflow

- PASS: secret scan (15517 ms)
- PASS: architecture audit (428 ms) - 0 violation(s), 0 blocker(s), 0 error(s), 0 warning(s)
- PASS: backend format (201 ms)
- PASS: backend Clippy (58528 ms)
- PASS: backend compile (8812 ms)
- PASS: backend tests (280973 ms) - 31 result(s)
- PASS: frontend lint (3904 ms)
- PASS: frontend format (3404 ms)
- PASS: frontend tests (12931 ms) - 85 result(s)
- PASS: frontend end-to-end tests (24448 ms)
- PASS: frontend real full-stack smoke test (18822 ms)
- PASS: frontend production build (8921 ms)
- PASS: Git hook syntax (101 ms)
- PASS: project initializer tests (502 ms)
- PASS: framework release configuration gate (101 ms)
- PASS: framework upgrade tests (1103 ms)
- PASS: template quality gate (502 ms)
- PASS: template quality gate tests (602 ms)
- PASS: observability configuration gate (102 ms)
- PASS: optional tracing and compliance configuration gate (102 ms)
- PASS: production deployment configuration gate (101 ms)
- PASS: audit retention and archive configuration gate (102 ms)
- PASS: Rust OpenAPI and Angular client generation gate (702 ms)
- PASS: supply chain security configuration gate (101 ms)
- PASS: arc-flow format (202 ms)
- PASS: arc-flow Clippy (7508 ms)
- PASS: arc-flow tests (7511 ms) - 58 result(s)

TEST_SUMMARY: PASS
