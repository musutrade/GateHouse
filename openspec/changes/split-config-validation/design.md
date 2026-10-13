# Design: Split Configuration Validation

`config/validation/mod.rs` remains the private implementation entry point for
`FlowConfig::validate`. It calls private primitive and step validation helpers.
No child module is exposed outside `config`.

Validation order, conditions, and error messages are retained verbatim.
