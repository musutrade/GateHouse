# Rust native missing-file classification (GH-221)

## Why
The historical LLVM file summary lists 42 of 47 pinned backend Rust files. Missing
rows do not prove absence of executable code. GH-220 / PR #223, merged as
bc06455814f03a5a87e4f2340f1f026387851690, supplies a separate complete MIR mapping.

## What Changes
Execute F0–F5: bind generic file classification to authenticated complete sources,
compiler definitions, selected build configuration, runtime owners and raw LLVM.
Retain per-file provenance and unresolved historical gaps. Compile native positive
fixtures; mutated evidence is negative evidence only.

## Impact
Development measurement tooling, fixtures and evidence only. Preserve line/region
>=80%, CRAP <=30, inventory entries, historical series and debt/baseline rules.
No Arc-Admin business-test changes, database access, baseline acceptance, official
gate migration or GH-215 completion. ADR-0040/0044/0049 remain authoritative.

## Goals
Account for all 47 files without interpreting module forwarding or macro syntax as
machine code evidence. Reproduce all conclusions within this workspace.

## Non-goals
Reconstructing unauthenticated historical LLVM configuration or certifying other
features, targets or test selections from this capture.

## Success Metrics
All input hashes reconciled; each row links compiler and LLVM evidence or reports
measurement_error; real native positive/negative tests and required checks pass.

## Risks
Incomplete historical archives or missing binaries restrict revalidation. Report
these limits explicitly; reviewed archived evidence is not a fresh native capture.
