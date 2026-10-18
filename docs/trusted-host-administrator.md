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
The candidate transport exporter and stager described below prepare that boundary;
an ordinary source export is not ownership approval. Actual protected staging,
migration and restart acceptance remain pending.

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

## Candidate input transport

`package-administrator-inputs.py` runs as the ordinary operator from a clean
committed checkout. It takes reviewed host/tool recipes, official tool archives,
locked crate archives, test archives and onboarding source. The host recipes are
checked against the actual protected OS files before export. Tools, crates and
test archives are selected by the committed locks and manifests and streamed
against their expected digests; onboarding is checked against its existing fixed
source digest. No reconstructed compiler tree or ordinary Core is transported.

The exporter produces `inputs/`, a source export, `package.json` and
`stage-reviewed-inputs.sh`. Only `inputs/` is the closed transport tree. Its
canonical `transport.json` pins every file's size, SHA-256 and intended private
mode. It includes the exact helper closure and its manifest, source-only release,
host/tool recipes, external data and a generated administrator recipe. That
recipe uses the fixed destination `/root/gatehouse-issue98-linux-<first12head>`;
caller-supplied deployment outputs and verdicts are not accepted. The explicit
API proxy remains part of the reviewed recipe.

Review the exported head, helper/release/transport/recipe/program/launcher pins
and the launcher text before administrator use. The launcher authenticates through
sudo, then copies at most1MiB of the stager into its fixed private Root loader
directory. It checks the literal reviewed program SHA before executing that Root
copy with isolated Python and a minimal environment. It never executes project
Python from a mutable operator path. An existing loader requires the same private
file and digest; partial or unknown files block and remain available for diagnosis.

The stager opens source ancestors through directory descriptors without following
links. Each file is streamed from a stable regular single-link descriptor and
checked against the externally pinned transport manifest. It rejects special or
privileged files, unknown/empty directories, file/directory collisions, missing
members, byte/count/metadata budget excess and a changed source. The transport
budgets are1GiB total,256MiB per file,4096 files plus directories and2MiB manifest.

Copies first go into a fresh sibling `.tmp` directory, with directories0700,
data/code0400 and only reviewed onboarding executables0500. Files and directories
are synced. The complete protected tree and the helper/administrator bindings are
verified again before the fixed final rename and parent sync. A completion record
binds the exact transport. A completed retry verifies the entire private tree
without copying again. An interrupted copy is retained and blocks retry; no
partial stage is adopted or deleted automatically.

This launcher only stages inputs. It does not invoke `administrator-upgrade`,
create the16GiB volume, start services, change entries, issue Checks or run quality
workloads. The subsequent reviewed migration still needs actual Root/995
validation and live transport, mount/restart and resource acceptance. Neither
export nor staging changes the false administrator/production flags.

## Candidate persistent Linux mount

The source-only installation plan now has28 files. It retains the accepted
`srv-gatehouse-pilot.mount` bytes and adds `srv-gatehouse-linux.mount` for the
separate `/var/lib/gatehouse/linux.ext4` image at `/srv/gatehouse/linux`. The new
unit uses ext4 with `loop,nodev,nosuid,noatime`, a60-second mount timeout and
`WantedBy=multi-user.target`. Its bytes are part of the reviewed source payload,
configuration pins and complete file transaction; an existing unregistered unit
blocks initial installation.

After disabled installation, activation calls `persist-linux-mount.py` under the
same original lease and full live activation admission. This internal helper
binds a private `mount-persistence.json` to the activation generation, helper,
recipe, material, original lease, installation receipts, exact unit bytes and
ready storage registration. It records reload intent before `daemon-reload`,
then enable intent before `systemctl --no-reload enable srv-gatehouse-linux.mount`.
The already mounted image must pass the existing full storage admission.

Only the Root-owned absolute link from
`/etc/systemd/system/multi-user.target.wants/srv-gatehouse-linux.mount` to the
reviewed unit is accepted. Unit overrides, exact/type/prefix drop-ins, other
enablement slots, foreign links, unknown manager properties and unfinished
metadata block. The manager must show the exact loaded active/mounted fragment,
no drop-ins or pending reload, and the expected disabled/enabled state. The link
parents are synced before completion. A lost enable response can adopt only this
known link under its prior durable enable intent. Completed retry is read-only;
no prior intent means an existing link is not adopted.

Activation rechecks completed persistence before and after each full live
observation, including after the active barrier and before entry modes change.
No persistence transaction may begin after activation has advanced. The helper
does not use `--now`, start/stop units, format, remount or unmount an image, touch
the old pilot mount, or restore saved database state. Partial state stays private
for diagnosis. This changes candidate source only; installed659 remains untouched.

Native unit syntax and an ordinary `systemctl --root` temporary-directory enable
test establish parser/link behavior only. Actual Root manager adoption, enabled
boot mounting, resources and post-boot controller validation still require live
acceptance. Durable installation receipts keep their original device/inode
identities: a reboot is not permission to rebind them or weaken recovery checks.
Both administrator and production acceptance flags remain false.
