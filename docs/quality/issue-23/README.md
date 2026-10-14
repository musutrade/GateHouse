# Issue #23: scope detection measurement certification

The first [PR #35 CI run](https://github.com/musutrade/GateHouse/actions/runs/38518091224)
correctly rejected the production change to `src/scope/detection.rs`: the module
was absent from the required risk source inventory. Tests and coverage collection
passed, but risk collection refused the unsupported source. The missing sealed
producer evidence then blocked the aggregate and shadow consumer.

## Supported boundary and validation

Add only `scope/detection.rs` to `source_measure.SOURCE_FILES`. The pinned
GateProbes `rust-measure-v0.3.1` analyzer inventories its two production functions,
`detect` and `ensure_git_worktree`, without closures, source insertions or
unsupported syntax. Regression checks validate the AST and unchanged reparse
complexity under Linux amd64, macOS arm64 and Windows amd64 target configurations.
Unknown scope modules still fail the production-source review.

Real Linux LLVM collection used the committed PR head
`d83e564bfcbbcbf004fe34b3b0d8cf270ac7061a`, a disposable Git archive, and the
released digest-verified measurer. Eleven focused tests exercised existing scope
cases, the non-Git error, 60 rename/path cases and six actual verify cases. The
strict existing `measure` implementation joined both production functions to
their own LLVM records without missing or borrowed mappings.

| Function | Complexity | Line coverage | Region coverage | Exact CRAP | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| `detect` | 16 | 63/65 (96.92%) | 96/104 (92.31%) | 4396048/274625 | PASS |
| `ensure_git_worktree` | 4 | 7/7 (100%) | 11/12 (91.67%) | 4/1 | PASS |

Raw local manifests, LLVM exports and risk rows are retained in
`/tmp/gatehouse-issue23-scope-certification/`; the committed digest record is
[native-certification.json](native-certification.json). This focused collection
certifies the source boundary; it is not a replacement for complete CI collection
or a new baseline. Three-target AST checks do not claim three-platform native
coverage. Full hosted base/head collection and comparison remain required.

## Policy and compatibility

Engineering Policy semantics are unchanged. The analyzer, metric definitions,
compiler configuration, instrumentation, LLVM mapping, function selection,
CRAP <= 30, high-complexity coverage >= 80%, debt/ratchet, requiredness and Core
decision/release authority remain unchanged. This is an explicit supported-source
inventory addition backed by actual collection, not a blanket waiver.

CI measures base and head again using the same expanded inventory and pinned
tool declaration. Existing function identities remain intact; historical reports
are immutable and are not compared against a different inventory or rewritten.
No configuration/protocol migration or baseline adoption is needed. Reverting
the certification alone restores the measurement-review failure for scope
changes; rolling back the fix also restores the original rename defect. Neither
rollback can make unsupported measurements pass.
