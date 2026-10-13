# TypeScript original-source and measurement semantics

GH-130 implements OpenSpec tasks **2.1–2.2**, under
[ADR-0040](../adr/0040-language-agnostic-evidence-policy.md). These are replayable
identity and measurement primitives, not the collector, generic policy integration
or certification assigned to tasks 3.x–4.x. No generic contract changes are needed.

## TS-01: original-source identity

`typescript-original-source/v1` uses the pinned TypeScript 6.0.2 parser to inventory
original UTF-8 source bytes. The generic `subject-identity/v1` retains component,
target, boundary, full relative path, original-file SHA-256, kind, discriminator
and original span. Columns are UTF-16 code units, converted from zero-based parser
columns to the generic model's one-based columns; spans are end-exclusive.

Methods use lexical class/module ownership and the original name; anonymous
functions use their actual AST kind and unique source span. Istanbul's anonymous
labels are never identities. Native declaration coordinates must join exactly
one parser entry, the mapped body start must lie between that declaration and
the AST body, and the ending source line must agree. Known end columns must agree;
missing native end columns are resolved only from the parser's original span.
Unmatched, repeated or incomplete joins fail. Reduced identity is not supported.

The real fixtures expose three `quote` methods (`Pricing`, `StandardQuote`,
`PriorityQuote`), two nested anonymous arrows on one route line, erased type
annotations and null native end columns. All produce distinct original subjects.
Parser spans, rather than generated or reversed Istanbul statement end ranges,
provide subject identity. The accepted parser forms are methods, function
declarations/expressions and arrows with bodies. Other forms fail if they cannot
join; no constructor/accessor/overload support is claimed.

## TS-02: transformation provenance and boundaries

The replay caller supplies a trusted receipt pinning the complete native archive
and parser-index digests, plus the complete source snapshot of the requested
revision. The receipt must come from the caller/reviewed fixture, not from an
untrusted report. Rehashing an attacker's own archive does not authorize it.
The native manifest must be complete, every command successful, and its full
artifact inventory and configuration digests exact. Sources must match the
requested snapshot byte for byte. Archive links, duplicate artifact names,
duplicate JSON fields and path escapes are rejected without filesystem extraction.

Coverage absolute paths are translated only through the receipt's exact collection
root into full original paths. Basename, suffix and unknown-file joins fail.
Each accepted file requires exactly one retained **test** source map and its
linked emitted JavaScript. Regular v3 maps require original contents for every
source, canonical paths, valid VLQ segments, bounded generated/original UTF-16
coordinates, valid source/name indices and nonempty mappings for every source.
Every measured statement start and function declaration must appear in the map.
Both emitted bytes and map bytes remain bound by the archive digest. Production
maps cannot substitute for test instrumentation maps.

A bundle may contain multiple original files (for example the client wrapper and
generated imports). Only a separately mapped original TypeScript file can become
a subject. A multi-source *measurement* does not become a synthetic subject.
Template-bearing components, external templates and generated-client coverage
remain unsupported. Real `App_Template` coverage has reversed ranges and missing
columns; it is not TypeScript method evidence. The native `app.config.ts` report
has no retained test map and therefore fails closed. No claim is made that the
entire native report is acceptable. A generic multi-source contract would need a
separately reviewed amendment; this change does not introduce one.

## Exact counters and capability matrix

| Scope / capability | State and semantics |
| --- | --- |
| Mapped TypeScript file line coverage | `supported`: unique native statement-start lines; maximum hit count for statements sharing a line; covered means hits > 0 |
| Mapped TypeScript file function coverage | `supported`: exact native function hits > 0 / native function counter count; all counters must join parser entries |
| Mapped function/method line coverage | `supported`: same native line rule restricted to statement starts in that AST body; nested function statements belong to the innermost body |
| Mapped function/method function coverage | `supported`: that native function counter > 0 / 1 |
| Empty line/function denominator | `not_applicable`, no numeric value; never 100% |
| Template/generated multi-source coverage | `unsupported`, no numeric value or fabricated subject |
| Branch, cyclomatic/cognitive complexity, CRAP | `unsupported`, no numeric value |
| Missing collection/provenance, invalid counters, ambiguous identity | Measurement fails closed before returning rows; the future collector must retain the corresponding unavailable/failure state |

Counts stay integers; percentages are not normalization inputs. Negative,
fractional, Boolean and missing counters fail. Branch artifacts remain retained,
but no branch coverage is claimed. Native reversed statement **end** ranges do
not affect Istanbul's start-line rule. They cannot be used as source spans.
The pricing fixture is 4/6 covered file lines and 3/5 method lines, with 1/1
function coverage at both scopes. Thus file coverage cannot replace method coverage.

## Measurement series and baseline compatibility

The existing generic series schema fingerprints the complete TypeScript semantic
manifest through its versioned tool/rule fields: ecosystem; compiler; production
and test builders; Angular/CLI; runner; provider; Node/npm/DOM; recorded environment;
configuration digests (including lockfile, tsconfig and test/build configuration);
source-identity, mapping and rule versions; runtime target; source boundary; and
normalization semantics. Normalization is
`istanbul-start-line-max-innermost/v1`. Configuration hashes make compatibility
conservative: even incidental configuration changes require explicit acceptance.

`harness_evidence.require_compatible_series` accepts identical series only. Tests
change each tool/environment/configuration field, target, boundary, mapping and
normalization, and compare a Rust series: all fail with explicit migration
required. No baseline is created, accepted or migrated here. Digest/span-changing
subjects still require the existing explicit lineage and debt rules; name/path
similarity cannot authorize reuse. Full policy/ratchet integration remains 3.2.

## Reproduction and evidence

- [Semantic implementation](https://github.com/musutrade/Harness-Gate/blob/8d93ea53c7cf8b314bda816eafe13209da25c742/tools/quality/typescript_semantics.py)
- [Pinned parser, native oracle and reproduction commands](../../tools/quality/fixtures/typescript-angular/semantics/README.md)
- [Native collection](../../tools/quality/fixtures/typescript-angular/evidence/README.md)
- [Adversarial and native replay tests](../../tools/quality/tests/test_typescript_semantics.py)
- [GH-130 validation](gh-130/README.md)
- [OpenSpec decisions and remaining tasks](../../openspec/changes/typescript-angular-reference-adapter/tasks.md)
