# Local storage and retention

Source checkouts, reusable build caches, disposable run outputs, and historical
backups have different lifetimes. Keep caches in designated build directories;
keep run evidence and migration backups separately, outside the cache directory.
Coverage and base/head measurements must continue to use their own isolated build
and cache identities. A shared development cache must not replace those identities.

`tools/maintenance/local_storage.py` is an explicit manifest-driven cleanup tool
for Linux and Python 3.11 or later. It does not discover deletion candidates, run
in the background, elevate privileges, or delete an entire `target/` by default.
Its default action is to preview. It requires a Git checkout with an existing
commit, and the root must be the exact canonical repository root.

## Review and register candidates

Inspect each candidate first. A directory named `target` or a `CACHEDIR.TAG` marker
is useful inventory evidence, but does not establish that all its contents can
be discarded. Register only specific, reviewed directories whose contents can be
regenerated and contain neither evidence nor backups. Leave unknown directories,
active outputs, pending reviews, unresolved failures, and unarchived records out
of the deletion inventory, or register them with a protected category/reason.

Store the manifest, plan, and execution receipt outside every candidate directory:

```json
{
  "version": 1,
  "entries": [
    {
      "path": "tools/harness-gate/target/debug",
      "kind": "cache",
      "reviewed": true,
      "reproducible": true,
      "contains_evidence": false,
      "contains_backup": false,
      "reason": "Reviewed Cargo development build output; regenerate with the documented build command"
    },
    {
      "path": "target/migration-backup",
      "kind": "backup",
      "reason": "Keep until the archive has been downloaded and its digest and recovery checked"
    }
  ]
}
```

Only `cache` and `run` categories are eligible. Unknown categories and missing
review declarations are retained with a reason. `protected: true` always retains
an entry. Overlapping paths, duplicate paths, traversal, Git metadata, symlinks,
special files, nested repositories, cross-device mount points, and another user's
files are rejected. Every candidate must be a directory. Missing candidates are
an error; update the manifest after an earlier cleanup rather than reusing it.

For `kind: "run"`, also require `state: "completed"`, `failure_closed: true`,
`archived: true`, `review_pending: false`, a nonempty `group`, and `completed_at`
(the completion Unix timestamp, in seconds). The newest two eligible runs in each
group are retained. Protected/incomplete/unarchived runs are retained separately
and do not displace those two. These declarations describe the reviewed state;
the tool does not prove archival completion or infer it from a directory name.
Only register the regenerable part of a run; evidence remains outside candidates.

## Preview, then execute during a maintenance window

Stop builds and other work using the registered directories. Keep them stopped
until cleanup completes. Use a new plan filename for each attempt:

```sh
python3 tools/maintenance/local_storage.py plan \
  --root /absolute/repository \
  --manifest /absolute/operations/manifest.json \
  --output /absolute/operations/plan.json
python3 tools/maintenance/local_storage.py apply \
  --plan /absolute/operations/plan.json
python3 tools/maintenance/local_storage.py apply \
  --plan /absolute/operations/plan.json --execute \
  > /absolute/operations/receipt.json
```

The second command revalidates and previews without deleting. The third deletes
only the plan's eligible entries after revalidation. Plans expire after one hour;
`plan --ttl SECONDS` may select up to 24 hours. Changing HEAD, the staged file
inventory, candidate inode/size/mtime/ctime metadata, root identity, or retention
selection requires a new plan. Both HEAD and index file names protect tracked
content, including files staged for deletion. Deletion uses directory descriptors
and Python's symlink-resistant `rmtree` implementation.

Allocated bytes are an upper bound, not promised recovered space: hard links,
open handles, filesystem behavior, and concurrent system activity affect the
result. Record filesystem free bytes before and after execution to measure
actual recovery. This is sequential cleanup, not a transaction. If later
validation fails, previously deleted entries stay deleted; inspect the directories
and make a fresh manifest/plan for any remaining candidates.

## Activity checks and their limits

On Linux, preview and execution inspect `/proc` for working directories,
executables, process roots, open file descriptors, mapped files, and kernel file
locks. Inode checks detect hard-linked open files and mappings. The scanner skips
only its own exact PID and zombies with no live address space; parents and child
processes remain in scope. It rechecks immediately before each deletion. Other
operating systems refuse cleanup when there are deletion candidates.

Unreadable processes belonging to the repository owner block cleanup by default.
For an ordinary user, readable processes from other accounts are also checked;
those that cannot be inspected are recorded under `activity.other_uid_unobservable`.
This is a documented visibility boundary, not a claim that those processes hold no
files. When invoked as root, unreadable processes from every account block by
default. The tool does not request or configure elevated access.

A reviewed, managed process whose file state cannot be inspected can be registered
as an exact instance exception in the manifest's `process_exemptions` array. Every
entry must contain all of `pid`, `starttime` (Linux process start ticks), `uid`,
`comm`, `cmdline` (an argument array), `cgroup` (the complete `/proc/PID/cgroup` text
with its final newline removed), `ppid`, and a nonempty `reason`. The tool compares
all seven identity fields at each scan and only waives unavailable observations
for that matching instance. It still checks any readable references and all
children. A process-name-only exception, PID reuse, or changed identity is not
accepted. Used exceptions appear in the receipt. An exception is a reviewed
operational assumption, not complete file-descriptor coverage.

Neither process inspection nor metadata snapshots lock arbitrary builders or
prevent a new process from starting after inspection. The maintenance window is
required. This tool protects against routine mistakes and detectable activity;
it is not isolation against an administrator or malicious concurrent mutation.
There is no scheduled deletion until this manual workflow has been established.

Run its focused tests with:

```sh
python3 -B -m unittest discover -s tools/quality/tests -p test_local_storage.py -v
```
