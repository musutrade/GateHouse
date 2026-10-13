# rc.2 original Angular and signed Core acceptance

Independent package 0.1.0-rc.2 adds the published project adapter-v2 envelope,
file-level original line coverage, declaration-only discovery, and capture
pipeline/input pinning. Core code and legacy presets remain unchanged.

The installed package passed 15 unit and four Core/protocol tests. CodexSymphony's
actual Angular original-source instrumentation passed six tests and released
Core 0.4.5 accepted the host-signed collection and required coverage >= 80% /
CRAP <= 10 policy. Five negative cases reject stale context, expired signature,
signature tampering, replay, and artifact mutation.

`angular-acceptance.json` identifies the package, measurement series and archive.
`angular-evidence.tar.gz` retains source/config/tests, native counters, compiled
host state, signed requests, public keys and Core reports. No private key is
included. The capture producer and independent local host rehearsal source are
in CodexSymphony (`web/angular/tools/probe-typescript-risk.cjs` and
`tools/frontend_host_acceptance.py`). The project script uses an original-source
copy, preserves inputs, and signs only after executing the instrumented tests.

This is local original-TypeScript frontend acceptance. It does not grant whole
project, template, production keystore/isolation, CI protection or release
authority. Signed requests are short-lived and cannot be replayed as baseline
evidence. rc.1 receipts remain historical and do not authorize rc.2 identity.
