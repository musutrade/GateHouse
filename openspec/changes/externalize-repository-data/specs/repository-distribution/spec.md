## ADDED Requirements

### Requirement: Lean source distribution
The repository SHALL retain source and small build fixtures while distributing large regression data through immutable, digest-pinned assets.

#### Scenario: Ordinary build
- **WHEN** a developer clones the source repository and builds the CLI
- **THEN** the build does not download historical evidence or regression archives

### Requirement: Verified regression preparation
Full regression consumers SHALL restore the exact bytes declared by their committed manifest and SHALL fail on unavailable, changed or malformed data.

#### Scenario: Corrupt cache
- **WHEN** a cached archive does not match its pinned size or SHA-256
- **THEN** preparation fails and the consuming required CI job cannot pass

#### Scenario: Base and head comparison
- **WHEN** quality measurements compare two revisions
- **THEN** each snapshot restores its own manifest without changing measurement policy or resetting accepted debt

### Requirement: Migration lineage
GateHouse SHALL preserve the CLI name, original evidence lineage and protected required checks while excluding old data-heavy history from new clones.

#### Scenario: Initial import
- **WHEN** the first source import is compared against the placeholder repository seed
- **THEN** an explicit reviewed bridge binds measurement to the accepted Harness-Gate source commit rather than inventing a fresh baseline
