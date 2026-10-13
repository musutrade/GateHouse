# Design

Implement M1–M4 as one bounded source-parser delivery. Visit parsed values through the existing AST visitor; do not manufacture spans or flatten closure ownership. Native source counters are not runtime complexity. Parsing success does not resolve names, procedural attributes, cfg or expansions.

Alternative: replace the default parser grammar. Rejected because that would silently change historical measurement semantics. The opt-in CLI emits a separate analyzer/rule identity and does not feed source_measure.py.

Rollback: stop invoking the optional flag; legacy mode and all production collectors remain available. Do not rewrite retained evidence or downgrade quality failures.

Implementation sequence: reproduce source failures, implement/test grammars, replay source and publish CI-reviewed PR. No deadline or complete native measurement claim is attached to this first stage.
