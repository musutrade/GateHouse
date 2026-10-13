# Native standalone local acceptance — 2026-09-14

The native delivery branch is based directly on main `04344d6`, independently of
the stable candidate in PR #261. The old driver, normalizer, classifier, policy
bridge and projection sources are byte-identical to that main revision.

The driver was rebuilt from source with Rust 1.97.1 and then packaged with a
std-only launcher, existing Python modules/schemas and authenticated license
notices. Local artifact: **1,339,320 bytes**, SHA-256
`ad2f9648512fc7b1fecce229e305d92968d09d28d5bd57b49f5579bdd1b928b4`.
This is an unsigned development binary, not a signed release.

- **16 real native tests passed, zero skipped**, including independent macro,
  derive, constructor, closure and async counts; two generic instances; selected
  Cargo features and test exclusion; exact old-engine report equivalence;
  independent LLVM re-export; damaged, missing and ambiguous evidence rejection.
  Conditional file classification retains the old alternate-feature witness: missing
  proof yields explicit per-file errors; compatible anchored proof completes classification.
- The copied standalone executable ran outside the repository, using external
  Rust and isolated Python. Missing dependencies, compiler overrides and changed,
  extra or symlinked cache payloads failed. A separate real Rust 1.98.1 probe was
  rejected as an incompatible toolchain. No stable fallback was attempted.
- Actual released Core **0.4.2** authenticated the executable and full signed
  protocol-v2 request, accepted native collection, and blocked tampered arguments
  and replay. The existing policy fixture retained unchanged debt at CRAP 56,
  blocked changed debt, and blocked regression from CRAP 7 back to 56. The packaged
  `evaluate` command also returned Core's expected failing aggregate.
- **88 release tests passed**, including the shared inventory/signature-set
  invariants, exact version/tag/main-CI eligibility, and native publication job
  dependencies. YAML parsing and syntax checking of all 16 shell steps passed.
- Documentation/schema consistency, Rust formatting, strict OpenSpec validation
  and `git diff --check` passed. The actual packaged binary/SBOM/inventory checksums
  passed; the unsigned package was rejected for missing Sigstore products.

Machine results and retained logs are in
[validation.json](native-external-evidence/validation.json),
[native acceptance](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/native-external-evidence/native-acceptance.log),
[release tests](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/native-external-evidence/release-tests.log) and
[build inventory](native-external-evidence/build.json). The machine record names
the complete local evidence directory and its independently retained fixture
anchor. Raw local evidence remains available there; it is not a production trust
receipt or a newly accepted baseline.

Local host: Ubuntu 26.04, Python 3.14.4, matching external Rust/LLVM 1.97.1/22.1.6.
The initial precompiled-driver smoke test ran before adding `rustc-dev`; the
subsequent source build used that build-only component. Default Rust remained
`stable-x86_64-unknown-linux-gnu`. The executable embeds no toolchain/interpreter.

The first hosted Ubuntu 24.04/Python 3.12 native acceptance succeeded at commit
`81c0d173fc86cfa5a23486320345a3fe97e0a511` in
[run 34819401620](https://github.com/musutrade/Harness-Gate/actions/runs/34819401620).
That is separately identified in [hosted-initial.json](native-external-evidence/hosted-initial.json);
it predates the conditional-classification/output-preservation follow-up. The
latest code still requires its own hosted check. Protected-main CI, Sigstore,
provenance and immutable tag publication are separate release steps. Existing
signed RC3 assets are unchanged.
