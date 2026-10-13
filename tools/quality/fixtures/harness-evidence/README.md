# Synthetic normalized evidence fixtures

`polyglot.json` contains TypeScript, Rust, Python and Java records using the
existing [project subjects and source bytes](../project-model/README.md).
`artifacts/` retains illustrative tool-native JSON with original byte digests.
No real Istanbul, LLVM, coverage.py or JaCoCo collector ran.

All records expose the same generic metric names and values, but have distinct
collector/tool/rule/runtime/normalization identities and incompatible series.
Every record deliberately contains all six capability states (including an
explicit measurement error), so envelope validity never implies release success.
`requirements.json` exercises the same availability rules across components.
`expected.json` is the caller's explicit synthetic commit/base/target/run context.

`negative.json` defines single mutations of the first record using a key/index
path, either a replacement `value` or `delete: true`, and the expected error.
The [tests](../../tests/test_harness_evidence.py) also exercise duplicate IDs,
symlink escape, byte tampering, all series identity fields, missing base,
malformed JSON, typed-value boundaries and required/informational behavior.

The [contract record](../../../../docs/quality/harness-evidence.md) specifies
validation, canonical serialization and capability policy boundaries.
