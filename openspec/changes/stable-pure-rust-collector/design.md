# Design and migration requirements

Our installable collector, dependency doctor, process orchestration, evidence
validation/normalization and user-facing lifecycle implementation must all be Rust.
Repository-only release/test automation may retain existing languages; it must not
be shipped as an interpreter dependency or invoked by the installed plugin.

Use public stable coverage instrumentation and matching external LLVM/cargo-llvm-cov
interfaces. Analyze source syntax with a maintained Rust parser. Do not substitute
nightly tools, parse unstable MIR text, or hide rustc_private behind another executable.
External tools must expose supported public interfaces; preview distribution status
and LLVM format/version limitations must be documented rather than misrepresented.

Choose and document explicit source boundaries for macro expansion, derives, async,
closures, cfg/features, generated code and unexecuted functions. Function-level CRAP
is supported only where complexity and coverage have demonstrably identical owners
and scope. Failed or uncertain mapping cannot become zero, inherited parent coverage,
or an optimistic skip. New measurements get a new series and independent evidence.

Keep policy authority in released Rust Core. Preserve signed adapter requests,
artifact inventories, source/config/tool identities and all failure semantics.
Use existing evidence to characterize differences, not to claim equivalence by renaming
series. Compare stable and historical measurements explicitly before reviewed adoption.
Historical replay may run separately; required CI must not build/run the unstable driver.

Publish per-target binaries and necessary resources with existing verification,
atomic install/update and rollback guarantees. List required externally installed tools
and actionable missing/incompatible dependency commands. No forced global toolchain
changes, bundled compiler/Python default, routine user download of review archives,
or publisher OS/kernel fingerprints. Do not promise platform support without actual
native tests. Rust version changes require supported-interface/format validation,
not unconditional full toolchain redistribution or baseline reset.

## Source-level remediation and macro accuracy

Do not conflate the macro library implementation with its per-invocation generated
code. Fix dependency version, macro input and relevant cfg/features/target/build inputs.
Measure verified generated code or use explicitly validated macro measurement rules;
retain owner mapping and declared metric definitions. Obtain coverage through execution,
never infer it from source availability or from a parent invocation having executed.

For a concrete gap, produce a minimal reproducer and regression fixture, identify the
responsible layer and prepare a focused collector/dependency PR. Prefer upstream fixes;
track submission, acceptance and deployed fixed versions separately. A temporary patch
must retain source/version provenance and an exit plan. Missing stable compiler metadata
requires a tracked capability request or validated alternative, not a claim that source
access solves every case. Neither macro presence nor library popularity determines support.

Validation must include the same derive/macro source used with different invocation
inputs/configuration, generated owners with different execution outcomes, and a faulty
mapping repaired by a source-level patch. Do not change business behavior or metric
semantics solely to make a gate pass. Unresolved required measurements remain blocking.

## Shared generation and structured observations

For a diagnosed procedural-macro gap, prototype extracting generation into a normal
Rust library called by both the macro entry and the analysis integration. Prefer
optional structured observations from that same generation path over a duplicated
expansion implementation. Bind observations to dependency/source versions, invocation
inputs, cfg/features/target/build context and generated owners. Do not assume that
TokenStream text preserves hygiene, nested expansion or final coverage ownership.

The bounded Rust-only fixture at tools/quality/fixtures/rust-macro-observation proves
shared generation and execution under different invocations/features only. Production
adoption additionally requires real coverage alignment, unexecuted and ambiguous
owners, nested macros and a faulty-mapping regression. Keep missing evidence blocking.
No third-party library modifications are justified until a concrete defect is reproduced.

Keep code expansion size and cognitive complexity separate from the cyclomatic
complexity used by CRAP. Do not introduce macro-stats, cargo-expand or other unstable
compiler options into official collection or required CI. Visualization and tracing
ideas may guide implementation without adding those tools as user dependencies.
