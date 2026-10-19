# GitHub consumer moved to GateProbes

The implementation and platform behavior tests now belong to [GateProbes github-check](https://github.com/musutrade/GateProbes/tree/main/plugins/github-check). Use the independently released `github-check-v1.0.0` archive, verify its fixed digest and provenance, and follow its operator configuration documentation. The old `tools/consumers/github_check.py` path is removed; no forwarding launcher executes a downloaded consumer automatically.

The migration preserves result mapping, credentials separation and ordering semantics. Do not recreate protected state or approve arbitrary incoming results during migration. Existing configuration and state remain operator-owned. For rollback, use a previously reviewed fixed consumer artifact/source revision without changing historical evidence or blindly resetting ambiguous delivery state.

Core never imports or launches this consumer in normal verification. `tools/compatibility/result_boundary` retains Core identity, saved evidence and network-boundary contract regressions using the pinned external consumer. Platform delivery behavior tests moved to GateProbes; they are not silently discarded.
