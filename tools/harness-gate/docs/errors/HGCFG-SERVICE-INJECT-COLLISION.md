## Why this fails

A service injection variable conflicts with another use of that name. This can
occur between services in one step, between a service and that step's
`runner.threads_env`, or between independent services used by unordered steps.
The diagnostic path and related entries distinguish these cases.

## Diagnose and repair

For services in one step, select the intended service or assign distinct
`inject_env` names and update their consumers. For a runner collision, give
`runner.threads_env` a distinct variable name and update the runner configuration
that consumes it. Using only one service does not resolve a runner collision.

For a conflict across unordered steps, add the intended explicit dependency,
use distinct injection names or split the workflow. Ordering declarations in
the file does not establish a dependency. Run `harness-gate config check` after
the repair; other configuration constraints must still pass.

## Example

A step can use a single service that injects DATABASE_URL and still fail if
its runner also uses DATABASE_URL for the thread count. Keep the service URL
and the numeric thread setting in separate variables. If the conflict instead
involves two independent steps, use the related locations to identify both
service declarations before choosing an ordering or naming repair.
