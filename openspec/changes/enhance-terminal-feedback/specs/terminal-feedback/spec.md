# Terminal Feedback Specification

## ADDED Requirements

### Requirement: Terminal-aware color control

The CLI SHALL provide a global `--color <auto|always|never>` option. In `auto` mode, it SHALL color human-readable terminal output only when the relevant stream is interactive and `NO_COLOR` is not set. `always` SHALL force color, and `never` SHALL disable it.

#### Scenario: Forced color in redirected output

- **WHEN** a user runs a human-readable command with `--color always`
- **THEN** status output contains ANSI styling sequences

#### Scenario: Disabled color

- **WHEN** a user runs a command with `--color never`
- **THEN** its output contains no ANSI styling sequences

### Requirement: Verification progress

The CLI SHALL render one dynamic progress indicator on stderr while `verify` executes in an interactive terminal. It SHALL include the two mandatory gates and every selected configured step, and SHALL finalize before the verification summary.

#### Scenario: Redirected verification output

- **WHEN** verify stderr is not an interactive terminal
- **THEN** no progress redraw control sequences are emitted

### Requirement: Machine-readable compatibility

The CLI SHALL NOT add color or progress output to JSON command output.

#### Scenario: JSON audit output

- **WHEN** a user runs `audit --json`
- **THEN** stdout remains valid JSON without ANSI escape sequences
