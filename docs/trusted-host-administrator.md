# Candidate trusted-host administrator interface

The fixed-659 migration has an authenticated preparation and dispatch entry:
`import-protected-inputs.py administrator-upgrade`. Its completed result concerns
the migration component only. It keeps `administrator_ready` and
`production_gate_enabled` false. Live isolation, transition workload, restart,
resource, cache and final coverage acceptance remain separate requirements for
issue98. The accepted659 installation is unchanged by exporting this candidate.

## Reviewed inputs

The operator package must contain the exact committed helper closure and source
release, with their independently reviewed SHA-256 values. The release contains
source and the original fixed659 source inputs, including its Git bundle; it
contains no ordinary-built Core binary. Official tool archives, locked crate
archives, test-data archives, onboarding data and host/tool recipes are external
inputs. The input import verifies their existing official/locked inventories;
the Core release is built under the existing995 worker before installation.

The entry requires these inputs already staged under Root-protected paths.
Preparing an ordinary source export is not ownership approval or permission to
copy arbitrary files into a protected stage. The complete operator staging
package and its live acceptance are still pending.

The sealed administrator recipe is canonical compact sorted JSON with one final
newline, mode0400, no links, Root ownership and protected ancestors. Its SHA is
an explicit command argument. Its exact fields are:

```json
{
  "schema": "gatehouse-administrator-upgrade/v1",
  "head": "<exact reviewed 40-hex revision>",
  "helper_manifest_sha256": "<reviewed helper manifest SHA-256>",
  "paths": {
    "release": "/root/reviewed-stage/bootstrap-source.tar",
    "host_recipe": "/root/reviewed-stage/host-recipe.json",
    "tool_recipe": "/root/reviewed-stage/volume-tools.json",
    "tool_archives": "/root/reviewed-stage/tool-archives",
    "crate_archives": "/root/reviewed-stage/crate-archives",
    "test_archives": "/root/reviewed-stage/test-archives",
    "onboarding": "/root/reviewed-stage/onboarding"
  },
  "pins": {
    "release": "<source archive SHA-256>",
    "host_recipe": "<host recipe SHA-256>",
    "tool_recipe": "<volume tools recipe SHA-256>"
  },
  "api_proxy": null
}
```

This expanded example illustrates the fields, not the canonical bytes for an
actual host. No output map, Core binary path or caller-supplied verdict is
accepted. Input paths must remain separate from mutable pilot/staging storage.
The API proxy may be null or the explicitly reviewed canonical credential-free
HTTP(S) host and port, such as the operator-approved `http://192.168.0.2:7890`.
It affects preparation preflight and the sealed revocation/readback route;
runtime controller/Git transport needs separate live verification. It never
disables TLS verification or enables environment proxy inheritance in the
accepted client.

## Entry and ownership

Once a protected stage and its recipe have been reviewed, the interface is:

```sh
/usr/bin/python3.14 -I /root/reviewed-stage/helpers/tools/trusted_gate/deploy/import-protected-inputs.py administrator-upgrade \
  --helper-root /root/reviewed-stage/helpers \
  --manifest /root/reviewed-stage/manifest.json \
  --manifest-sha256 '<reviewed helper manifest SHA-256>' \
  --recipe /root/reviewed-stage/administrator.json \
  --recipe-sha256 '<reviewed canonical recipe SHA-256>'
```

The entry is administrator-only and adds no sudo delegation. Authentication
precedes deployment imports. The original
`/srv/gatehouse/pilot/control/controller.lock` is opened and held continuously;
actors run in-process with that descriptor, rather than through a shell sequence
that releases/reacquires the lease. An occupied or replaced lease blocks entry.

## Durable progress

`/srv/gatehouse/pilot/control/administrator-upgrade.json` records an ordered
preparation prefix, the one known inflight stage and immutable transaction pins.
Each stage persists intent before its effect and verifies its completed artifact
before recording its digest. Destinations come from the authenticated source:

| Stage | Complete artifact |
| --- | --- |
| Bounded volume | Existing volume helper's ready registration |
| Preserve659 | `/var/lib/gatehouse/retained-linux-<first12head>/preservation.json` |
| Source stage | `/srv/gatehouse/linux/bootstrap-source/stage.json` |
| Official/locked inputs | `/srv/gatehouse/linux/bootstrap-inputs/binding.json` |
| Linux roles | `/srv/gatehouse/linux/bootstrap-linux-inputs/role-binding.json` |
| Protected Core | `/srv/gatehouse/linux/bootstrap-core/build.json` |
| Configuration | `/srv/gatehouse/linux/bootstrap-host-config/binding.json` |
| Passive inspection | `/srv/gatehouse/linux/bootstrap-core-inspection/binding.json` |
| Installation recipe | `/srv/gatehouse/linux/administrator-installation-inputs.json` |

The retained659 snapshot stays outside the new16GiB volume. All original raw
reports, including failures, remain evidence. The saved pending database is
never restored over live WAL state. The generated installation recipe is mode0400
and binds the preparation artifacts and explicit API route to the same revision
and helper manifest.

## Recovery boundaries

Fresh entry rejects preexisting deployment state or preparation outputs. For a
known inflight stage, restart verifies only its completed artifact; it does not
repeat a build after an ambiguous response. A blocked/partial build, unknown
output, changed digest, incomplete metadata or missing completion cannot be
adopted. Those files and logs remain private for diagnosis; reconciliation is a
separate reviewed action, with no automatic deletion, rollback or new build.

After preparation, the fixed lifecycle fences entries, withdraws the old result,
verifies remote/copy effects, retires old source and installs all27 files while
disabled. The activation transaction then advances the reviewed barrier while both entries
remain disabled, verifies the actual installed Snapshot and configuration, and
enables internal entry before public entry with known intents. Once activation has begun, dispatch resumes it directly
instead of invoking the historical disabled-file observer again.

Formal `host.run` remains blocked until its Root-protected activation completion
record matches. An executable public file during `public-intent` does not permit
CLI imports or effects. Completed administrator retry rechecks recorded artifacts
and activation prerequisites without rerunning preparation. Replay remains a
quiet migration operation; it is not a concurrent workload or status API.

No source-recipe verdict or successful preparation substitutes for actual Core
verification. Publishing authoritative success remains blocked in transition
coverage, and native required checks remain in force.
