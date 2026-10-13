# Security Policy

## Supported Versions

Security fixes are applied to the latest published release and to `main`.
Older tags are not guaranteed to receive fixes; upgrade before opening a
compatibility report.

## Reporting A Vulnerability

Please use the repository's private GitHub Security Advisory workflow:

<https://github.com/musutrade/GateHouse/security/advisories/new>

Do not open a public issue for an unpatched vulnerability. Include the affected
version or commit, operating system, a minimal reproduction, impact, and any
mitigation. Remove credentials, tokens, private keys, and customer data from
the report. The maintainers will acknowledge a report within seven days and
will coordinate a fix, disclosure, and release timeline through the private
advisory.

Harness-Gate's secret scan and report redaction are defense-in-depth checks.
They do not replace dedicated secret scanners, host isolation, or a review of
the permissions granted to CI jobs and webhook destinations.
