=== Verification report ===
Timestamp: 2026-09-10T08:42:21.846997380+00:00
Profile: full
Components: backend,frontend,workflow

- PASS: secret scan (13444 ms)
- PASS: architecture audit (392 ms) - 0 violation(s), 0 blocker(s), 0 error(s), 0 warning(s)
- PASS: backend format (239 ms)
- PASS: backend Clippy (21229 ms)
- PASS: backend compile (2084 ms)
- PASS: backend tests (175986 ms) - 31 result(s)
- PASS: frontend lint (3123 ms)
- PASS: frontend format (3363 ms)
- PASS: frontend tests (12183 ms) - 85 result(s)
- PASS: frontend end-to-end tests (24299 ms)
- PASS: frontend real full-stack smoke test (22435 ms)
- PASS: frontend production build (9534 ms)
- PASS: Git hook syntax (9 ms)
- PASS: project initializer tests (480 ms)
- PASS: framework release configuration gate (80 ms)
- PASS: framework upgrade tests (1042 ms)
- PASS: template quality gate (480 ms)
- PASS: template quality gate tests (640 ms)
- PASS: observability configuration gate (79 ms)
- PASS: optional tracing and compliance configuration gate (79 ms)
- PASS: production deployment configuration gate (78 ms)
- PASS: audit retention and archive configuration gate (78 ms)
- PASS: Rust OpenAPI and Angular client generation gate (720 ms)
- PASS: supply chain security configuration gate (79 ms)
- PASS: arc-flow format (239 ms)
- PASS: arc-flow Clippy (320 ms)
- PASS: arc-flow tests (560 ms) - 58 result(s)
Quality profile null: blocked (configuration); full quality: blocked
quality workflow input is missing: /mnt/dev-ssd/gh206-runner/_work/Harness-Gate/Harness-Gate/target/arc-admin-ci-trial/source/.harness-gate/runtime/full-state.json

TEST_SUMMARY: FAIL
