# GH-230 container access recheck

> Storage update (2026-10-13): historical raw logs/bundles are available through
> the [verified archive guide](archive.md). Dates, digests and outcomes below are unchanged.

Status: blocked; P6/P7 remain incomplete. All relevant Engineering Policy
semantics remain unchanged. Existing source edits and original evidence are preserved.

This recheck follows the operator Core v0.4.0 recovery handoff. The authenticated
GitHub API confirms predecessor PR #235 merged at 2026-09-11T06:29:31Z as
`e24aeb351168610c00c14032a473fad61659ec2b`, and issue #229 is closed.
Latest `origin/main` is `ee544690662645806cda7dd4cd2c9192566f7929`, already the HEAD
of the preserved workspace-local Git metadata described in
[the continuation record](core-0.4.0-continuation.md). No new integration is needed.
The original read-only `.git` remains unchanged.

The staged official Core SHA-256 matches
`8e3df8303ca8f650d4ef768b29cfefb60ca115122a47b116245cc19e4649bbdd`.
The early wrapper probe, with exact command, timestamp, stdout, stderr and exit
status retained in [the original probe record](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/container-access-recheck.json),
exited 1 before starting a container:

```text
permission denied while trying to connect to the docker API at unix:///var/run/docker.sock
```

The operator must provision sandbox access to the approved container test
infrastructure before clean-host capture, independent producer observation,
retained native re-export and cold/warm/peak-disk acceptance can proceed. Host-side
Docker/ptrace diagnostics do not establish this sandbox access. No restriction
bypass, repeated download, fallback, global toolchain change, release publication,
or Arc-Admin action was attempted. The reviewed compatibility matrix remains empty.

No implementation was changed in this recheck. Prior complete local test commands,
results and genuine failures remain in the continuation record; those results are
not represented as new runs. No task checkbox, acceptance PR or controller handoff
is declared for unfinished work. The existing source changes are preserved for
continuation once access is provisioned.

Fresh documentation consistency and strict change-scoped OpenSpec validation
both exited 0; [exact commands and output](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/container-access-recheck-validation.json)
are retained. The Rust/Python implementation suites were not repeated for this
documentation-only recheck. Project-local `harness-gate config check` and
`harness-gate verify --profile ci --all` remain not applicable because this
checkout has no `.harness-gate/flow.toml` declaring a `ci` profile.
