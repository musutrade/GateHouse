# Design

## Contract and authority
Classification is a separate versioned report, never a coverage series replacement.
Its identity includes the complete lexical inventory digest, mapping series,
compiler/tools, flags, cfg/target/features/test selection, Cargo inputs and trusted
capture anchor. Verify the entire mapping before classifying any file. A missing
owner/export or tampered artifact invalidates every absence inference.

Rows retain source hashes, compiler-loaded units, declaration and constant-evaluation
IDs, expansion call/definition chains, runtime owner IDs, LLVM symbols and actual
block counts. Module declaration spans identify forwarding declarations; child
module code is attributed to its own source origins. Macro definitions and calls
are inspected through compiler expansion and CTFE/runtime roles, never a filename
allowlist or a count of AST functions.

A file with runtime origins has measured coverage, including real zero counts with
positive denominators. A loaded file with complete compiler evidence and no runtime
origins can be not_applicable for this compilation, with declaration/compile-time
reasons and no invented 0/0 metrics. Nonloaded files remain measurement_error unless
an authenticated alternate-feature compilation of identical sources proves runtime
origins for the file. Both complete mappings must share tools, target, non-feature
cfg, Cargo inputs and test selection; only feature selection may differ. Such exclusion
is scoped to the declared configuration, never all configurations. Unsupported
syntax/provenance remains unresolved. Excluded test definitions cannot prove
production absence.

## Archive replay
A reviewed archive can support inspection of already accepted GH-220 mapping when
its committed hash, manifest and retained artifacts match. Missing original binaries
are recorded; do not invoke their external paths or claim a fresh native re-export.
Any new compilation receives its own source/config/tool identity and anchor. An
old llvm-file-summary-unfiltered/1 gap remains measurement_error without its own
authenticated configuration and exports, even when new MIR evidence classifies it.

## Verification
Native fixtures cover declaration/module forwarding, macro_rules, derive,
task_local, cfg and unexecuted owners. Omitted exports/owners, source/config/CTFE
changes, resealing and cross-series substitution fail closed. Retain full backend
rows and source reconciliation, exact commands, raw fixture captures and check logs.
Full classification neither passes thresholds nor accepts a baseline. Final
acceptance belongs to controller review/CI; tasks are checked only with evidence.
