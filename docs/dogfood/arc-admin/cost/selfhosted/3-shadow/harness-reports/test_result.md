=== Verification report ===
Timestamp: 2026-09-10T09:10:05.413410894+00:00
Profile: full
Components: backend,frontend,workflow

- PASS: secret scan (13371 ms)
- PASS: architecture audit (409 ms) - 0 violation(s), 0 blocker(s), 0 error(s), 0 warning(s)
- PASS: backend format (238 ms)
- PASS: backend Clippy (19070 ms)
- PASS: backend compile (2166 ms)
- PASS: backend tests (169514 ms) - 31 result(s)
- PASS: frontend lint (2962 ms)
- PASS: frontend format (3363 ms)
- PASS: frontend tests (12045 ms) - 85 result(s)
- PASS: frontend end-to-end tests (24538 ms)
- PASS: frontend real full-stack smoke test (22039 ms)
- PASS: frontend production build (9306 ms)
- PASS: Git hook syntax (9 ms)
- PASS: project initializer tests (480 ms)
- PASS: framework release configuration gate (80 ms)
- PASS: framework upgrade tests (1043 ms)
- PASS: template quality gate (480 ms)
- PASS: template quality gate tests (640 ms)
- PASS: observability configuration gate (78 ms)
- PASS: optional tracing and compliance configuration gate (78 ms)
- PASS: production deployment configuration gate (79 ms)
- PASS: audit retention and archive configuration gate (78 ms)
- PASS: Rust OpenAPI and Angular client generation gate (721 ms)
- PASS: supply chain security configuration gate (79 ms)
- PASS: arc-flow format (240 ms)
- PASS: arc-flow Clippy (239 ms)
- PASS: arc-flow tests (560 ms) - 58 result(s)
Quality profile null: blocked (configuration); full quality: blocked
quality workflow input is missing: /mnt/dev-ssd/gh206-runner/_work/Harness-Gate/Harness-Gate/target/arc-admin-ci-trial/source/.harness-gate/runtime/full-state.json

TEST_SUMMARY: FAIL
