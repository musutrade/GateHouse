# GH-284 / GH-282 / GH-290 integrity repairs

These repairs preserve the Engineering Policy: Rust Core owns delivery decisions;
requiredness, metric definitions, thresholds, baseline identity, ratchet and
aggregation remain unchanged. No gate is waived, no frozen reference is rewritten,
and no prior evidence is assigned a new measurement-series or commit identity.

## GH-284: installer lifetime

`install.sh` keeps its allocated temporary root in script-lifetime state. Its
EXIT handler saves the original status before clearing atomic staging and that
root. Paths remain quoted arguments, never interpolated trap source. HUP/INT/TERM
retain 129/130/143. The existing install activation and verification order remain.

The actual Bash fixture uses isolated `TMPDIR` paths containing spaces and a
single quote, retains a neighboring sentinel, and checks allocated installation
roots after Core binary, source, Rust-only and combined success. Download (22),
build (23), activation (1), checksum and signature failures preserve the old
installation and leave no temporary root. Foreground Python drivers deliver each
signal at a fake-copy ready/release barrier, record the active root/status, then
check root and atomic-file cleanup. A `finally` releases the barrier on failure.

Entry: `HARNESS_GATE_INSTALL_TEST_EVIDENCE=/absolute/fresh/evidence bash
tools/release/tests/test_install.sh`. Keep the command's stdout/stderr separately.
The fixture copies its files and lifecycle logs to that directory, retaining its
original tree if copying fails. Without that variable, failed runs keep their
temporary tree. Platform dispatch is simulated; this is not native release matrix
certification and does not install anything into the host.

## GH-282: complete inventory and type integrity

Only canonical imports and direct `httpResource` calls are accepted. Unsupported
uses fail even alongside valid calls, in another scanned file, or as the only use.
The checker derives the complete declaration set from the expected generated
types and compares every Request/Response structurally in both directions.

`business.test.cjs` adds two actual `cli.cjs discover/collect` regressions:

- `real discover and collect reject every indirect httpResource use across the
  scanned inventory`: aliases, parameter passing, properties, indirect calls,
  shadow declarations and exports at three placements, with a valid control.
- `real CLI checks Request and uncalled Response structures with rebound receipts
  and unchanged stamps`: field type, requiredness and enum drift on both
  declarations, plus whitespace-only success.

Each mutation rewrites captured input and consumer-source hashes. Assertions name
the inventory/type-integrity rejection, ensuring obsolete receipt hashes cannot
explain a pass. Every invocation retains its request, complete original input
strings, baseline, status and CLI stdout/stderr with
`HARNESS_GATE_HTTP_TEST_EVIDENCE=/absolute/fresh/evidence`; failures also retain
their temporary directory by default. Run the existing collector `npm test` from
`tools/quality/http-json-contract` to include all prior provenance negatives.
The implementation hash naturally produces a new series; collect fresh evidence
and approve its identity instead of reusing old series evidence.

## GH-290: explicit scope validation

Rust selects the existing schema branch for each of the seven supported scope
kinds, then applies its required/allowed fields and value domain. The shared
helper applies the JSON domain even when invoked through independent `select`,
including rejection of canonical IDs with a trailing newline. Document validation
permits nonempty aliases before compiler resolution; resolved policy and selection
enforce canonical identities. Reference error ordering for unknown subjects stays.

Unit entries in `quality-core/policy_tests.rs`:

- `scope_documents_validate_every_supported_kind_and_reject_invalid_fields`
- `resolved_scope_validation_preserves_alias_compilation_and_canonical_selection`

The real CLI entry
`explicit_policy_scopes_include_failing_components_and_reject_invalid_input`
invokes `tools/quality/fixtures/workflow/compiler/scope_acceptance.py`. A valid
component control passes; the valid project selects both components and reflects
the failing one. Unknown/extra scopes use the existing runtime-error exit 1 with
an exact `ERROR [E1000]` scope diagnostic and remove an earlier verified pass at
the same output path. A separate expected-error field distinguishes these input
errors from the valid project's exit 1 and retained FAIL report. It supplies complete bound
source/artifact/evidence/context inputs through explicit `--project --policy`,
without `--state`. Invoke the Python fixture with `--harness-gate /path/to/binary
--output /absolute/fresh/evidence` to retain all successful and failed case bytes.
The Rust test retains the fixture tree on failure.

Reuse the existing frozen Core corpus and
`compiled_configuration_matches_direct_rust_and_rejects_stale_inputs` for alias
compilation/state verification. `quality-core/policy.rs` is already in the
certified Rust source boundary; this batch adds no risk source selection. Final
exact-commit coverage/risk and required gates remain the verifier's responsibility.
