# Quality collection overhead

The [measured run](https://github.com/musutrade/Harness-Gate/actions/runs/34806146019/job/103858306352)
took 844 seconds in Quality Coverage and Critical Paths. The retained
[timing extract](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/quality-collection-before.json) records the candidate identity
and its SHA-256, command durations, archive sizes and test-run summaries.

| Stage | Seconds |
| --- | ---: |
| Legacy coverage | 247.05 |
| Production evaluation of retained coverage | 2.17 |
| Base/head function risk | 439.42 |
| Critical paths | 76.70 |

Each risk-side build took about 20 seconds; tests took about 183 seconds.
The Python evidence oracle generated all 399 cases for each of five separate
nextest processes. The policy oracle generated its full matrix twice. Their
in-process weak caches could not share data across nextest processes.

The evidence matrix now runs the same five assertion groups inside one test,
and the policy matrix runs both assertion groups inside one test. Every original
assertion remains, including source/artifact mutation and contract provenance.
The complete 399-case evidence oracle and policy oracle still run independently
for each measured source snapshot. No oracle module, corpus, expected outcome,
production implementation or coverage/risk threshold changes. Temporary fixtures
remain owned until all their consumers finish and are dropped on success or
unwind. The number of Rust test entry points drops by five; assertion coverage
does not drop.

Source snapshots retain `tools/harness-gate`, `tools/quality`, `schema` and
`docs/dogfood` from the exact measured commit. Rust integration tests and their
Python oracles consume these trees, including embedded documentation fixtures.
Historical reports in other documentation directories are not collection inputs.
The former broad `docs` archive copied about 704 MB per side, mostly already
compressed historical evidence; the uploaded ZIP was 1,358,880,700 bytes.
Both source archives and all newly collected raw evidence remain mandatory,
hashed candidate artifacts. Fresh coverage profiles, separate base/head
measurements, all four stages and unconditional failure-evidence upload remain.

Each collection command now prints its start, exit code and elapsed seconds to
the hosted log while retaining its full output and timing in the candidate.
This exposes compilation, test and measurement delays without downloading the
artifact first. Hosted after-state timing is needed to quantify the improvement;
these changes do not claim a result based only on local timing.
