# Honor project CRAP policy in the standalone native plugin

The direct native `evaluate` bridge replaces the project's CRAP ceiling with 30.
This contradicts the existing Core/project policy ownership boundary. Resolve an
explicit quality binding and pass its exact rational limit to Core.

Engineering-policy intent is unchanged: project policies own thresholds; Core
owns outcomes, debt and non-regression; collectors own measurement facts. The
repository's default 30, coverage 80%, changed CC > 10, requiredness, evidence
failure rules and native formula/driver remain unchanged. The integration delta
is intentional: installed projects can select a different CRAP ceiling using
the existing policy format. No compatibility fallback for unconfigured native
evaluation is retained, as directed by the user.

Require project-root and binding arguments; preserve the selected configuration
bytes/hashes and Core validation output. Reject invalid, missing, ambiguous or
incompatible inputs. Do not certify the entire project profile from a single
native capture. Existing raw anchors and normalized baselines remain intact;
the changed projection source has its actual identity and must be reviewed as
such, with both raw baseline/head captures projected together.

Validate through actual packaged binaries and Core: ceiling changes on identical
facts, inclusive/fractional boundaries, ceilings above 30, unchanged debt,
changed debt, regression even below the configured ceiling, and fail-closed
configuration/series/scope checks. Publish through the existing four-host release
workflow after required CI, with external tools and immutable releases.
