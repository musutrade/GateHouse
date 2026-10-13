# Accepted Phase 1 Baseline

This is the accepted `main` quality-baseline evidence package. It was captured
from commit `16873ca9c5ebbd06c6b06d4391ab098fafc05194` by run
[37296148100](https://github.com/musutrade/Harness-Gate/actions/runs/37296148100)
on 2026-10-05.

The committed summary is [current.json](current.json) with the reviewable
Markdown companion [current.md](current.md). The original raw per-sample
reports are retained in the run's
[quality baseline artifact](https://github.com/musutrade/Harness-Gate/actions/runs/37296148100/artifacts/11339193006).

The previous accepted baseline (commit
`b92755f9ec87c251516863a0136c28f811187ab5`) is retained byte-for-byte in
[phase-1-accepted-b92755f.json](../../../tools/quality/fixtures/rust-reference/phase-1-accepted-b92755f.json)
so historical compatibility replay remains pinned to its original series.

Baseline series:

`x86_64-unknown-linux-gnu:rustc 1.99.0 (b940084d7 2026-09-28):1:1:cold-and-warm`

The five-sample Linux verification medians are 0.769s serial and 0.451s
parallel with a configured and observed peak of two workers. The scope matcher
cached median is 5703.1us and the uncached median is 10925.4us (1.92x speedup).
The test warm median is 116.823s after a 162.193s cold sample. The Linux
release-small binary is 10,249,672 bytes with SHA-256
`85ec6cccd1b321ad952709e02beee13e59d8fa7b6d6765f7c0fc040f898e8fa5`.

The pull-request `quality-baseline` matrix also captures the same benchmark and
binary-size evidence on macOS and Windows. Those artifacts are platform-local
series and are not compared numerically with this Linux canonical record.

Future baseline updates must be raised as reviewed pull requests by the
scheduled/manual refresh workflow. A local checkout containing unrelated
project audit rules may fail audit fixture tests; the clean-checkout CI run is
the authoritative baseline evidence for this package.
