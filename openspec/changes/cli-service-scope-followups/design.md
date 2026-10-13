# Design

`config check` keeps JSON output machine-only. The human path catches project
discovery diagnostics and prints the stable category, structured field help,
the generic preset repair command, and a minimal schema v2 flow shape.

The scheduler collects service IDs from external plan nodes before entering its
worker loop. `ServiceManager` creates owned warmup jobs that transition a
resource to `Ready { users: 0 }`; jobs run in scoped threads alongside gates and
are joined before the scheduler returns. A failed startup is stored as
`Failed`, so a subsequent lease fails fast with the original detail.

The hidden `scope --benchmark-repeat N` command discovers Git paths once and
repeats classification in memory with cached compiled rules and with a fresh
compile on every iteration. The quality benchmark creates a deterministic
large-path fixture and stores per-sample timing and speedup under a dedicated
`scope_matcher` result field.
