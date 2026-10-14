#!/usr/bin/env bash
# Check presence only; registry authorization is still enforced by Cargo.
set +x
set -euo pipefail

registry_token="${CARGO_REGISTRY_TOKEN:-}"
if [[ -z "${registry_token//[[:space:]]/}" ]]; then
  printf '%s\n' '::error::CARGO_REGISTRY_TOKEN is missing or blank. Configure a credential with harness-gate publish permission in the release environment or repository Actions secrets before publishing.' >&2
  exit 1
fi
