# Content-bound directory inputs

`harness-gate --project-root PATH verify --all --snapshot` copies a local
directory into a private temporary input before running the existing Rust Core
verification plan. It does not query Git, GitHub, PR state or a scheduler.
Configuration, required steps and quality evaluation retain their usual meaning.
Reports stay in the original project's configured report directory.

This is an explicit input mode. Ordinary working-tree and staged invocations
retain their existing interfaces. `--snapshot` requires `--all`; changed
path, Git baseline, explicit component and plan-only selection are not supported
in this first directory mode. A missing quality-policy baseline still fails under
the existing quality contract. A directory snapshot does not invent a HEAD,
changed-path set or baseline, and does not supply Git history to checks that need
it. Such checks must use a supported Git input instead.

For index bytes, use `verify --all --snapshot --snapshot-source index`.
The Git adapter validates and materializes the index, then applies the same
regular-file manifest and normalization as the directory adapter. Its reported
input mode is `content-index`; both adapters use `content-v1:sha256:` identity.
Identical supported files/configuration therefore have identical content identity.
This verifies the whole selected index input, not a changed-path subset. It
does not read or parse unstaged workflow edits, and does not invent a HEAD or
use HEAD as the candidate identity. `--project-root`, when supplied, must be the
Git repository root. Existing ordinary staged verification remains unchanged.

## Content identity

The invocation records a version 1 `candidate_manifest`. Each regular file has a
UTF-8 relative path, `kind = file`, normalized mode, byte count and SHA-256 digest.
Entries sort by UTF-8 path bytes. Identity is
`content-v1:sha256:` followed by SHA-256 of the compact JSON serialization of that
manifest. The field order is `version, files`, with file fields
`path, kind, mode, bytes, sha256`.

Normalization retains only whether any POSIX executable bit is set: files become
0755 or 0644. Non-Unix platforms use 0644. Other permissions, timestamps,
ownership, hard-link relationships and empty directories are not part of this
portable identity. There is no claim of equivalence for those filesystem facts.
Symlinks, special files, non-UTF-8 paths and path components containing backslash
or colon are rejected. Nested Git repositories/submodules are rejected.

Only the top-level `.git` administrative entry and the exact configured report
subtree are omitted. Ignore files do not select the file set. Build artifacts,
untracked files and other hidden files remain input; provide a clean directory.
The current bound is 100,000 entries and 1 GiB of regular-file contents.
Identical supported files have identical identities regardless of their absolute
directory or presence of top-level Git metadata. This does not imply equivalent
policies, baselines, tools, platforms or execution conditions.

The copy is checked against the manifest, and the source is checked again after
materialization. Before a result can be published, Core checks that the execution
input still matches. The source must remain stable while copying; preventing adversarial concurrent
writers is an execution-environment responsibility. Steps must put build products and other writes outside candidate input
(for example under their declared report output). The quality collector may
write its explicitly declared artifact root inside the snapshot only when that
root contains no candidate files. Core creates missing empty artifact directories,
rejects overlap or symlink aliases, and excludes only that verified collection
output root from the final input comparison. It still validates collected
artifacts through the existing Core evidence contract. Other writes reject
the invocation. Repository-bound steps are rejected because they would bypass
the selected input. The private input is removed when the invocation releases it;
its manifest remains in the existing invocation evidence.

## Trust and delivery boundary

This input mode is not an OS sandbox or production isolation claim. Execution
identity, protected policies/tools, process containment and evidence access must
be supplied by the execution environment. Candidate-provided commands still run
with the caller's privileges. Optional Git-based step reuse is bypassed in this mode; existing reuse behavior
for other input modes is unchanged. No installer, cache manager or result publisher is
started by selecting this mode.

Issue #130 also tracks explicit non-SCM baseline acceptance. This change does
not close #130 or the architecture umbrella #134. Plugin measurement
implementations remain in [GateProbes](https://github.com/musutrade/GateProbes).

## Explicit execution root and non-SCM baseline

Signed collector requests bind their workspace and artifact roots. When the
trusted host needs to know the execution root before signing, supply
`--snapshot-destination /absolute/new/path`. Its parent must already exist;
the destination must be new and separate from candidate input. Core creates it
with private permissions on Unix, copies and verifies input, then removes its
own directory on invocation cleanup. Existing destinations are rejected rather
than reused or removed. Both snapshot adapters support this option. Without it,
Core allocates a private temporary directory as before.

The host must sign requests for that explicit destination before invoking Core.
Core never rewrites signed roots, arguments or permissions to make an old request
fit a new invocation. A root/signature mismatch fails existing protocol checks.
This option names storage; it does not attest the host or grant isolation.

For a non-SCM baseline, use the existing quality configuration
`baseline.provider.kind = "retained_artifact"` and its explicit host-pinned
`quality-baseline-request/v1`. The existing bundle includes the base state,
evidence and exact source/config/artifact file inventory. Core validates those
bytes with the same quality/ratchet engine; it neither queries Git nor substitutes
the candidate for a missing baseline. A required absent, corrupted or wrong
lineage baseline remains a failure. Fresh collector output roots must not
overlap input files; pre-existing candidate files are never reclassified as
disposable output.
