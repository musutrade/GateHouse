## ADDED Requirements

### Requirement: Native macro syntax inventory SHALL have an independent identity
The development analyzer SHALL expose `--native-inventory source.rs` with a distinct analyzer/rule identity and `certified_llvm_mapping: false`. Default invocation SHALL retain legacy output behavior and SHALL NOT accept the new macro grammars implicitly.

#### Scenario: Explicit native inventory
- **WHEN** a supported tracing field or select branch is parsed in native mode
- **THEN** its value expressions, source spans and closure ownership are retained
- **AND** source counters are not accepted as certified runtime CRAP

### Requirement: Unsupported native macro grammars SHALL fail closed
Unknown paths, unresolved aliases and unsupported generated-code macros SHALL produce errors in production source. Unconsumed tokens SHALL NOT be discarded.

#### Scenario: Unresolved generated code
- **WHEN** production source contains task_local, macro_rules or an unsupported macro
- **THEN** inventory fails with the macro location and diagnostic
- **AND** no complete production inventory or quality pass is asserted

### Requirement: Native parser delivery SHALL retain reproducible validation
Implementation SHALL proceed through reproduction, bounded parsing, negative tests and source replay in one delivery, before downstream LLVM certification. Tests SHALL demonstrate independent identity and legacy compatibility; true source replay SHALL retain hashes and failures.

#### Scenario: Retained source still contains unsupported macros
- **WHEN** the new grammar resolves a tracing or select syntax error but another macro remains unsupported
- **THEN** the whole file remains failed and its diagnostic is retained
- **AND** no backend-wide completion claim is made
