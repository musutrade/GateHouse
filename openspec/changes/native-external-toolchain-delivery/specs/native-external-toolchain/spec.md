## ADDED Requirements

### Requirement: Publish Core's native platform matrix
The product SHALL publish its own precompiled executables for Linux x86_64 GNU,
macOS x86_64, macOS aarch64 and Windows x86_64 MSVC. Each executable SHALL use
matching externally installed Rust and LLVM tools, and SHALL retain the existing
native measurement and Core policy rules.

#### Scenario: Each platform passes real acceptance before publication
- **WHEN** a native release tag is submitted
- **THEN** all four native host jobs build and exercise the actual packaged executable
- **AND** each target retains measurement, dependency and authenticated Core evidence
- **AND** publication signs and inventories all four executables and their distinct SBOMs
- **AND** a failed or missing platform job blocks the release

#### Scenario: A host selects a different compiler architecture
- **WHEN** the compiler host differs from the installed executable's supported host
- **THEN** dependency discovery rejects collection without switching tools or baselines

### Requirement: Preserve the native measurement engine
The independent product SHALL retain the existing native MIR owner/counter,
complexity and exact CRAP rules. Core SHALL retain thresholds, requiredness,
historical debt and non-regression authority. Stable SHALL remain an explicitly
selected candidate with no automatic fallback or baseline adoption.

#### Scenario: A derived method or expanded function is measured
- **WHEN** the old engine has a uniquely mapped owner and independently counted blocks
- **THEN** the standalone product emits the same measurement facts as native certification
- **AND** missing or ambiguous evidence remains a blocking error without a macro waiver

#### Scenario: A tool path or measurement identity changes
- **WHEN** base and head have incompatible native identities
- **THEN** comparison blocks until an explicit supported migration is supplied
- **AND** original anchors and historical baselines are not rewritten or reset

### Requirement: Distribute our code with external tools
The product SHALL distribute the precompiled driver, launcher and required
adapter/resources. Rust, LLVM, Python and native build environments SHALL remain
external. Commands SHALL validate the selected dependencies and SHALL NOT install
them or change the user's default toolchain.

#### Scenario: Matching dependencies are already installed
- **WHEN** the binary is copied outside the source checkout and given a matching sysroot
- **THEN** it performs real Cargo capture and independent native re-export
- **AND** it verifies the embedded payload before using its cache

#### Scenario: A dependency or cache is invalid
- **WHEN** the selected compiler is incompatible or a payload is missing, extra, altered or symlinked
- **THEN** execution fails without successful measurement evidence or automatic fallback

### Requirement: Use Core's existing release boundary
Native publication SHALL reuse Core's exact-version eligibility, protected-main
CI checks, release environment, inventory/checksums, Sigstore, provenance and
immutable publication. The workflow SHALL build and accept the exact binary
before publication. It SHALL NOT require a separate private-candidate signing
round or bundled-runtime installer. Existing releases SHALL remain immutable.

#### Scenario: A valid release tag is selected
- **WHEN** the tag matches the plugin manifest and an eligible main commit
- **THEN** publication requires successful native acceptance and Core's release checks
- **AND** signatures bind the exact native release workflow and tag

#### Scenario: A release fails eligibility
- **WHEN** the tag is mismatched, CI is unsuccessful, acceptance fails or the release already exists
- **THEN** publication blocks and existing assets are not overwritten

### Requirement: Keep authenticated Core transport
The product SHALL use Core's existing signed protocol v2 and digest-pinned capture
binding. Full arguments, executable identity, context, environment and replay
protection SHALL remain authenticated by Core. The executable digest SHALL cover
the embedded payload instead of requiring a separate private-runtime receipt.

#### Scenario: A signed capture selection is changed or replayed
- **WHEN** arguments are changed after signing or a nonce is reused
- **THEN** Core blocks the call before exposing collection evidence
