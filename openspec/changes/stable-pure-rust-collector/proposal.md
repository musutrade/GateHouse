# Stable, pure-Rust collector delivery

User-approved normative policy delta: remove unstable compiler interfaces from
first-party plugin releases and required CI, implement the Rust collector entirely
in Rust, and publish our code rather than a private development environment.
External dependencies are user-installed and checked through documented capabilities.

This supersedes the delivery architecture of the legacy compiler-private/Python
collector. Host compatibility PR #258 alone does not implement this replacement.
Retain history and immutable releases. Do not continue publishing the old architecture
as the answer to this requirement. Repository Core's existing stable CI remains required.

See Engineering Policy section 10 for durable requirements. All Core decision,
threshold, requiredness, trust and baseline rules remain unchanged except for the
explicitly reviewed measurement-series transition described by this change.
