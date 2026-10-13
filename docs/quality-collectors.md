# Trusted project collectors

For the published Rust native collector, use the
[installation guide](quality/rust-collector-installation.md) and
[exact compatibility record](release-status.md#compatibility). The interfaces
below remain generic and apply to configured collectors of any ecosystem.

GH-181 implements OpenSpec tasks 3.1–3.4 over the existing signed, out-of-process
adapter v2 host. `quality collect` is an advanced project orchestration interface:

```sh
harness-gate quality collect --repository-root . --state trusted-state.json --trusted-keys trusted-keys.json --output collection.json
```

The state has the [trusted compilation contract](quality-compilation.md).
Its artifact inventory and existing artifact root must be empty. The output
belongs outside that root. Trusted keys are a host-owned
JSON array of `{ "key_id": "…", "public_key": "base64 Ed25519 key" }` objects.
The CLI uses the adapter host's default capability, timeout, request age, output
and artifact budgets. Both `verify` and `quality collect` persist consumed nonces
in `<canonical repository root>/.harness-gate/collector-nonces` by default.
See [durable replay protection](#durable-replay-protection) for external host storage.
The internal orchestration entry point accepts a host-owned `HostPolicy` for
explicit limits. Collectors cannot enlarge those limits or grant themselves trust.

## Binding and execution

The validated quality profile selects collector bindings. Each binding's request
path points to a complete signed adapter request. Resolved pack state supplies
collector/package identity and canonical series metadata; configuration supplies
subject/component/relationship targets and expected capabilities. The generic
orchestrator iterates those bindings without inspecting ecosystem metadata.
All selected producers fail closed on execution or validation errors.

The quality configuration's `harness-collector-request/v1` binding selects the
project collector contract below. The signed adapter request retains protocol
version `2` and result schema `1`.
Its `step_id` is the configured binding ID, `invocation_id` is the trusted run,
package name/version match the trusted series collector, and `artifact_root`
matches the compiled canonical root. The host verifies the signature, executable
digest, freshness, nonce, requested capabilities and enforceable resource limits
before launch. Request and response JSON reject duplicate keys at every depth.

The signed `input` is exactly:

```json
{
  "schema": "harness-project-collector-request/v1",
  "project": "trusted project ID",
  "collector": {"name": "configured package", "version": "pinned version"},
  "context": {"commit": "…", "base_commit": null, "target": "…", "run": "…"},
  "workspace_root": "/canonical/source/root",
  "output_root": "/canonical/artifact/root",
  "selection": null,
  "bindings": [{"subject": "canonical subject ID", "capability": "bundle.size", "series": "canonical series ID"}]
}
```

`selection` is the compiled selection contract or null. Bindings are sorted by
subject, capability and series; they exactly resolve the configured targets.
Overlapping authoritative producer claims are rejected before launch.
The outer adapter `artifact_root` must equal the compiled canonical artifact
root as well; sign this resolved path when the workspace uses a symlink alias
(including macOS temporary directories).

`config_digest` is SHA-256 of canonical generic JSON containing `schema` equal to
`quality-collector-binding/v1`, `config_files`, compiled `project`, `policy`,
`expected`, `selection`, `mappings`, `exceptions`, and trusted `profile`, `series`.
Here `config_files` excludes configured request paths to avoid a self-referential
signature. The trusted state separately pins the complete signed request bytes.
The existing adapter signing domain covers the complete input, arguments, roots,
limits and executable identity. A trusted caller prepares and signs these
requests; collectors never generate their own trusted state.

## Measurement response and output

The adapter response contains exactly `schema_version: "1"`, `status: "PASS"`,
the matching `invocation_id`, `artifacts`, and `collection`. Here PASS means the
measurement transport completed. `collection` contains `schema` equal to
`harness-project-collector-response/v1`, an `evidence` array of existing
`harness-evidence/v1` records, and optional `error` (absent or null on success).
This project envelope is distinct from the frozen Python collector v1 protocol.

Each configured subject/capability/series must be present exactly once. The
Rust generic core validates normalized values, capability honesty, canonical
subject/series identity, collector/tool/runtime metadata, source and context,
artifact hashes and links. The outer artifact inventory must exactly match the
records' artifact descriptors and every file on disk must be declared. Symlinks,
missing files, extra files and mixed provenance fail closed. Source/config pins
are rechecked after execution. No partial collection output is published on
failure; an earlier output is removed after protecting input aliases.

Successful output has schema `quality-collection/v1`, compiled `inputs` bound to
the collected artifact inventory, `evidence`, producer origins and reusable response envelopes. It is an inspectable measurement
bundle, not an authenticated cache or delivery decision. The existing Rust policy
evaluator owns requiredness, thresholds and approval. Unavailable capability
states remain distinct; no numeric values or CRAP series are synthesized.

The signed executable fixture in `tools/harness-gate/fixtures/workflow/collectors/`
launches an arbitrary ecosystem with a custom binding, package, tool and series
through the same code as the reference fixture. The configured `bundle.size`
contract succeeds; an arbitrary `quasar.payload_bytes` capability is resolved and
launched, then fails closed at the released core's unsupported-metric validation.
This preserves the GH-180 boundary without certifying a new metric or ecosystem. Runtime tests
and a source guard reject introducing closed language dispatch into this path.

`adapter run` remains available for advanced/debug use.
[Verification composition](quality-verification.md) consumes host-prepared signed
requests and combines the resulting quality decision with execution gates.
Automatic request construction remains a host/pack responsibility.

[Retained head evidence](quality-profiles.md#retained-head-collection) permits host-authenticated reuse before fresh producer launches; invalid retention fails closed.


## Durable replay protection

Use the global `--replay-state-dir PATH` option to select trusted host storage.
It applies to `verify`, `quality collect`, `hook`, compatibility verification and
`adapter run`; repository configuration and signed collector payloads cannot
select or override it. Relative overrides resolve against the host's current
working directory. Provision its parent first; Core creates only the ledger
leaf directory. Use an absolute path without symlink components or `..`.

```sh
harness-gate --replay-state-dir /srv/gate-host/project-a/nonces \
  --project-root /srv/snapshots/project-a verify --profile ci --all
harness-gate --replay-state-dir /srv/gate-host/project-a/nonces \
  quality collect --repository-root /srv/snapshots/project-a \
  --state /srv/gate-host/project-a/state.json \
  --trusted-keys /srv/gate-host/project-a/keys.json \
  --output /srv/gate-host/project-a/collection.json
```

The ledger path defines the scope: every signer, profile, invocation and command
using that directory shares one nonce namespace. Mint globally unique nonces
within that scope. Keep the same path across process restarts, source snapshots,
workspace relocation and artifact cleanup. Default verify storage uses the
original repository root, including staged verification, rather than the
throwaway execution snapshot. Advanced `adapter run` retains its request-adjacent
`.harness-gate-adapter-replay` default; pass the same override to share the
collector scope explicitly. Library embedders must set `HostPolicy.replay_state_dir`
for protection across host restarts; its default remains in memory.

Core authenticates the request and checks freshness, executable identity and
collector source/configuration bindings before claiming its nonce. Claim uses
exclusive file creation, flushes the record before starting the collector and
retains the marker after timeout, crash or measurement failure. Concurrent hosts
can authorize at most one execution. Empty, malformed, symlinked or partial
markers for the same nonce all remain consumed. Markers are never automatically
removed, even after request expiry; artifact retention does not control them.
In-memory markers remain through the inclusive `expires_at_ms + clock_skew`
boundary, with saturating arithmetic. Request age and signature checks remain
unchanged.

The ledger is trusted host control-plane state. Keep it outside writable test or
collector mounts and outside the artifact tree. Use OS sandboxing or a separate
identity to prevent untrusted children from writing the ledger or any ancestor;
protocol capability checks do not enforce that isolation. Unix ledger directories
must belong to the host UID and forbid group/other writes; ancestors must belong
to that UID or the filesystem root owner and forbid shared writes (root-owned sticky temporary roots
are permitted). A umask of `0002` (the default for many Linux user accounts)
creates group-writable project directories, so the default
`<request-parent>/.harness-gate-adapter-replay` ledger is rejected under such a
project; the error names the directory and its mode. Run `chmod go-w` on the
reported directory or select a private host-owned `--replay-state-dir`.
Windows storage requires equivalent host-managed ACLs. Core
rejects symlink/reparse components and unwritable/non-directory storage, pins the
directory before verify's execution steps and rejects replacement before launch.
Unix claims use descriptor-relative no-follow operations; Windows directory
handles deny rename/delete sharing and marker writes use write-through flushing.

No filesystem ledger can detect host-authorized deletion or rollback of all its
state across a restart. Restrict deletion, backup restoration and cleanup to the
trusted host. Do not reset or prune an active scope: archived signatures could
become replayable, especially after clock rollback or increased skew. Retire a
ledger only together with its signer/request scope and preserve it as evidence.
Read-only source snapshots need the external override and separately provisioned
writable artifact/report destinations. Missing or unsafe state fails closed;
Core never silently falls back to memory for a configured durable ledger.

The public adapter request preparation path enforces the same directory boundary
as `verify` and `quality collect`: the ledger and artifact root must be different
directories, and neither may contain the other. Overlap is rejected before nonce
claim or child launch, including artifact paths that resolve through an alias.
Ledger names are canonicalized only after the no-follow directory walk and are
revalidated against the pinned directory. This gives ordinary Windows paths and
their `\\?\` representations one ledger scope without accepting reparse points.

GH-269 strengthens request integrity only. Generic Core remains language-neutral;
measurement identities, formulas, required gates, thresholds, baselines, debt,
ratchets and release authority in the [Engineering Policy](engineering-policy.md)
are unchanged.
