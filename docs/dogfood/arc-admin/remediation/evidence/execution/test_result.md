=== Verification report ===
Timestamp: 2026-09-10T09:58:46.475863926+00:00
Profile: full
Components: backend,frontend,workflow

- PASS: secret scan (13399 ms)
- PASS: architecture audit (389 ms) - 0 violation(s), 0 blocker(s), 0 error(s), 0 warning(s)
- PASS: backend format (239 ms)
- PASS: backend Clippy (19792 ms)
- PASS: backend compile (2811 ms)
- PASS: backend tests (198439 ms) - 31 result(s)
- PASS: frontend lint (4483 ms)
- PASS: frontend format (3768 ms)
- PASS: frontend tests (14851 ms) - 85 result(s)
- PASS: frontend end-to-end tests (24141 ms)
- PASS: frontend real full-stack smoke test (20673 ms)
- PASS: frontend production build (7292 ms)
- PASS: Git hook syntax (8 ms)
- PASS: project initializer tests (399 ms)
- PASS: framework release configuration gate (78 ms)
- PASS: framework upgrade tests (880 ms)
- PASS: template quality gate (478 ms)
- PASS: template quality gate tests (639 ms)
- PASS: observability configuration gate (78 ms)
- PASS: optional tracing and compliance configuration gate (80 ms)
- PASS: production deployment configuration gate (80 ms)
- PASS: audit retention and archive configuration gate (78 ms)
- PASS: Rust OpenAPI and Angular client generation gate (721 ms)
- PASS: supply chain security configuration gate (78 ms)
- PASS: arc-flow format (318 ms)
- PASS: arc-flow Clippy (319 ms)
- PASS: arc-flow tests (646 ms) - 58 result(s)
Quality profile "full": blocked (configuration); full quality: blocked
quality workflow input is missing: /mnt/dev-ssd/workspaces/symphony/GH-207/target/quality/gh-207/parity/source/.harness-gate/runtime/full-state.json

TEST_SUMMARY: FAIL
