# Gate Hardening Follow-up

## MODIFIED Requirements

### Requirement: Bounded scanner and log inputs

Audit and secret scanning SHALL reject any individual input file larger than
16 MiB before retaining its contents. JSON Lines error extraction SHALL use
bounded streaming passes and SHALL retain at most 20 records before the first
matching error and 30 output records.

#### Scenario: Oversized audit input fails closed

- **WHEN** an audit rule reaches a regular source file larger than 16 MiB
- **THEN** the audit operation fails with an input-size error
- **AND THEN** it does not read or scan that file

#### Scenario: Long log extraction remains bounded

- **WHEN** `parse-logs` receives a JSON Lines file with more than 30 matching
  records
- **THEN** it emits no more than 30 compact records
- **AND THEN** it does not retain the complete input log in memory

### Requirement: Preset initialization is rollback-capable

Preset initialization SHALL stage all generated configuration files, including
`.harness-gate/.gitignore` when absent, before replacing any destination. A
staging or commit failure SHALL remove temporary files and restore destinations
that were backed up during the commit.

#### Scenario: Initialization writes one complete batch

- **WHEN** a user initializes a project without existing preset files
- **THEN** flow, audit, secrets, and the generated ignore file are all written
  from the same batch
- **AND THEN** a failure before commit leaves no partially replaced file

#### Scenario: Existing broken links are treated as destinations

- **WHEN** a destination is a broken symbolic link
- **THEN** batch initialization treats it as existing and replaces the link
  itself rather than following its missing target

### Requirement: Service cleanup failures are verification failures

Verification SHALL collect errors from managed service teardown after all
workers have joined. A cleanup error SHALL be included in the failed report and
SHALL cause the command to return a verification execution error.

#### Scenario: Teardown failure is observable

- **WHEN** a started service cannot be stopped during verification cleanup
- **THEN** the verification report includes a failed `service cleanup` entry
- **AND THEN** the command exits unsuccessfully even if every step passed

### Requirement: Audit tests and lint policy remain independent of local config

Audit unit tests SHALL use repository-owned fixtures rather than the mutable
project gate configuration. Production and integration Rust sources SHALL not
use Clippy `allow` attributes to hide unused or dead code.

#### Scenario: Local audit policy changes do not break unit tests

- **WHEN** a user edits the repository's `.harness-gate/audit.toml`
- **THEN** the audit unit tests continue to exercise their fixed rule fixtures
- **AND THEN** strict all-target Clippy remains warning-free without new allows
