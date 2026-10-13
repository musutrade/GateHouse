# Angular reference app

CLI-generated Angular 22.0.8 fixture. See the parent [fixture record](../README.md)
for the frozen toolchain, test boundaries and reproduction command. Run
`collect.py` from the repository root to start the required live Rust provider
and retain build/test/coverage evidence. A bare `npm test` intentionally fails
the provider test when `REFERENCE_PROVIDER_URL` is absent.
