# Synthetic collector fixtures

`synthetic.py` is both an importable internal reference and an external stdin/stdout
adapter. It copies already-retained GH-112 synthetic raw artifacts into the
requested output root and preserves their normalized records. No ecosystem tools
or Rust instrumentation are invoked. The four existing component fixtures remain
synthetic examples, not certified Rust/TypeScript/Python/Java integrations.

`cases.json` enumerates positive, unsupported, measurement-error, stale-commit,
duplicate-subject, tamper, inventory, path and transport cases with expected typed
errors. The unsupported scenario consumes the existing explicit unsupported
branch capability without inventing a percentage. Every transport-independent
case runs through both adapters; process-only cases run externally.

Run `python3 -m unittest discover -s tools/quality/tests -p 'test_collector_runner.py' -v`.
Additional cases cover request mutation, required fields, fresh output roots,
source/series integrity, capability measurement errors, all four synthetic
components, internal exceptions, mixed error/evidence, and POSIX descendants
holding stdout open during timeout. A consumer test verifies identical capability
availability results without knowing the adapter transport.
