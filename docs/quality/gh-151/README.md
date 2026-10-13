# GH-151: Rust generic authority transfer

OpenSpec tasks 6.1–6.3 transfer generic evaluation and reporting to the released
Rust core. The operator-authorized transfer decision follows successful hosted
shadow and required CI on reviewed candidate
`0fc96d7644d623f31e31fe1a512cb68db0b2003b`. The separate transfer commit enables
`quality evaluate`; its full SHA and final hosted validation are recorded in
[PR #158](https://github.com/musutrade/Harness-Gate/pull/158). Task 7 and full
proposal acceptance remain outside this issue. The required aggregate is unchanged.

The predecessor is complete: GH-150 PR [#157](https://github.com/musutrade/Harness-Gate/pull/157)
merged as `9a73ae0f7c61d804fbfc2fe35eac8377eb1fc22e` after required CI succeeded
on `4bc5d5bd6486e47458666ca3c516a1d909268c89`. Its
[Required Quality Aggregate](https://github.com/musutrade/Harness-Gate/actions/runs/34231377223/job/102081158898)
and [Generic Quality Shadow](https://github.com/musutrade/Harness-Gate/actions/runs/34231377223/job/102081158943)
are green. That existing shadow job projects Python reference results; it is not
evidence that the new Rust shadow workflow has run.

## Authoritative Rust entry point

The `harness-gate` package now includes the generic Rust library and its embedded
schemas. The unpublished `harness-gate-quality-core` workspace package becomes a
compatibility facade for the migration comparator, which uses the same library
as the executable. Publishing the CLI does not require publishing that facade.

The accepted candidate exposed `quality shadow`. After the explicit acceptance
below, the separate transfer commit enables `quality evaluate` as the authoritative
generic decision and report entry point. It consumes existing contracts with
trusted context supplied separately from collector records:

```bash
harness-gate quality evaluate \
  --project project.json --policy policy.json --evidence evidence.json \
  --expected expected.json --source-root source --artifact-root artifacts \
  --now 2026-09-08T00:00:00Z --output project-report.json
```

`--selection`, `--mappings`, and `--exceptions` accept the existing selection,
lineage and exception documents. Baseline comparison requires all of
`--base-project`, `--base-evidence`, `--base-expected`, `--base-source-root`, and
`--base-artifact-root`. An omitted clock uses UTC now. The command does not
discover a project manifest or manufacture expected collector/source identity.

The output is the existing `harness-project-report/v1` document. Its legacy
`mode: "shadow"` field is preserved byte-for-byte in the compatibility contract;
the field is not a release-approval signal. Exit 0 requires aggregate `pass`;
policy failures and evaluation errors exit 1, and invalid CLI arguments exit 2.
An evaluation error removes a prior report rather than leaving a stale pass.
Input/output document aliases are refused. No collector, Python interpreter or
Python C-class module is invoked by evaluation or reporting.

External collectors retain protocol v1 and measure only. The Rust library owns
validation, policy requiredness, aggregation, debt/trend, relationships and report
indexes. The frozen oracle, external schemas, collectors and goldens are unchanged.
Task 7's final Python dispositions remain outside this issue.

## Opt-in shadow evidence

The separate `Rust Generic Shadow` workflow runs on manual dispatch, or on PRs
and main pushes when repository variable `RUST_GENERIC_SHADOW` equals `true`.
Same-repository PRs from `symphony/GH-151` also opt in, as authorized by the
operator; no repository-variable write is required.
It builds the product binary and migration comparator, then runs:

```bash
python3 tools/quality/fixtures/generic-core/authority.py \
  --harness-gate tools/harness-gate/target/debug/harness-gate \
  --rust-replay tools/harness-gate/target/debug/examples/differential_replay \
  --output target/quality/rust-generic-shadow
```

Use fresh output directories. With `CARGO_TARGET_DIR`, substitute that directory's
binary paths. The driver verifies the frozen corpus, saves complete Python and
Rust outputs, retains every field difference, and checks the installed command's
report and exit status with an empty executable search path. Both ecosystems and
the consolidated negative matrix run without recollecting native evidence.
The only classified diagnostic variation is GH-150's missing-file OS wording;
original strings and logical paths remain retained. Any unexplained difference
fails the job. Acceptance artifacts explicitly declare `authoritative: false`.

An `always()` artifact upload retains both engines, CLI reports, stderr, field
differences and materialized evidence for 30 days. Download evidence before that
retention window expires if it is needed for a later review. The existing
`.github/workflows/ci.yml` is unchanged, including the `Required Quality Aggregate`
name, dependencies, thresholds and release authority. Branch protection and repository variables remain unchanged. Each artifact
includes the exact checked-out PR head SHA and hosted run identity.

## Explicit transfer decision: accepted candidate

The operator authorized versioned measurement extensions on 2026-09-08.
`production-source-2` includes the linked core as an eleventh blocking production
boundary, including production outside `src/`. Wire declarations without LLVM
counters are source-hash pinned; no counters are invented. The unpublished
comparator facade is migration tooling, while its linked library remains production.
`mccabe-rust-3` / analyzer 0.3.0 measures all 23 declared production source paths,
with explicit absent-source records for predecessor files. Both revisions use the
same analyzer, instrumentation and thresholds. Every measured production symbol
must join real LLVM counters. Historical debt retains syntax-based lineage.

The [hosted API records and explicit decision](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-151/accepted-candidate/decision.json)
accept reviewed candidate `0fc96d7644d623f31e31fe1a512cb68db0b2003b` only after:

- [Rust Generic Shadow run 34243502830](https://github.com/musutrade/Harness-Gate/actions/runs/34243502830) succeeded: 33 cases (14 Rust, 16 Angular, 3 contract), zero unresolved mismatches, and complete CLI reports with an empty executable search path.
- [Required CI run 34243502555](https://github.com/musutrade/Harness-Gate/actions/runs/34243502555) succeeded, including all existing required jobs and `Required Quality Aggregate`. Its GitHub merge checkout was `4e0c4b59bc721120777dda8ccfbd21ebdde84b4f` for that reviewed head.
- The retained required test log records 332 passing tests, including the consolidated 12-category negative matrix and the shipped CLI corpus test.
- Rollback below was documented before acceptance.

[All raw Python/Rust/CLI comparisons](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-151/shadow-0fc96d7.tar.xz) are retained with
identity, the original artifact index and per-file SHA-256 hashes. Archive SHA-256:
`de982349e25a5d8e0c138130eca7e51d8c569d2e99eb0abdbbab520f5f25b7a2`.
Hosted artifact `10063111207` retains complete materialized evidence for 30 days;
the frozen corpus retains original inputs. Required coverage/risk evidence is
artifact `10063624125`. The raw API records retain every job conclusion, including
push-only jobs correctly skipped by the unchanged pull-request workflow.

[Downloaded required quality reports and collection logs](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-151/quality-0fc96d7.tar.xz)
retain the successful production, risk and critical-path stages, both risk
reports, candidate identity and archive/per-file manifests. Archive SHA-256:
`b70587783a33dc116dad2e7275aacaf4bf63d76dca436e645ac5c0a52754d557`.
The hosted artifact retains the additional native coverage profiles and sources.

Acceptance is based on this candidate's hosted results, not predecessor or local
results. The transfer commit is a distinct descendant; PR #158 records both full
commit identities and the final shadow/required CI results. The existing required
aggregate retains its name, dependencies and authority. Differential tooling stays
non-authoritative; it cannot substitute for a required result.

Earlier failed boundary and read-only Git diagnostics in `validation.json`,
`validation.tar.gz` and `validation-retry.tar.gz` are historical records. Current
operator authorization permits these measurement extensions and writing this
workspace's Git metadata; staging has been verified. They are not current blockers.

## Local transfer validation

[Exact commands, environment, exit statuses and complete logs](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-151/local-transfer/validation.json)
record 332 passing Rust tests and 293 passing Python tests. Cargo formatting,
Clippy with warnings denied, documentation consistency and strict OpenSpec
validation pass. The CLI corpus test exercises authoritative `quality evaluate`
with an empty executable search path. Local Cargo home and target directories
avoid the host's read-only/shared build configuration. Final hosted shadow and
required checks validate the distinct transfer SHA recorded in PR #158.

## Candidate review iterations

PR [#158](https://github.com/musutrade/Harness-Gate/pull/158) reviews this change.
Candidate `7870dcc43a9f974aca38a9dbf1736dc19a1d873d` passed hosted Rust shadow
[run 34241643293](https://github.com/musutrade/Harness-Gate/actions/runs/34241643293)
with 33 cases and zero unresolved mismatches. Its required CI did **not** pass:
risk preflight treated the unchanged move of the unpublished replay example as
an unsupported production deletion. Transfer was not accepted.

The repair recognizes only Git's exact, 100%-similarity relocation from
`quality-core/examples/differential_replay.rs` to
`quality-replay/examples/differential_replay.rs`. A changed executable is rejected;
all linked core production symbols remain in the measured series. A real Git
fixture tests both the exact move and changed-content rejection. Required check
names, dependencies, measurement thresholds and failure handling are unchanged.

[Retained raw comparisons](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-151/shadow-7870dcc.tar.xz) contain all 133 downloaded
Python/Rust/CLI comparison, report, stderr and identity files, plus a per-file
SHA-256 manifest and the original archive index. Their archive SHA-256 is
`78b3e6edba669a4a46f0742e18c29fd1753ca3a3596b02196da106a11e7bce4a`.
The complete materialized inputs remain in hosted artifact `10062375474` and the
frozen corpus. This historical shadow success does not accept another SHA.

## Rollback

Before transfer, disable `RUST_GENERIC_SHADOW`, remove the GH-151 branch opt-in,
and stop manual dispatches. Existing
required CI remains authoritative; preserve downloaded differential artifacts.

After transfer, revert the transfer commit(s) and redeploy the
previous accepted release, restoring its recorded authority path. Keep the
frozen Python reference, corpus, original evidence, baselines and source/series
identities. Re-run required CI before releasing the rollback. Do not interpret a
Rust error as permission to fall back automatically to favorable Python output.
Any later re-transfer repeats corpus, negative-matrix and hosted acceptance.

This checkout has no `.harness-gate/flow.toml` declaring a `ci` profile.
`harness-gate config check` and `harness-gate verify --profile ci --all` are not
applicable and were not run. Task 7 and full-proposal acceptance remain outside
this issue. The controller owns merge and issue closure.
