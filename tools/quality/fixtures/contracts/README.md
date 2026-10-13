# Synthetic cross-component contracts

The four component sources and local coverage records derive from the GH-111/
GH-112 synthetic fixtures. The API contract now requires `primaryEmail`; its
retained baseline, frontend expectation and generated client require `email`.
`artifacts/facts.json` declares synthetic measurements without an OpenAPI tool.

`project-config.json` binds the model and policies using repository-relative
paths. `single-rust-config.json` selects only the existing Rust function. See
[commands and reporting semantics](../../../../docs/quality/project-reporting.md).
The single-component replay exits 0; the four-component replay exits 1 despite
four green local gates. Raw baseline, head contract, client and consumer bytes
remain linked in every contract gate for machine inspection.

These files test architecture and provenance. No Angular, Python, Java, OpenAPI
comparison or client-generator adapter implementation/certification is claimed.
