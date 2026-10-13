# GH-183 report risk repair

The hosted risk comparison for PR #192 at `124467f954162947131814083ad587a3e65bb2f0`
rejected `verify/report.rs::write` and `verify/report.rs::assess_evidence`.
The former had 72.60% line coverage, 65.89% region coverage and CRAP 30.069;
the latter had 79.31% line coverage, 83.94% region coverage and CRAP 24.906.
The existing requirements remain 80% line/region coverage and CRAP at most 30.

Report-format generation now has an explicit boundary that collects all rendering
and writing errors before finalization. Evidence declaration separates step/retry
bindings from coordinator-owned report outputs; closed-set validation and artifact
binding remain in `assess_evidence`. Publication order, error propagation and
incomplete machine-result behavior are unchanged.

The existing AST analyzer measures `write` at complexity 14 (previously 21),
`write_report_documents` at 8, `assess_evidence` at 10 (previously 21),
`declare_step_evidence` at 7 and `declare_report_outputs` at 6.

Five regression tests exercise successful HTML/JUnit publication, failed writes
for each report format, registry/manifest publication failures, missing or stale
step bindings, and failed execution with missing evidence. They assert externally
visible failure/incomplete results and manifest integrity rather than helper layout.

Validation:

- Formatting check and Clippy (warnings denied) passed.
- Focused report suite: 42 passed, zero failed/ignored.
- Critical-path tooling suite: 17 passed; actual inventory bindings/probes validated.
- The critical-path source hashes and moved probes were refreshed without changing
  test observables, mandatory paths, applicability or thresholds.
- Full production coverage and base/head risk results must come from fresh hosted
  CI. No new coverage percentages or complete risk pass are claimed locally.
- Disk cleanup and quota changes are deferred by user instruction. This repair
  does not change them or discard existing evidence.
