# Baseline acceptance corpus

Run `acceptance.py --harness-gate <binary> --output <new-directory>` or the nextest
`trusted_baselines_preserve_lineage_and_reject_stale_unknown_ecosystem_inputs` test.
The script retains generated repositories, requests, manifests, resolved inputs,
reports and `acceptance.json` for inspection.

The corpus derives its unknown ecosystem from the compiler's
`unknown-ecosystem.json`: `nebula-unregistered-2049`, language `quasar`, collector
`quasar-spectrometer`, custom canonical series and capability `quasar.pulses`.
Both Git and artifact providers accept that arbitrary transport identity and a
typed custom pulse count. Stale
custom evidence and incompatible custom series fail. Strict evaluation continues
to reject the unsupported metric; supported generic coverage under the unknown
ecosystem passes and preserves rename/move debt. The Rust reference debt case
uses the same provider. No provider language case is added.

Other cases exercise a divergent merge base, repeated deterministic resolution,
unchanged dirty/untracked head and index bytes, missing refs/requests/manifests,
head substitution, changed config/tool/run identity, inventory and hash tampering,
path traversal/symlinks, invalid mappings and output isolation. Direct and compiled
evaluation must produce identical report bytes and exit status using resolved bases.
