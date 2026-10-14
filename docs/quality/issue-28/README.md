# GateHouse #28: plugin installation ownership

GateHouse's active installer installs Core only. Legacy plugin flags fail
before downloads or replacements with migration instructions; compatibility
execution is retired immediately. GateProbes owns the sole active implementation,
exact native version/tag/asset selection, signer declaration and installation,
upgrade and rollback tests. Historical tagged installers remain frozen.

The [migration guide](../standalone-installation.md) binds the public plugin
installer to full source commit 1507805205944229950e0fe57d4fe1f17fc3e130 and script
SHA256 317e7f98e3863d44d32640106091596e227be0795463dd81adcd26ef6797bce6.
The actual public download was verified and installed into an isolated directory:
two real Cosign checks passed, and the binary digest is
da9b52b7097d5b824db44af80b82a7a9f03f0e9ed8c90801cf8ba2e65ab69744.
See the [GateProbes receipt](https://github.com/musutrade/GateProbes/blob/aaac3078675962133a2da19b19d3391e4e77a53e/docs/native-installer-acceptance.json).

RC8 remains legacy mutable. The plugin installer pins the original signed
manifest in addition to exact-tag signature checks. This change does not
replace or resign any published release, tag or asset. Future native publication
retains the existing protected and immutable release requirements.

| Validation | Result |
| --- | --- |
| Core installer shell contracts | Passed: integrity, atomic failure, signals, unsafe paths, Core-only download and legacy-flag diagnostic before downloads |
| Core release Python suite | 46 tests passed; frozen historical replay also passed 64 methods |
| Core repository/signer cutover | All four tests passed, including the complete installer fixture replay with GateHouse signer |
| Plugin-owned installer shell contracts | Passed: install, two-version signed-fixture upgrade/rollback, four-platform dispatch, wrong identity/tag/workflow, missing verifier/signature, pollution, copy/chmod/rename failure and signal cleanup |
| Public full-commit script checksum and real RC8 install | Passed on Linux x86_64; two real Cosign verifications and exact public binary digest |
| Core documentation consistency (pure) | Passed; Engineering Policy and links passed |
| GateProbes docs/declarations | Passed |

The offline rollback test does not claim GateProbes RC4 publication; currently
RC8 is the only public native release there. Older Harness-Gate versions use
their historical installer and original identity, with no cross-signer fallback.
No fresh four-platform measurement/runtime certification is claimed.
Installation changes no configuration, protocol, measurement series or baseline.
Core retains requiredness, thresholds, baseline/debt/ratchet, final decision and
release authority; missing plugin tools are diagnosed rather than provisioned.

Rollback of this ownership change is a coordinated source revert in both repos.
Historical user restoration uses the original tagged installer; reverting source
does not permit rewriting historical assets/signers or resetting evidence.
