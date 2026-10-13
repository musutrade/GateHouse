# Tasks

- [x] T1: Record the user-approved normative policy and guard legacy publication.
- [ ] T2: Inventory legacy collector/runtime/installer behavior and stable CI; define the pure-Rust replacement contract and supported measurement boundaries.
- [ ] T3: Implement pure-Rust entry, dependency checks, stable coverage collection and source complexity analysis without compiler-private APIs or Python runtime dependencies.
- [ ] T4: Implement protocol-compatible validation/normalization, preserved Core authority and explicit capability/identity failures; investigate macro/derive gaps with minimal reproductions and focused source-level PRs, validating per-invocation generated code and real coverage before claiming accuracy.
- [ ] T5: Implement lightweight verified publication/installation/upgrade/rollback with user-installed dependencies and measured artifact sizes.
- [ ] T6: Run real multi-toolchain, cross-host, negative, source-boundary and old/new measurement comparisons; review the new series transition without baseline reset.
- [ ] T7: Update CI with stable-only acceptance, publish support/limitations documentation and complete all required checks.
- [ ] T8: Remove the legacy publication hold only with reviewed replacement acceptance; production publication remains separately protected.

Documentation or a prototype alone does not complete T2–T8. The policy PR closes
only T1; the implementation issue remains open until the runnable replacement,
release preparation and actual acceptance are complete.

## Macro observation work under T4

- [x] Validate shared generation in a normal Rust library with an actual proc-macro consumer, different inputs and feature selections.
- [ ] Bind structured observations to source/configuration and unambiguous generated owners.
- [ ] Verify actual stable coverage for executed/unexecuted owners, nested expansions and a deliberately faulty mapping.
- [ ] Prepare a focused upstream PR only for a reproduced library defect; track deployed fixes separately.

The first prototype does not complete T4 or authorize changing the accepted series.
