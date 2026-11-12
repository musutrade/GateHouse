# GH-196 audit exclusion measurement review

This is the normative additive measurement-selection delta for PR #196.
`gh196-audit-exclusions/1` adds `audit/scanner/filter.rs` to the complete
`gh33-findings/1` inventory. No earlier production source or selected hotspot
is removed. The source was already owned by the blocking audit boundary in
`production-source-2`; this change also collects its function risk evidence.

The pinned analyzer remains `harness-gate-rust-measure/0.4.0`, with the same
`mccabe-rust-3/1`, closure instrumentation, UTF-8 insertion mapping and compiler
configuration identity. Tests check the new source's AST, exclusion of test
functions and instrumentation round trip for Linux, macOS and Windows target
configurations.

Both the actual base and candidate must be freshly collected with this exact
expanded inventory and tool identity. Comparisons reject the previous selection
and mixed selections. Existing reports retain their original identities; no
baseline is adopted or rewritten. CI continues to require real AST/LLVM joins,
coverage, risk and the Required Quality Aggregate.

The existing 80% line/region rules, exact CRAP <=30 requirement, changed-function
selection, debt retention and no-regression ratchet remain unchanged. There is
no new waiver or synthetic executable counter. Evidence is produced by the
PR's paired risk shards and ordinary quality artifact assembly.

`src/audit/tests.rs` remains a test-only module: `audit/mod.rs` declares it under
`#[cfg(test)]`, and `production-source.json` already excludes it for that reason.
The risk scope validator now recognizes this existing classification. Unknown
audit sources and undeclared nested test paths still fail measurement review.

Rolling back the selection change restores the previous inventory and its
rejection of changes to the unmeasured filter. Historical evidence and thresholds
remain intact; reports from the two selections must never be mixed.
