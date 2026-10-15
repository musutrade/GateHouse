## ADDED Requirements

### Requirement: Event-specific CI timing evidence
The repository SHALL analyze PR and push runs against their respective required
job inventories and SHALL separate critical latency from summed runner work and
advisory lifecycle.

#### Scenario: Push performance baseline is the final required child
- **WHEN** a complete successful push run is normalized
- **THEN** push-only required jobs participate in critical-path analysis
- **AND** a missing, skipped or unsuccessful required child rejects the sample

#### Scenario: Mixed event cohort
- **WHEN** PR and push observations are supplied as one timing distribution
- **THEN** normalization rejects the mixed cohort

### Requirement: Evidence-backed scheduling optimization
A coverage scheduling optimization SHALL preserve package/test scope, fresh
coverage, exports, thresholds and failure propagation, and SHALL document sample
size, resource costs, applicable execution path and rollback.

#### Scenario: Runner or export fails
- **WHEN** either coverage backend fails a test or an export command
- **THEN** collection fails without substituting old evidence or success

#### Scenario: Small experiment
- **WHEN** fewer than five comparable A/B samples are available
- **THEN** the report explicitly states the sample limitation
- **AND** it does not present observational timing differences as causal hosted speedup
