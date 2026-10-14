# Frozen legacy collector delivery protections

This directory contains historical test inputs, not an active Core product.
GateProbes owns any future development or revival. Signing/publication of the
bundled product remains suspended; restoring source does not authorize release.

`manifest.json` binds each original file to GateHouse commit
`da8e8a58a4ac5959dc0346b79bad4a4437b964cb`. The snapshot bytes are unchanged.
It contains the 24 former plugin delivery scripts/templates, six test modules,
two manual measurement scripts, four imported quality helpers, the original
contract-test fixture helper, and two historical measurer identity files.
It contains no Rust plugin implementation, compiler, runtime or binary.

Ordinary Core release CI discovers `tools/release/tests/test_frozen_legacy.py`,
which verifies the snapshot, creates a disposable layout, links current shared
Core tools/protocols, and executes all 64 original methods without skips. The
method list comes from the original 102-test release-suite execution. The runner
rejects changed/missing/extra frozen files, restored active copies, missing shared
inputs and any difference in the executed method inventory. These synthetic
protections do not claim actual product signing or measurement certification.

Shared tools remain editable only in Core; none are frozen copies here.
Historical workflow bytes, identities and migration holds are unchanged.
See [cleanup and rollback](../../docs/quality/issue-13/README.md).
