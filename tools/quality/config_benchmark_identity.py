"""Declared build-input provenance for the opt-in configuration benchmark."""
import os
from pathlib import Path
import shutil
import subprocess
import tomllib

from quality_common import sha256


def build_identity(root: Path, environment: dict[str, str]) -> dict:
    explicit = {
        "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_RUSTFLAGS", "CARGO_BUILD_TARGET",
        "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
        "CARGO_INCREMENTAL", "CARGO_BUILD_INCREMENTAL",
    }
    prohibited = sorted(name for name in environment if name in explicit or name.startswith("CARGO_PROFILE_")
                        or (name.startswith("CARGO_TARGET_") and name.endswith(("_RUSTFLAGS", "_LINKER", "_RUNNER"))))
    if prohibited:
        raise ValueError("unset build-control overrides for this benchmark: " + ", ".join(prohibited))
    config_paths = [parent / ".cargo" / name for parent in (root, *root.parents)
                    for name in ("config", "config.toml")]
    cargo_home = Path(environment.get("CARGO_HOME", str(Path.home() / ".cargo")))
    if not cargo_home.is_absolute():
        cargo_home = root / cargo_home
    config_paths += [cargo_home / name for name in ("config", "config.toml")]
    configs = {str(p.resolve()): sha256(p) for p in config_paths if p.is_file()}
    for name in configs:
        config = tomllib.loads(Path(name).read_text())
        unsupported = set(config) & {"target", "profile", "env", "unstable"}
        build_keys = set(config.get("build", {})) - {"jobs", "target-dir"}
        if unsupported or build_keys:
            raise ValueError("unsupported build-altering Cargo configuration: " + name)
    verbose = subprocess.check_output(["rustc", "-vV"], cwd=root, env=environment, text=True)
    targets = [line.removeprefix("host: ") for line in verbose.splitlines() if line.startswith("host: ")]
    if len(targets) != 1:
        raise ValueError("rustc did not identify one host target")
    executables = {}
    for name in ("cargo", "rustc"):
        executable = shutil.which(name, path=environment.get("PATH"))
        if not executable:
            raise ValueError(f"missing executable: {name}")
        executables[name] = {"path": executable, "sha256": sha256(Path(executable))}
    return {"explicit_target": targets[0], "rustc_verbose": verbose,
            "executables": executables, "cargo_config_sha256": configs,
            "rustup_toolchain": environment.get("RUSTUP_TOOLCHAIN"),
            "scope": "Declared input identity; config hashes are provenance, not resolved flags. Compare only matching configuration identities; test harness is not the shipped binary."}
