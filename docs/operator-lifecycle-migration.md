# External operator lifecycle

Core no longer exposes `hooks install/status/uninstall` or `compat canary/rollback`.
Use GateProbes `operator-lifecycle-v0.1.0`, released from
`4046922f655646e7ecc77dc038df8d9e1e10a765`, archive SHA-256
`1e7989b6ed566aba1139f02903dfa1df48b480f5173ddf9c15ea32ac4f9b7e60`.
Verify the release inventory and workflow provenance before using the binary.
Linux x86_64 is the supported distribution; no other platform is certified.

The external command is `gateprobes-operator --project-root /project hooks
--core /protected/harness-gate --core-sha256 <reviewed-sha256> install`.
Replace install with status/uninstall for those operations. Read its bundled
README for trust, filesystem ownership and single-writer requirements. Core
configuration inspection remains read-only and pinned; installation never runs
workload checks. Existing hooks retain their exact script bytes and still invoke
Core `hook` / `verify --all` when Git runs them. Those execution commands remain
supported, as do `compat run` and machine-result comparison.

Canary and rollback commands move directly to `gateprobes-operator canary` and
`gateprobes-operator rollback` with the same explicit state/slice arguments.
These are external operator records, not a deployment controller or quality
policy. Core neither reads nor changes those state files during verification.

Upgrading Core does not install/uninstall hooks, move state, reset a lock, or
modify any operator file. Existing modified/foreign hooks must remain untouched.
The external tool retains exact ownership/no-clobber/rollback protection, tested
in disposable repositories including linked worktrees. Platform lifecycle tests
belong to GateProbes; Core retains execution and result-comparison regression.
Rollback to an older Core restores its old CLI but does not authorize two writers
against the same hook directory or state file. Keep one operator owner.
