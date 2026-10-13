# Synthetic project identity fixtures

`base.json` declares Angular/TypeScript, Rust, Python and Java components using
the same model. Three directed relationships join the components; the API/client
relationship also references a synthetic OpenAPI contract subject. The `sources/`
files provide reproducible digest bytes, not executable adapter tests. Only
Python's standard library is needed.

`head.json`, `head-sources/` and `mappings.json` demonstrate a method rename,
a function move, and a function split. Tests check both snapshots' digests against
retained source bytes. Identities and spans are synthetic declarations; no real
collector is invoked and no measurement results are claimed.

```bash
python3 tools/quality/project_model.py tools/quality/fixtures/project-model/base.json
python3 -m unittest tools.quality.tests.test_project_model -v
```

Negative fixtures are mutations in `test_project_model.py`, covering unknown and
duplicate references, source/path conflicts, unknown versions/kinds, absent or
malformed digests, ambiguous lineage and unauthorized baseline inheritance.
No metric, adapter capability, baseline acceptance or release result is asserted.
