# GH-228 evidence transport

> Storage update (2026-10-13): historical raw logs/bundles are available through
> the [verified archive guide](archive.md). Dates, digests and outcomes below are unchanged.

The native evidence archive is stored as nineteen ordered parts of at most
4 MiB. Concatenation restores the original 79,452,501-byte gzip archive with
SHA-256 `09388d2b355091bc00ea086e5fed97cddc90fb9db2c78711d3871c50ea8ebb06`.
No capture, compression stream, timestamp, manifest or anchor was regenerated.
The original archive remains in the assigned workspace, ignored by Git.
[retained-evidence.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-228/retained-evidence.json) records each part's size/hash,
the original archive hash, and every archived file's original identity.

From the repository root, restore into a fresh workspace-local directory:

```bash
mkdir target/gh-228/restored-evidence
cat docs/quality/gh-228/native-evidence.tar.gz.part-* > target/gh-228/restored-evidence/native-evidence.tar.gz
sha256sum target/gh-228/restored-evidence/native-evidence.tar.gz
```

Compare the printed digest with the one above before extracting. The original
`retain-evidence.py` records how the archive was created; this storage conversion
only splits those existing bytes. Git stores all parts, rather than substituting
hashes or relying on an external download. The roughly 1 GB runtime bundles remain
subject to the separate workspace-retention requirement in [runtime.md](runtime.md).

## Genuine submission failures and recovery

The existing [submission-blocker.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-228/submission-blocker.json) remains an
unaltered historical record. On resumption, the ordinary Git push again returned
exit 128: `Failed to connect to github.com port 443 via 192.168.0.34`.
Uploading the complete archive through the GitHub API failed locally with
`code-mode IPC frame length 105937016 exceeds 67108864 bytes`.
A 40 MiB part request timed out with `%Req.TransportError{reason: :timeout}`.
The connector alternative returned `MCP tool call requires approval, but approval
policy is never`; no approval was available in this unattended session.
The authorized GitHub REST tool successfully accepts 4 MiB parts.

The submitted commit contains the final tree based on accepted predecessor
`b627bbc0e202b6e8a40354b731bd1a4291f31d7a`. The earlier unpublished commits and
unsplit archive remain recoverable in `target/gh-228/submission.bundle`; they
are not required parents of the submitted commit, since their oversized blob
cannot pass through this transport. Original read-only checkout Git metadata
and implementation files are unchanged by this storage conversion.

Validation of this conversion verifies each part, restores the original archive,
and verifies every archived member against its retained identity, including tar
hardlinks. Documentation consistency and strict OpenSpec validation are rerun.
The earlier native, Rust and Python test results still apply to unchanged runtime
implementation files; this conversion makes no measurement or policy changes.

Results: [archive restoration](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-228/transport-validation.json) verified all 1,563
archived members and all nineteen parts. [Final checks](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-228/transport-checks/docs-openspec-diff.json)
record documentation consistency, strict OpenSpec and diff checks exiting 0.
