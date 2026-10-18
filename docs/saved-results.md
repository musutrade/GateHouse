# Verify saved invocation evidence

`harness-gate report-verify --input index.json` checks one saved invocation using
the existing report-history input contract and artifact verifier. It does not
discover a project, require its source directory, execute checks, contact GitHub,
or change the historical quality result.

Use an existing v1 or v2 report-history index with exactly one sample:

```json
{
  "schema": "harness-report-history/v1",
  "samples": [{
    "directory": "saved-invocation",
    "machine_sha256": "<64 lowercase hexadecimal digits>",
    "manifest_sha256": "<64 lowercase hexadecimal digits>"
  }]
}
```

The directory is relative to the index. Copy the entire completed invocation,
including `test_result.json`, `invocation.json`, `artifact-registry.json`,
`manifest.json`, logs and other manifested artifacts. Preserve bytes and relative
paths. The original `project_identity`, `execution_root` and report paths remain
historical metadata; do not rewrite them to the archive location.

Obtain the two pins from a trusted completion record before storage transfer.
Pins calculated from an untrusted download alone establish neither provenance
nor execution trust. The index must be protected by the caller's trust boundary;
this command does not sign evidence, establish the executor's identity or turn
self-reported agent output into trusted execution evidence.

The reader checks the pinned control documents, required evidence, artifact
digests/sizes, unique paths, confinement, invocation bindings and agreement with
the manifested invocation metadata. A missing final result, incomplete evidence,
duplicate artifact, modified log, cross-run binding or changed control bytes
fails verification. Existing resolver limits and rejection of symlink paths
remain in force. All required bytes must be retained before source cleanup.

On success, stdout is JSON containing:

| Field | Meaning |
| --- | --- |
| `historical_integrity` | `verified` against the supplied pins and artifact contract |
| `current_applicability` | `not_evaluated`; no request for a new candidate was evaluated |
| `invocation_id` | The original invocation identity |
| `machine_sha256`, `manifest_sha256` | The independently supplied pins that were verified |
| `result` | The unchanged saved machine result, including its original quality status |

Exit code 0 means this evidence check succeeded. A historically valid **FAIL**
remains FAIL; this command is not another quality engine. A verification error
returns nonzero and does not modify files or promote the result to success.

Historical integrity is separate from applicability to a current candidate,
baseline, policy, toolchain or runtime. Consumers must evaluate those bindings
before reusing a result; equal content alone is insufficient. A failed check
does not automatically rerun verification, repair evidence or revoke remote
Checks. Storage retention and cleanup belong to the caller. Source cleanup
requires stopped execution and safely retained evidence, never PR merge state
or a publication slot.
