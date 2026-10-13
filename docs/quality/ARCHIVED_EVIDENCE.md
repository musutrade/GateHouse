# Archived quality evidence

Large quality evidence files (more than 512 KiB) are no longer stored in Git
history. Their original bytes are retained in a GitHub Release asset:

- Archive: https://github.com/musutrade/Harness-Gate/releases/download/evidence-archive-20261010/quality-evidence-current.tar.zst
- Archive SHA-256: `059db8a81c0559dea83c925a69f06dfc4ace82ca3b5bf07eef448537eb951565`
- File index: [archived-evidence-index.json](archived-evidence-index.json)

The index records the original size, SHA-256 and Git blob for each externalized
path. Download and verify the archive before using an archived file:

```bash
curl -L -o quality-evidence-current.tar.zst \
  https://github.com/musutrade/Harness-Gate/releases/download/evidence-archive-20261010/quality-evidence-current.tar.zst
sha256sum quality-evidence-current.tar.zst
tar --zstd -xf quality-evidence-current.tar.zst docs/quality/<path>
```
