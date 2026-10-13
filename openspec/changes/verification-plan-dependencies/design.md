# Design: Verification Plan Dependency Foundation

`StepConfig.depends_on` defaults to an empty list. Validation builds a DFS over all configured steps and fails closed for invalid references and cycles. The private `verify::plan` module expands dependencies of profile-selected steps, then performs a stable depth-first topological emission using configuration order as the tie breaker. Existing execution and result reporting consume the ordered list unchanged.

Rollback is a revert of the field, validator, plan module, and tests; old TOML files remain valid because the field is optional.
