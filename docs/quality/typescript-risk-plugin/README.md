# TypeScript collector plugin local acceptance

Implementation and independent installation contract:
[`tools/quality/typescript-risk/README.md`](https://github.com/musutrade/GateProbes/blob/155b233fb2f525f520b4fcc0ed73e017154345e0/plugins/typescript-risk/README.md).
This candidate leaves existing Engineering Policy semantics and all existing
certified series/required checks unchanged. Generic Core code is not modified.

The local native suite has 12 tests; the released-Core/transport suite has 3.
It verifies exact boundary 10 (pass), 11 (fail), CC=10 with insufficient line
coverage (fail), raw evidence mutation (rejected), complete function identity,
source/counter/receipt mutation, duplicate JSON keys, source inventory and path
escapes. A tarball installed with production dependencies only passed the same
Core and generic transport tests using its installed executable.

`local-acceptance.json` records package/report digests and the retained local
fixture artifact location. Fixture commit values are test contexts, not a claim
that an application Git commit was measured. The complete source/coverage/receipt,
policy/evidence and Core reports can be retained again in a fresh directory with:

```sh
npm ci --ignore-scripts --prefix tools/quality/typescript-risk
npm test --prefix tools/quality/typescript-risk
HARNESS_GATE_TYPESCRIPT_ACCEPTANCE=/absolute/new-acceptance-directory \
  HARNESS_GATE_TYPESCRIPT_CLI=/absolute/installed/plugin/cli.cjs \
  npm run test:core --prefix tools/quality/typescript-risk
```

Do not infer full Angular certification or production release readiness from
these tests. Original-source instrumentation of a real Angular compilation is
being probed separately by CodexSymphony. Signed host requests, complete source
coverage and an accepted new measurement series remain application prerequisites.

## Independent chain (0.1.0-rc.4)

[`independent-chain.json`](independent-chain.json) records a clean-room run of
the pinned package: `npm pack`, install with production dependencies only, the
installed executable's exact `--version`, and Core 0.4.7 deciding against the
installed artifact. The collector was present only as the installed package; the
record lists the commands, versions and tarball digest.

An incompatible collector protocol request (for example
`harness-collector-request/v2`) now fails closed with an explicit version
diagnostic instead of a bare comparison; see the plugin unit test "incompatible
collector protocol version fails closed with an explicit diagnostic".

