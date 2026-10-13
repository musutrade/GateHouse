=== Verification report ===
Timestamp: 2026-09-10T08:56:14.955917355+00:00
Profile: full
Components: backend,frontend,workflow

- PASS: secret scan (13367 ms)
- PASS: architecture audit (400 ms) - 0 violation(s), 0 blocker(s), 0 error(s), 0 warning(s)
- PASS: backend format (239 ms)
- PASS: backend Clippy (19626 ms)
- PASS: backend compile (2245 ms)
- PASS: backend tests (172137 ms) - 31 result(s)
- PASS: frontend lint (2961 ms)
- PASS: frontend format (3444 ms)
- PASS: frontend tests (11951 ms) - 85 result(s)
- PASS: frontend end-to-end tests (24605 ms)
- PASS: frontend real full-stack smoke test (21400 ms)
- PASS: frontend production build (9704 ms)
- PASS: Git hook syntax (9 ms)
- PASS: project initializer tests (481 ms)
- PASS: framework release configuration gate (79 ms)
- PASS: framework upgrade tests (962 ms)
- PASS: template quality gate (480 ms)
- PASS: template quality gate tests (640 ms)
- PASS: observability configuration gate (79 ms)
- PASS: optional tracing and compliance configuration gate (79 ms)
- PASS: production deployment configuration gate (79 ms)
- PASS: audit retention and archive configuration gate (79 ms)
- PASS: Rust OpenAPI and Angular client generation gate (720 ms)
- PASS: supply chain security configuration gate (78 ms)
- PASS: arc-flow format (239 ms)
- PASS: arc-flow Clippy (239 ms)
- PASS: arc-flow tests (560 ms) - 58 result(s)
Quality profile null: blocked (configuration); full quality: blocked
quality workflow input is missing: /mnt/dev-ssd/gh206-runner/_work/Harness-Gate/Harness-Gate/target/arc-admin-ci-trial/source/.harness-gate/runtime/full-state.json

TEST_SUMMARY: FAIL
