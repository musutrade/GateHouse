## ADDED Requirements

### Requirement: Read the project-owned CRAP ceiling
The standalone native evaluation command SHALL require an explicit project root
and quality policy binding. It SHALL resolve `policy_file` and `rule` through
`quality.toml`, validate the project with Core, and use the selected rational
CRAP limit for every measured function. It SHALL NOT substitute a fixed ceiling.

#### Scenario: The configured ceiling changes
- **WHEN** identical certified facts are evaluated against two configured limits
- **THEN** Core applies the respective exact rational limits, including values above 30
- **AND** the measurement facts and series do not change with the limit
- **AND** the report retains the selected policy inputs and effective rule

### Requirement: Reject invalid or unrelated policy inputs
The command SHALL reject absent/invalid configuration, ambiguous rule identities,
escaping policy paths, changed configuration inputs, component scope mismatch
and exact measurement-series mismatch. It SHALL NOT fall back to an unconfigured
or historical invocation path or claim complete profile assurance.

#### Scenario: The binding names another measurement series
- **WHEN** the configured series differs from the certified native projection
- **THEN** evaluation blocks and reports the two identities
- **AND** the projected facts remain reviewable without a successful Core report
- **AND** historical anchors and baselines are not adopted or reset

### Requirement: Preserve debt and non-regression authority
Core SHALL retain requiredness, absolute checks for new/changed/selected functions,
unchanged legacy-debt eligibility and non-regression. The native bridge SHALL
require an explicit debt ratchet and reject policies that disable required
failure, non-regression or the required baseline provider. A configured
prohibition on legacy debt SHALL make every function's CRAP ceiling absolute.

#### Scenario: A higher ceiling still contains a regression
- **WHEN** a function worsens from its anchored baseline while below the ceiling
- **THEN** Core rejects the regression
- **AND** changing the ceiling does not reset the baseline or waive measurement errors
