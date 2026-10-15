# Issue #30 acceptance and compatibility review

Scope: generic, explicitly selected, offline **data-only preset** packages.
Core supports `harness-plugin-lock/v1`, `harness-plugin/v1` and
`harness-preset-catalog/v1`. GateProbes product names and dependency lists are
external data; this change introduces no ecosystem enumeration or executable
plugin registration. Adapter v2 signatures, host capabilities and replay state
remain independent requirements. Existing quality presets remain unchanged;
the new minimal Rust/Python presets explicitly perform execution checks only.

The Core CLI suite exercises unknown publisher names, init/config/doctor/plan/
verify and staged indexed-lock behavior, compatibility failures, missing tools
and packages, manifest/file corruption, cryptographic signature forgery,
untrusted/project-proposed keys, duplicate keys/pins, unsafe paths, symlinks,
nonregular/oversized inputs and unlisted catalog references. The critical path
`plugins.signature` independently binds the signature-verification source to a
CLI rejection before any invocation directory or verification report exists.
No existing mandatory path or applicability rule is removed.

Local validation: 528 Rust nextest cases passed, zero skipped; format and
Clippy with warnings denied passed. GateProbes' eight preparation/install tests
passed. The real two-ecosystem script completed 29 subprocess steps: actual
operator signing, preparation, planned installation, fresh Rust/Python
init→config check→doctor→plan→verify, missing-trust/corruption rejection, explicit
upgrade, byte-for-byte rollback and subsequent verification. Rust compiles
offline and Python uses standard-library unittest. Per-command logs and the
Core binary digest accompany `report.json`; CI uploads no private key.

A CLI authority regression asserts that plugin-only keys cannot populate adapter
request trust, adapter-only keys cannot populate plugin trust, and explicit
selections of both remain independent. The flattened plugin argument uses its
own parser identity. Full Windows staged verification additionally guards Git
checkout prefixes against mixed Win32 verbatim/slash paths; the same source is
analyzed under Linux, macOS and Windows compiler configurations.

The Linux Test job checks out GateProbes at immutable reviewed source
`53c01505bed2ff61dfad1d106abbec2dcd961d05` and repeats that acceptance against
the Core binary built from the current candidate. This source acceptance does
not change GateProbes' independently pinned legacy Core release contract.
Three-platform Core tests and all existing required quality jobs remain enabled.

Measurement review: the production config boundary now includes `plugins.rs`.
Required AST/LLVM risk selection adds exactly that source and retains every
earlier source and hotspot. Its explicit new selection is
`gh30-plugin-lock/1`; BOTH base and head are recollected using the identical
expanded inventory. Analyzer `harness-gate-rust-measure/0.3.1`, McCabe rule,
instrumentation, target configuration, production coverage, CRAP thresholds,
critical-path threshold and mandatory policy are unchanged. Previous reports
remain intact and incompatible/mixed series remain rejected. Three-target
analyzer/instrumentation tests include the new preflight source.

Migration is opt-in by adding a reviewed lock; no-lock projects retain explicit
external directory behavior. Unknown schema/protocol/version/platform fails
closed. GateProbes install plans retain readable exact selections and previous/
next lock material; receipts precede package installation. Apply verifies both
prepared and installed bytes using only operator-supplied keys. Rollback restores
exact previous lock bytes and preserves installed packages. Stale plans and
unrelated later lock edits are rejected. No installation automatically adopts a
measurement series/baseline, edits flow/quality policy, expands a waiver, skips a
required gate, or replaces a system toolchain.
