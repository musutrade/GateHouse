# Content-bound directory inputs

`harness-gate --project-root PATH verify --all --snapshot` copies a local
directory into a private temporary input before running the existing Rust Core
verification plan. It does not query Git, GitHub, PR state or a scheduler.
Configuration, required steps and quality evaluation retain their usual meaning.
Reports stay in the original project's configured report directory.

This is an explicit input mode. Ordinary working-tree and staged invocations
retain their existing interfaces. `--snapshot` requires `--all`; staged, changed
path, Git baseline, explicit component and plan-only selection are not supported
in this first directory mode. A missing quality-policy baseline still fails under
the existing quality contract. A directory snapshot does not invent a HEAD,
changed-path set or baseline, and does not supply Git history to checks that need
it. Such checks must use a supported Git input instead.

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
writers is an execution-environment responsibility. Steps must put build products and other writes outside the
snapshot (for example under their declared report output), or this mode rejects
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

Issue #130 also tracks explicit non-SCM baseline support and shared content
identities for the staged Git adapter. Those remain separate work; this change
does not close #130 or the architecture umbrella #134. Plugin measurement
implementations remain in [GateProbes](https://github.com/musutrade/GateProbes).
