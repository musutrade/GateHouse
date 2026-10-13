# CodexSymphony native collector diagnostic candidate

Candidate `0.1.0-rc.7` caches successful dependency-root attribution per source
path during one LLVM export. It still checks every native symbol, mapping and
region. This avoids repeatedly traversing the same immutable root inventory in
large Axum/SQLx builds. An optional signed `source_prefix` binding maps a package's
source paths into a monorepo without basename matching or changing source hashes.

Validation on 2026-09-15: 8 project transport tests plus the dependency-cache test
passed; the full native package acceptance suite passed all 23 tests. The fresh
CodexSymphony native capture and certification completed. A prior rc.6 attempt
was interrupted after more than 20 minutes in repeated dependency attribution;
a diagnostic cache probe of that capture completed in 46.8 seconds. The probe was
not accepted as certified evidence. The independently built candidate subsequently
completed fresh capture/certification normally.

Local candidate binary SHA-256:
`b962db45f51b21fda29655292f3d9894345f4b062641bf58c4c5f51474dad7fb`.
It remains a local unsigned candidate, not a published release or the default
installed native collector. The default signed rc.6 remains installed.

The MIR series includes async state machines and generated macro control flow.
CodexSymphony selected a separate source-risk plugin for its required CRAP ≤10
policy. This change preserves MIR diagnostics; it does not relabel MIR metrics as
source metrics or relax a gate threshold.
