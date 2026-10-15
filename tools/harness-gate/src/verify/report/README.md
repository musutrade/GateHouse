# Report module boundaries

`verify::publish_report` calls the report writer before quality publication,
compatibility mirroring and notifications. The report module keeps ownership of
filesystem evidence finalization; `rendering` supplies text without publishing it.

| Responsibility | Implementation | Dependencies / callers |
| --- | --- | --- |
| Machine model and serialization | `report.rs`: machine structs, `machine_result` | Verification model, failure contracts; consumed by JSON publication and `VerificationReport` serialization |
| Invocation allocation and metadata | `report.rs`: `allocate_invocation`, metadata writers | Project identity, report-directory lease, confined filesystem writes |
| Publication and evidence integrity | `report.rs`: `write`, registry/manifest assessment and verification | Declared artifacts, digests, redaction, report paths; called by verifier |
| Templates and path checks | `report.rs`: `configured_html`, template loading, `report_target` | Repository files, Tera, path confinement; called during document publication |
| Retention | `report.rs`: pruning and completion binding | Lease/guard ownership, completion certificates; runs after finalization |
| Notification | `report.rs`: `notify`, policy transport/resolver | Webhook configuration and network policy; called after publication |
| Pure text rendering | `rendering.rs`: `markdown`, `junit`, `escape` | Verification model, redaction, quality diagnostics; called by document writer |
| Regression tests | `report.rs`: `tests`, additional `cfg(test)` helpers | Exercise rendering through parent imports and real publication boundaries |

The initial split isolates formatting because it needs no filesystem, lease or
network access. Previously those functions shared a source module with all of
those operations, obscuring which code could be changed without touching
publication. Rendering remains a private child module; it is not an external
formatting API. Its only parent-facing exports are the three formatting helpers.
HTML remains with template loading because the actual production path reads and
validates template files. The small `render_html` helper is test-only and is not
mistaken for the production HTML renderer.

This is a responsibility split, not a claimed complexity or performance reduction.
Machine formats, formatting bytes, waiver/skipped-step representation, redaction,
path safety, atomic writes, manifest ordering and failure behavior are unchanged.
The existing tests remain at their established paths. Production/test separation
continues to follow the production inventory and compiler configuration, rather
than treating every text line in `report.rs` as production code.

`verify/report/rendering.rs` is included in both the blocking production coverage
inventory and the certified source measurement input set. Base and head must use
the same input inventory; the path is absent at base. Moved functions acquire new
source identities under the existing `(source, kind, syntax hash)` comparison
and must pass the normal changed-function gates. No historical debt exemption is
transferred to the new path. Critical-path bindings are refreshed for the retained
publication functions and tests, without changing their probes or assertions.
These changes preserve Engineering Policy and the accepted measurement series.
