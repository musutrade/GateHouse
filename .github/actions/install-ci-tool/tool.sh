#!/usr/bin/env bash
set -euo pipefail

mode=${1:?expected resolve or verify}
tool=${2:?expected tool name}
case "$tool" in
  cargo-nextest) version=0.9.143 ;;
  cargo-llvm-cov) version=0.9.0 ;;
  cargo-audit) version=0.22.2 ;;
  cargo-tarpaulin) version=0.37.5 ;;
  *) echo "::error::Unsupported CI tool: $tool" >&2; exit 1 ;;
esac

case "$mode" in
  resolve) printf 'spec=%s@%s\n' "$tool" "$version" ;;
  verify)
    # Invoke through Cargo exactly as the consuming gates do, including PATH lookup.
    output=$(cargo "${tool#cargo-}" --version)
    printf '%s\n' "$output"
    read -r name actual rest <<< "$output"
    # These pinned releases repeat their subcommand in Cargo's version output.
    expected_name=$tool
    case "$tool" in
      cargo-audit) expected_name=cargo-audit-audit ;;
      cargo-tarpaulin) expected_name=cargo-tarpaulin-tarpaulin ;;
    esac
    if [[ "$actual" != "$version" ]] ||
       [[ "$name" != "$tool" && "$name" != "$expected_name" ]]; then
      echo "::error::Expected $tool $version; installed executable reported: $output" >&2
      exit 1
    fi
    ;;
  *) echo "::error::Unsupported CI tool operation: $mode" >&2; exit 1 ;;
esac
