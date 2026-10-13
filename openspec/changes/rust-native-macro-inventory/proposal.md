# Native Rust macro source inventory

## Why
GH-219 addresses tracing and select grammar failures found by the real Arc-Admin pilot in GH-215. The existing reference parser is not a certified native production inventory.

## What Changes
Add an opt-in source inventory with a separate identity, bounded macro grammars, strict unknown-macro diagnostics and retained source replay. Preserve the legacy default mode.

## Goals
Preserve source expressions, spans and closure ownership inside supported macros; expose unresolved grammars honestly.

## Non-goals
LLVM mapping, generated-code certification, CRAP, missing-file classification and authority transfer belong to GH-220, GH-221 and GH-215.

## Success Metrics
Macro counter/ownership and negative tests pass; the existing Python suite passes; all 47 retained source files have hashed replay outcomes in both modes. Unknown grammars remain failures.

## Impact
Development tools and documentation only. Low runtime/performance risk because the mode is explicit and no collector switches to it. Medium interpretation risk is bounded by separate identity and certified_llvm_mapping=false. No untrusted code executes during syntax inventory. See ADR-0040 and ADR-0049 and the existing dogfood proposal.
