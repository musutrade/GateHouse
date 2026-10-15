# Design

Extend the retained timing normalizer to select required jobs by event and
reject mixed event cohorts. Keep historical PR output stable. Diagnose collection
from existing candidate/log evidence rather than introducing another collector.

Expose an explicit coverage test backend. Standalone defaults to cargo; the
candidate nextest backend uses the same package, lockfile, instrumentation,
exports and gate evaluator, with default profile, zero retries, no fail-fast.
Record the actual command. CI promotion is contingent on whole-suite parity and
measurements. Existing legacy llvm-cov execution does not collect doctests;
nextest introduces no new doctest omission. Build/profile state remains owned by
the existing fresh collector directory. Base/head risk evidence remains separate.

Alternatives: target caching previously had no demonstrated net benefit;
production already reuses raw coverage; matrix already builds once. Parallel
base/head or stage splitting introduces profile/artifact trust and resource
contention costs without current supporting measurements. Test fixture changes
belong to #55 after this shared diagnosis.

Implementation sequence within this change: retain diagnosis, test event-aware
normalization, run bounded complete-suite A/B, promote only if warranted, then
validate required hosted CI and record results. No completion-time promise.

Rollback: remove `--test-runner nextest` from the collector. Keep diagnostic
records and failed runs. No baseline or measurement-series reset is needed.
