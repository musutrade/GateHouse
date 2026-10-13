# Repository size policy

Keep product source, current documentation, build/install scripts and required
compatibility inputs in Git. Store historical logs, full reports and temporary
project snapshots in the existing evidence Release archive, with their source
commit, original path and SHA-256 recorded in the archive index. See
[fixture ownership](quality/fixture-ownership.md) before removing test inputs.

`tools/maintenance/repository_size.py` enforces incremental limits in CI's existing
**Classify change** job, before docs-only selection. Failure therefore fails
**Required Quality Aggregate** for both code and documentation changes. Required
job names and dependencies remain unchanged.

This is repository maintenance. Engineering Policy measurement definitions,
coverage/risk thresholds, baseline identities, collector certification and
release authority remain unchanged. A storage check never substitutes for a
quality gate. For local build outputs and retained runs, use the separate
[preview and cleanup guide](local-storage.md).

## What is measured

- A new or changed blob above **512 KiB (524288 bytes)** needs an exact-path
  exception, owner, retention reason and maximum byte size.
- A change above **5 MiB (5242880 bytes)** needs a committed review inventory.
  That inventory does not exempt individual large files.
- Size is the full uncompressed Git blob size, not the textual patch size, Git
  pack size or working-directory disk usage. Each distinct `(path, blob OID)`
  counts once. Copies at different paths count separately. Repeated versions
  at one path count separately, even if a later commit removes the file.
- The guard compares base/head trees and examines commits reachable from head
  but not base. Unchanged historical blobs and deletions add no bytes. Historical
  large files do not require retroactive exceptions. Changed large files and
  rename destinations do require exceptions; a rename cannot inherit the old
  path's allowance. Changes only to executable permissions add no blob bytes.
- Git paths are read with NUL delimiters; tabs, newlines, non-UTF-8 bytes and
  glob-looking names cannot split entries or act as patterns. Reports use JSON
  escaping. Policy files are read from the requested head commit, not local
  uncommitted files or symlink targets.

CI uses the pull request base SHA and checked-out merge SHA, or the push's
`before` and head SHAs. Full history is fetched. An absent, all-zero or
unresolvable base fails with an explicit error: substituting `HEAD^` could miss
other commits in a push. Bootstrapping or replacing `main` requires establishing
an explicit known baseline; this check never silently scans all old history or
turns itself off.

## Local check and single-file exceptions

Commit the intended changes, then run (replace `BASE_COMMIT` with the actual
comparison commit):

```sh
python3 tools/maintenance/repository_size.py \
  --base-sha BASE_COMMIT --head-sha HEAD \
  --output target/quality/repository-size.json
```

The command writes a JSON inventory with byte counts and errors, prints the
same report for CI review, and exits nonzero when policy fails. It does not
change Git history or tracked content. Only committed content is evaluated.

The checked-in `.github/repository-size-exceptions.json` starts with no
exceptions. If a large input must remain in the product repository, add an
entry such as this, using the real exact path and a narrow maximum:

```json
{
  "schema": "repository-size-exceptions/v1",
  "exceptions": [
    {
      "path": "tools/quality/fixtures/example/frozen-input.bin",
      "owner": "core-maintainers",
      "reason": "Frozen input required by the named compatibility replay test",
      "max_bytes": 600000
    }
  ]
}
```

Paths are literal: `*.bin` would cover only a file literally named `*.bin`.
Duplicate paths/JSON fields, missing owners/reasons and invalid bounds fail.
These entries are reviewed in the same PR as the input. Keep them small and
remove entries when their inputs leave the repository. Neither policy metadata
file can exempt itself, and each policy file is capped at 512 KiB.

## Reviewing a bulk change

For a change above 5 MiB, the report includes `review_template`. Copy that
object into `.github/repository-size-review.json`, fill in its `owner` and
`reason`, commit it, and rerun the check. The template includes the exact base
commit and every required path, full Git blob OID and byte size; no manual blob
hash calculation is needed. Resolve individual-file errors separately.

The inventory must cover every changed blob. Editing payload files, exceptions
or any other content after recording the inventory may require refreshing it.
The review file itself is excluded from its own inventory to avoid a recursive
OID dependency, but every version still contributes bytes to the 5 MiB total
and remains subject to the 512 KiB per-file cap.

The inventory may retain exact entries for intermediate PR versions omitted by
a squash merge. This permits the same review to pass on the PR and its squash
commit without allowing missing entries or different bytes for a required
blob. A changed base requires a new review. A retained review from a previous
change grants no general permission: small changes do not use it, and a later
large change must match its own base and blob inventory.

## Validation

The focused contract suite uses temporary Git repositories to exercise old
large files, modifications, renames, intermediate blobs, merge histories,
squashes, literal unusual paths, exact thresholds and fail-closed metadata:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover \
  -s tools/quality/tests -p test_repository_size.py -v
```
