# Archived historical evidence

[The archive index](archived-evidence-index.json) locates original files by their
repository-relative path, SHA-256, size and Git blob. Its top-level v1 record
continues to describe the unchanged 178-file archive published on 2026-10-10.
`supplemental_archives` contains additional archives with the same fields and an
explicit source commit. A file belongs to exactly one archive record.

The 2026-10-13 housekeeping batch externalizes 41 historical validation logs and
bundles from GH-228, GH-230 and GH-231. The source snapshot is
`17e22cef29fd3692aa1211160d7df6e599ef8c66`. It also removes 153 individual text
placeholders for already archived GH-228/GH-230 parts. Those placeholders were
instructions, not the original binary parts. Issue pages now link to one archive
guide each; the original inventories, summaries and failure records remain.

| Historical record | Storage and retained inventories |
| --- | --- |
| GH-228 native delivery work | [Archive guide](gh-228/archive.md) |
| GH-230 runtime integration work | [Archive guide](gh-230/archive.md) |
| GH-231 installer preparation | [Archive guide](gh-231/archive.md) |

## Restore exact bytes

Find the original path in the index, download that record's `archive_url`, and
keep the archive outside the source checkout. Use the repository's
[offline restore tool](../../tools/maintenance/restore_evidence.py) with the
downloaded archive and a destination that does not exist. Python 3.11 or newer
and `zstd` are required for the published `.tar.zst` assets. For example:

```bash
python3 tools/maintenance/restore_evidence.py \
  --archive /tmp/quality-evidence-housekeeping-20261013.tar.zst \
  --destination /tmp/gh231-restored \
  --member docs/quality/gh-231/evidence/nextest.log
```

Repeat `--member` to select more files from the same archive. The tool verifies
the whole archive digest and every indexed member's size and digest, rejects
unindexed/duplicate members, unsafe paths and links, and writes only selected
files beneath the new destination. It supports both the original top-level
record and supplemental records. It does not download files or overwrite a
checkout. A verification failure leaves no restored destination.

Historical JSON inventories still name the original repository-relative paths;
for externalized files those paths are archive member names. Restore the bytes
before using an old inventory or concatenating archived parts. Preserve part
order and verify the original assembled archive digest recorded in the issue's
manifest. The outer archive verification does not certify the safety of any
nested archive or historical command.

## Scope and consumers

This is storage housekeeping. Engineering Policy measurement, requiredness,
coverage/risk thresholds, baseline identities, failure outcomes and release
authority are unchanged. No historical result was regenerated or reinterpreted.
Git history and existing immutable releases are untouched.

The selected logs/bundles have historical documentation and inventory references,
but no runtime/test input consumer. Their references now resolve through the
archive guides. GH-228's retention helper writes new validation bundles; it does
not consume the archived historical output. The existing archive's 178 records
retain their original identities and the supplement records each new member.

Inputs with active consumers stay checked in: GH-96/GH-97 candidate replay
bundles, generic-core fixtures, ownership/digest inventories and Arc-Admin
self-hosted/shadow/remediation receipts. In particular, `ci_reproduce.py` checks
the complete self-hosted manifest dynamically; those files are not disposable
logs. The restore tool has focused negative tests; the existing replay and
documentation checks remain in place.
