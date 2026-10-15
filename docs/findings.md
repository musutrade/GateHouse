# Findings, SARIF and PR feedback (Issue #33)

This additive source interface is **not yet part of a published Core release**.
`harness-gate findings` consumes completed reports without running analyzers.
Engineering Policy semantics are unchanged: Core owns requiredness, thresholds,
debt/baseline/ratchet, the project decision and release authority. Finding severity,
history and GitHub annotations are diagnostic data. A successful conversion or
feedback delivery is never a project PASS.

## Versioned inputs and normalization

The bundle and output contracts are `harness-findings/1`; native source documents
use `native/1`, and SARIF inputs/outputs use SARIF 2.1.0. Generated structural schemas:
[bundle](../schema/findings-bundle.schema.json),
[normalized report](../schema/findings.schema.json).
Runtime checks additionally validate digests, provenance, paths and history.
Unknown bundle/model fields, duplicate JSON keys, malformed documents, unsupported
versions/formats and incomplete sources fail explicitly with a nonzero CLI exit.
Unknown tool-specific fields remain in the recursively redacted source document.

A bundle looks like this (replace the example SHA values with actual identities):

```json
{
  "schema_version": "harness-findings/1",
  "context": {
    "repository": "owner/repository",
    "commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "tree": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    "series": "project-static-analysis/full/1"
  },
  "report_sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
  "sources": [{
    "format": "native/1",
    "measurement_id": "tool-run-123",
    "complete": true,
    "document": {
      "schema_version": "native/1",
      "tool": "example-checker",
      "tool_version": "1.0.0",
      "findings": [{
        "rule": "unused-resource",
        "anchor": "logical-symbol-and-occurrence-42",
        "severity": "warning",
        "message": "Resource requires review",
        "locations": [{"path": "src/main.rs", "line": 12, "end_line": 12}],
        "fixes": null
      }]
    }
  }]
}
```

Use `format: "sarif/2.1.0"` with the complete SARIF document in `document` for
any SARIF-producing tool. Several sources/tools can be supplied in one bundle.
Each source must have its own nonempty measurement ID and declare collection
complete. The trusted producing workflow owns that declaration and the series,
which must encode the full source/rule/configuration coverage. This interface is
not a signature verifier or an analyzer-completeness oracle; hashing untrusted
artifacts does not establish their authenticity.

```sh
harness-gate findings normalize --input bundle.json \
  --report /trusted/completed-run/test_result.json --output findings.json
harness-gate findings export --input findings.json --output findings.sarif
```

The exact Core report bytes must match `report_sha256`. Schema 1 must have
`evidence_complete: true`, an invocation ID, consistent PASS/FAIL and `passed`,
and `source_identity: git-tree:<context.tree>`. Produce an immutable staged
report with `verify --staged` in a trusted checkout whose index matches the
analyzed commit. Working-tree/preview reports cannot establish an immutable tree;
WAIVED/CANCELLED reports are outside this first interface and fail explicitly.
The GitHub integration separately verifies that the tree belongs to the exact PR
commit. Local normalization does not assert that a Git commit owns a given tree.

Each finding includes rule, tool and version, severity, locations, stable ID,
source index, JSON pointer and optional fixes. Sources retain arbitrary rules,
properties, snippets and evidence after credential redaction; numeric values and
non-secret structure are preserved. Sensitive-key values become `[REDACTED]`.
Fixes are data and are never executed. Export projects the normalized fields to
SARIF and carries measurement/source pointers, context and Core report digest;
generic fix suggestions use the `properties.harnessGateFixes` extension rather
than promising applicable SARIF edits. It is not a byte-for-byte reconstruction of every original SARIF extension.
Keep the normalized report as the provenance record.

The supported SARIF subset requires tool name/version, explicit results arrays,
ruleId, plain message.text, and repository-relative physical locations with
startLine. Optional endLine and rule default severity are supported. Every
supplied invocation must declare executionSuccessful. URI base IDs, encoded separators or
absolute paths, non-failure/review result kinds, missing locations and unknown
severity fail explicitly; normalize them in a trusted upstream adapter rather
than guessing. Additional locations are preserved and can receive annotations.

Limits are 16 MiB per input/output, 100 source documents, and 10,000 findings or
history identities. Paths are literal repository-relative paths; traversal,
absolute paths, backslashes, URI syntax, literal percent signs and control characters
are rejected. SARIF URI escapes are decoded exactly once and then validated;
encoded separators and traversal are rejected. Export escapes spaces and UTF-8 bytes. Lines are positive inclusive ranges.

## Identity and retained history

The ID is SHA-256 of the JSON tuple `(schema version, namespace, rule, anchor)`.
Default namespace is tool name. Native sources can provide `namespace`; SARIF
uses `properties.harnessGateNamespace`. Only explicit canonical namespace/rule
mapping joins equivalent findings across tools. Tool/version and measurement
identity remain separate provenance even when the issue ID is shared.

Native `anchor` or SARIF `partialFingerprints["harnessGate/v1"]` identifies a
logical occurrence independent of file and line. The producing adapter must
supply a unique anchor per distinct occurrence and keep it stable across moves.
No fuzzy message/line matching is performed. Without this anchor, normalization
emits a location-only identity and a diagnostic; history comparison refuses it.
Other SARIF fingerprints are preserved but not silently promoted to this contract.
Duplicate observations keep their separate evidence and locations; history has
one entry per stable identity and annotation delivery deduplicates identical
identity/path/range tuples, keeping the strongest observed severity.

```sh
harness-gate findings compare --current findings.json --previous previous.json \
  --previous-sha256 "$TRUSTED_PREVIOUS_SHA256" --output compared.json
```

The previous digest must come from independently trusted retained storage, not
from an untrusted artifact's own declaration. Comparison requires the same
repository, series, ordered source formats and tool/version coverage, including
empty result sets. Missing/incomplete tools cannot resolve an issue. It keeps
new/persistent/resolved/reopened entries, original first-seen and last-seen commit,
redacted per-observation source snapshots, and the exact parent file digest. Resolution
does not delete an issue; reappearance reopens it. All output paths use exclusive
creation, so existing reports and checkpoints cannot be overwritten.

The workflow owner must retain the latest digest and previous report chain and
advance the pointer atomically under repository/PR concurrency. GitHub publishing
requires that independent latest pin to match `previous_sha256`; a fresh report
cannot silently replace a compared history. Initial enrollment is a separate
explicit action input. Archival at the history limit needs a reviewed continuation
contract; the CLI refuses automatic deletion or reset. This diagnostic history
never changes the separate authoritative Core quality baseline/debt lineage.

## Explicit GitHub Actions integration

The [composite action](../.github/actions/findings-feedback/action.yml) runs on a
runner with Node 20 or newer. Its default `publish: 'false'` validates the local
report and calculates annotation count without API calls. Use a reviewed immutable
revision of the action and a trusted report producer. Never execute PR-provided
scripts or check out an untrusted head with a write token. Do not use
`pull_request_target` to run project code. Fork feedback is explicitly unsupported
in this first integration and fails before writing.

```yaml
permissions:
  contents: read
  pull-requests: read
  checks: write
concurrency:
  group: findings-${{ github.repository }}-${{ github.event.pull_request.number }}
  cancel-in-progress: false
steps:
  # Download only the authenticated, completed report for this exact run/head.
  # Use the action from a trusted checkout or reviewed immutable action revision.
  - uses: ./.github/actions/findings-feedback
    with:
      report: compared.json
      report-sha256: ${{ steps.trusted_report.outputs.sha256 }}
      previous-sha256: ${{ steps.trusted_history.outputs.latest_sha256 }}
      commit: ${{ github.event.pull_request.head.sha }}
      pull-request: ${{ github.event.pull_request.number }}
      publish: 'true'
```

This is a step fragment, not a complete workflow or authorization to publish.
For the first reviewed enrollment only, omit `previous-sha256` and explicitly set
`initialize-history: 'true'`; later runs must supply the independent latest pin.
The normalized report digest and checkpoint must be obtained from trusted storage,
not merely downloaded alongside attacker-controlled report bytes.

Publishing uses a GitHub App token (`github.token` by default). Minimum permissions
are checks:write, contents:read, pull-requests:read. No issue comments, review
comments, code edits, fixes or code-scanning uploads are performed. Annotations
belong to a neutral advisory check run on the exact analyzed commit and can be
shown inline in the PR. The normal required Core check remains authoritative.

Before writing, the action checks open/same-repository PR head, commit tree,
complete recursive tree, regular-file paths, verified source blobs and line bounds.
It rejects truncated trees and files above 1 MiB. It pages up to 100 pages of 100
check runs, reuses an identical completed delivery, sends at most 50 annotations
per request, and refuses overflow above `max-annotations` (default/hard maximum
1000) before any write. It does not silently truncate feedback.

The PR head is checked before creation, each annotation batch and completion.
Previous commit checks remain historical; they are never moved to a newer commit.
A head change or API error cancels the partially delivered check and fails the
integration; partial prior delivery is not retried as if complete. The caller can
inspect retained evidence before explicitly starting a new delivery. Concurrent
identical publishers must be serialized by the caller's concurrency group.
Secrets in outgoing messages are redacted again; no external text enters a shell
command, workflow expression, arbitrary URL, title, or command log. Integration
failures use a nonzero action exit, with the original Core result unchanged.

The implementation follows [SARIF 2.1.0](https://docs.oasis-open.org/sarif/sarif/v2.1.0/os/sarif-v2.1.0-os.html)
and GitHub's [Checks API](https://docs.github.com/en/rest/checks/runs?apiVersion=2022-11-28).

## Compatibility, rollout and rollback

Existing verification, report schema 1, configuration version 2 and collector
protocols are unchanged. New consumers opt in to a separate versioned contract.
Future incompatible identity or normalization rules require a new version and
explicit history migration; unknown versions fail closed. No historical report
is adopted into a new measurement series automatically.

Rollback removes the optional action invocation and stops calling `findings`.
Keep original Core reports, normalized/source/history artifacts and independent
checkpoint pins. No baseline rewrite, waiver expansion, gate skip or release
permission is needed. This first release has no hosted history service, dashboard,
automatic fix application or automatic posting workflow.
