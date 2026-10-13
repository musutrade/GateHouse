# Generic policy example

`policy.json` evaluates the existing polyglot retained evidence using frontend
coverage and CRAP rules. Coverage passes and CRAP fails, intentionally returning
exit code 1. See the [contract and command](../../../../docs/quality/policy-engine.md).
The focused test suite extends the synthetic normalized facts in memory for
mutation and breaking-change comparisons; no real collector support is claimed.
