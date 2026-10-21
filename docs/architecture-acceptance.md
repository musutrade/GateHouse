# Result-boundary acceptance

## Core 0.7.1 registry repair candidate

Core 0.7.0 GitHub assets were published from `6242228a16f951bc30a881641c853afefae262a8`;
its crates.io job failed because the runtime protocol dependency was not publishable.
The tag and assets remain immutable. Core 0.7.1 repairs registry packaging and adds
protocol publication and Core package preflight before GitHub asset publication.
Onboarding presets 1.0.2 is published from `85b1856dd7061bfb19683df3921060b49490e5e2`
and pinned by Core CI. Its downloaded archive passed 29-step acceptance with
released 0.7.0 and source-built 0.7.1; the latter is candidate evidence only.
No real daemon acceptance, engine switch, lease migration or #62 completion is
claimed. See [the repair release plan](releases/0.7.1.zh-CN.md).

This matrix tracks the split from #98/#127 under #134. It is an implementation
and evidence index, not an issue-closure or merge record. PR #127 remains
reference-only. Rust Core remains the quality authority; external measurements
belong to [GateProbes](https://github.com/musutrade/GateProbes).

The integrated Core baseline is
`6b834552acc0997a53c2f60be973dca2e7c9841a`. PRs #141–#149 were merged
serially after successful main-target CI on their exact heads. Earlier stacked
PR evidence alone did not establish those CI results.

| PR | Accepted head | CI run (attempt 1) | Merge commit |
| --- | --- | --- | --- |
| #141 | `9267c46` | `39135114416` | `3c281dce` |
| #142 | `2843235` | `39176678418` | `b8485705` |
| #143 | `9768fac` | `39181088732` | `ab2e353` |
| #144 | `e38a581` | `39183691203` | `7b69145` |
| #145 | `0a253b6` | `39190204868` | `900357d` |
| #146 | `7f244f4` | `39192254515` | `102a8db` |
| #147 | `ed1344d` | `39200208180` | `120e73ab` |
| #148 | `72485948` | `39205867198` | `bbf65b4` |
| #149 | `6b8035b` | `39215437635` | `6b83455` |

Each listed run passed Required Quality Aggregate. Skipped jobs remain skipped;
these results do not establish external workload coverage or host attestation.
Issue closure still requires checking the issue's complete acceptance criteria.

| Issue | Implementation and evidence | Remaining acceptance |
| --- | --- | --- |
| #129 | Local results no longer wait for webhook transport. `directory_input_test` checks PASS and FAIL with an unavailable consumer and no connection. [Migration boundaries](result-boundary.md) retain historical records. | The independent consumer is integrated in #146; delivery remains outside Core completion. |
| #130 | Directory and validated index adapters share a normalized content manifest. `index_input_test` proves equal identities and ignores unstaged flow edits. Collector tests cover signed known destinations and explicit retained baselines, including absent/corrupt/wrong-lineage rejection. | Final integrated review of the documented supported filesystem range; no universal filesystem equivalence or sandbox claim. |
| #131 | Optional GateProbes `github-check-v1.0.0` consumes pinned saved Core results; 22 real-Core/fake-API tests cover target/pin/current-use binding, ordering, ambiguous delivery and withdrawal. Integrated in #146. | No live API writes were used for acceptance; operator-protected configuration, credentials and single-owner state remain deployment requirements. |
| #132 | #145 integrates Unix group cleanup before reap on normal completion, final cancellation observation and bounded error propagation. Process termination and adapter cancellation tests passed; Windows CRLF assertions preserve full output checks. | Process groups are not an OS sandbox; Windows directly manages the child only. Independently required host isolation is never inferred from fixture PASS. |
| #133 | Real Core directory/index verification, signed collector fixtures, retained baselines, strict result schema and saved evidence reader share the existing evaluator. Forbidden executable sentinels check the no-Git/no-publisher directory path. | Execution negatives are integrated through #145/#147; the real Rust workload passes through Core, and fixed release compatibility is recorded below; PR #149 supplies enforced local network-denial acceptance. Fixture measurements must remain distinguished from production coverage. |
| #134 | Input, result, evidence and identity boundaries are documented and independently tested in the chain above. | Close remaining rows by evidence; this matrix alone does not satisfy the umbrella. |
| #135 | [Operator runbook](operator-recovery.md) separates authentication, observation, preservation and retry conditions from ordinary verification. | Any actual recovery remains separately authorized environment work. Historical failure is not current host state or a prerequisite for local results. |
| #136 | `report-verify` reuses the existing pinned history resolver. Tests copy evidence, delete the source workspace, verify history and reject tampering. Manifest validation preserves run/path/digest/completeness checks. | Maintain the distinction between historical integrity and current applicability in consumers. |
| #137 | The selected main baseline does not contain #127's publication journal/controller. Core records have no PR, Check or publication-slot state; local regression paths do not require those services. | Preserve this exclusion when adding the consumer; do not import #127 orchestration under a new name. |
| #138 | The selected baseline does not contain #127's dedicated cache manager or maintain-cache interface. Native tool caches stay with the host; existing step-result reuse is retained. Saved-result reading needs no build cache. | Verify final configuration/docs introduce no cache service, layout mandate or maintenance prerequisite. Existing machine cache data remains untouched. |
| #139 | Exact compiled quality identity, separate verification selection identity and actual Core/runtime/collector/series bindings are recorded. Compiler and real-Core tests cover policy/execution separation; missing compiled policy is explicit. | PR #149 supplies the full-quality threshold/execution and stale-runtime regressions; independently trusted current-use checks remain necessary. `not_proven` is not permission to reuse. |
| #140 | Existing signed collector protocol and generic Rust evaluator are retained; fixture collectors supply facts, not a second project quality decision. | The fixed GateProbes artifact and source-independent released-Core compatibility are delivered below. PR #149 supplies shared #133 result-boundary acceptance; retain the limited Rust coverage declaration. |

## Evidence limits

Independent local evidence includes the original batch-specific isolated-target
regressions, PR #143 identity/schema/verify tests (46/46), PR #145 targeted
cancellation/process tests (13/13), and PR #146 consumer tests (22/22). These
counts belong to distinct revisions and suites; they must not be summed or
represented as one final full-suite execution. The CI table records subsequent
exact-head integration checks.

GateProbes #26 candidate `475c5ba14bc4878649eb3a8a1988fa10eff49aeb` is a
data-only unpartitioned Rust test preset. Independent acceptance has verified
signed installation, tampering/untrusted-key/lock/compatibility rejection, and
upgrade/rollback with unchanged Core bytes, using a disposable test signer.
These fixture signatures are not production release trust.

PR #147 preserves synthetic test input bytes while avoiding source-scan false
positives, without changing security rules or assertions. It also replaces an
artifact-exists cancellation trigger with an explicit signal after the fixture
closes its artifact and flushes its response. The old empty-file race failure is
retained; a retry that happened to pass is not the accepted fix.

On merged Core `120e73ab`, binary SHA-256
`9d58056098b5f739221be37a5724ac215318b35783445e3f54aad5ed63114dec`,
Core-owned `verify --all` passes the unchanged default secret scan,
architecture audit and the 749-test unpartitioned Rust workload. Earlier direct
nextest success at `102a8db` remains separate from the Core verification that
was blocked there. The workload does not claim Node action tests, standalone
schema/onboarding checks or quality/risk stages from the Linux CI job.

Core v0.6.0 is now an immutable release from `bbf65b4244236df217cf0ed145ccd0f60a18ac25`,
published by release run `39206824114`. Its Linux binary SHA-256 is
`cc14c94a42eeb272772a8605a3ff524300bfc9922fc5b195a792dc54a2d769cb`.
Released-binary onboarding passed 29 real steps. This does not relabel the
749-test result above as a run with the newly released binary.

GateProbes workload source is merged through PR #27; onboarding compatibility
with Core 0.6.0 is merged through PR #28. Workload release `gatehouse-linux-test-v1.0.0` was published from GateProbes
`3d146769bf14c4669defb9f72d65f4f1b6af3a15` by run `39212301628`.
The final archive SHA-256 is
`d8561885373e7a70e3e6dba52adea105707c0ffa62c49ff6be38c07c4a68ac03`;
the SBOM SHA-256 is
`50a38cb6e83a8e5c06d06bd5b0d00a548c7d5f2585967f958a7e5acce49ede70`.
These are the published bytes, superseding both development archive identities.

Independent downloaded-artifact acceptance with released Core v0.6.0 verified
SBOM members, prepare/plan/apply, plugin initialization, configuration and
selection of `linux.rust-tests`. Payload tampering was rejected with
`HGPLUGIN-DIGEST`. Distribution provenance binds the repository release workflow;
the temporary operator key used in installation acceptance remains a separate
trust boundary. This acceptance did not rerun the 749-test workload with Core
v0.6.0. PR #149 supplies shared result-boundary acceptance. Publication alone is not
the closure criterion for GP #26 or GH #140.

The directory regression removes GitHub tokens and traps git/gh/herdr/systemctl/
sudo through PATH sentinels. This checks those command paths, but is not proof
of enforced network denial or an exhaustive absolute-path process audit. PR #149 establishes the network boundary separately as recorded below.

Content checks detect supported mutations. They cannot protect against hostile
same-account change-and-restore. Runtime fields explicitly record
`not_attested`, `not_provided` and `not_proven`; no digest grants execution
authority. Consumers must verify their current independently trusted inputs and
environment requirements. Missing proof must not be converted to a successful
platform status, and consumer failure must not rewrite historical Core results.

No row authorizes automatic merge, host installation, recovery, cache cleanup,
or migration of PR #127. Publication and agent coordination remain outside
the delivered Core's runtime and test prerequisites.

## Final shared acceptance

PR #149 adds real full-quality pairs in one fixture root with unchanged collector
bytes. Threshold-only edits change both policy identities, preserve runtime
identity, and produce a complete quality FAIL. Execution-only edits preserve
both policy identities, change runtime identity, and complete PASS. The trusted
consumer rejects an old result under the new runtime requirement before API
transport or state creation; historical verification still succeeds.

Local acceptance passed 28 tests, including real Core inside unprivileged
`bwrap --unshare-net` (uid 1000, no GitHub credentials). The same listener probe
connects outside and is denied inside. The fixture produces a verified PASS;
separate consumer delivery uses fake transport. This does not make Core an OS
sandbox or require a publisher for verification.

Final CI run `39215437635` executed 26 tests, including all four paired identity
tests. The network class explicitly skipped because the hosted runner lacked an
available unprivileged namespace mechanism. Local enforcement is not CI network
isolation. The earlier green run `39214765287` skipped all consumer tests due to
an absent default binary path and is not accepted evidence. The corrected
workflow resolves Cargo's actual target directory. Main run `39216061407`
succeeded. These environment limits must remain visible in closure records.

## Criterion review outcomes

Each original #129–#140 criterion was reviewed against merged code, tests and
the evidence above. Satisfied rows and their decisive evidence:

- #129: real-Core directory verification completes with an unavailable consumer
  and no Git/publisher state (`directory_input_test`); preview never constructs a
  GitHub client and token-free apply leaves no state (`test_github_check`);
  [result-boundary.md](result-boundary.md) documents record migration and keeps
  unknown historical states unchanged.
- #130: directory and validated-index adapters emit the same `content-v1`
  identity; [input-snapshots.md](input-snapshots.md) fixes the supported
  normalized range, rejects symlinks/submodules/escapes/duplicates/colon paths,
  records materialized-input change rejection, and requires a pinned
  `retained_artifact` baseline, with absent/corrupt/wrong-lineage baselines
  failing closed.
- #131: the optional consumer maps only pinned saved results to Checks with fake
  transport in tests; duplicate/ambiguous/old delivery remains fenced and API
  errors never fabricate success; `verify` no longer delivers webhooks and warns
  on legacy notification configuration.
- #132: process group cleanup, cancellation and error propagation tests
  (13/13 targeted) reject PASS on unstopped processes or unusable evidence;
  [execution-boundary.md](execution-boundary.md) records environment
  requirements without image-size or recovery prerequisites.
- #133: PR #149 local run enforces network denial for the real Core fixture and
  rejects side-effect paths; consumer unavailability, unmerged-PR state and
  resource leases are independent of result completion.
- #135: [operator-recovery.md](operator-recovery.md) provides the authorization,
  observation, failure-preservation and retry runbook; no live recovery was
  attempted here and none is a gate prerequisite.
- #136: saved results verify after source cleanup, reject tampering, digest
  mismatch and cross-run references, and remain readable without a build cache;
  historical integrity stays separate from current applicability.
- #137: no publication slot, batch, PR, Check or merge state exists in run
  records; old runs cannot alter other runs; no scheduler or herdr coupling was
  added.
- #138: the baseline contains no cache manager, compatibility keys, TTL/GC,
  `maintain-cache` or dedicated cache volume, and configuration/installation
  docs contain no replacement; native caches remain host-owned and #32 step
  reuse is unchanged.
- #139: `result_identities_test` separates policy and execution identity
  movements; PR #149 adds the paired threshold/execution regressions and the
  stale-runtime rejection; `not_proven` is explicit and never reuse permission.
- #140: fixed plugin artifacts load without a plugin source checkout (workload
  release `gatehouse-linux-test-v1.0.0` from `3d146769`), downloaded-artifact
  acceptance ran with the released Core v0.6.0 binary, and coverage is declared
  as linux-x86_64 unpartitioned Rust tests only. The 749-test workload run stays
  attributed to merged Core `120e73ab` and is not relabeled for v0.6.0.
- #134 / #98: closed with the dependent rows above accounted for; #135 remains
  an independent operational track and is not a prerequisite.

Retained limits, explicitly not closure blockers: hosted CI network isolation is
not claimed (#133 local enforcement only, CI class skip recorded); the released
Core v0.6.0 binary did not rerun the 749-test workload; fixture signatures and
the disposable test signer are not production trust; and no production OS
isolation or host recovery is claimed.

## External artifact ownership

GitHub consumer implementation and 22 platform behavior tests migrated to GateProbes. Core retains four paired identity and two network-boundary contract tests against its digest-pinned released artifact. Onboarding compatibility consumes the published bundle, not a plugin source checkout. Exact tags, archive digests and source commits are in `tools/compatibility/artifacts.json`; this CI adapter never runs on the Core verification path. Published plugin acceptance remains Linux x86_64 only.

## External GitHub action ownership

The findings-feedback and report-artifact actions and their 16 platform tests
moved unchanged to GateProbes `github-actions-v1.0.0`, release source
`fffe3589e8920d14aba4e47338d316ecdbb3ba14`. Archive SHA-256 is
`e23d272d80aa9cacadfadd58266a14dd80fe9199c64f2f4d4a4959663899dab0`.
Downloaded extracted tests passed without live API writes/uploads. Core CI uses
the external report action pinned to that full commit for its synthetic UI
example; Core retains report generation/tamper regressions. GateProbes owns
platform transport implementation and tests, not quality decisions. Existing
explicit upload/publish choices remain unchanged.

## Installed-runtime delivery candidate 0.8.0

Installed host source a4e6dba and GateProbes offline importer/joint CI c63f1b6 are
merged. CI verifies source-built Core with independently pinned runtime0.1.1;
this is not released-Core acceptance or engine migration. Core0.8.0, protocol0.1.1,
onboarding1.0.3 and standalone importer0.1.0 delivery remain pending. Retain
runtime0.1.1 bytes and its old plugin-manager/accept.py compatibility boundary.

Onboarding1.0.3 prerequisite is published from `727dd742510afd01c22d91dc90d5ffe474300531`
(release run39554478429), archive SHA256
`d4d3889c199137b067845eac7869a372134e4e4b899895235d9cc2f0362b7337`.
Core CI now pins this immutable archive. Downloaded presets passed 29 steps with
released Core0.7.1 and source-built0.8.0; downloaded Core0.8.0 reacceptance remains pending.
