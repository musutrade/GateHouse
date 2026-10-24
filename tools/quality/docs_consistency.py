#!/usr/bin/env python3
"""Check Markdown links, embedded examples, generated Schema sync, and policy anchors."""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import tempfile
import unicodedata
from pathlib import Path

from quality_common import CRATE, ROOT, QUALITY_ROOT, fail, metadata, write_json


def local_links() -> list[dict[str, str]]:
    failures = []
    documents = [
        ROOT / "README.md",
        ROOT / "README.zh-CN.md",
        ROOT / "CONTRIBUTING.md",
        ROOT / "CODE_OF_CONDUCT.md",
        ROOT / "CHANGELOG.md",
        ROOT / "SECURITY.md",
        *ROOT.glob("docs/**/*.md"),
        *ROOT.glob("schema/*.md"),
    ]
    pattern = re.compile(r"\]\(([^)\s]+)")
    for document in documents:
        source = document.read_text(errors="replace")
        headings = {
            anchor(heading): heading
            for heading in re.findall(r"^#{1,6}\s+(.+?)\s*#*\s*$", source, flags=re.MULTILINE)
        }
        headings.update({identifier: identifier for identifier in re.findall(r"<a\s+id=[\"']([^\"']+)[\"']", source)})
        for target in pattern.findall(source):
            if target.startswith(("http://", "https://", "mailto:")):
                continue
            path, fragment = (target.split("#", 1) + [""])[:2] if "#" in target else (target, "")
            linked = document if not path else document.parent / path
            if path and not linked.exists():
                failures.append({"document": str(document.relative_to(ROOT)), "target": target})
            elif fragment:
                if linked.resolve() == document.resolve():
                    linked_headings = headings
                elif linked.is_file():
                    linked_headings = {
                        anchor(heading): heading
                        for heading in re.findall(r"^#{1,6}\s+(.+?)\s*#*\s*$", linked.read_text(errors="replace"), flags=re.MULTILINE)
                    }
                    linked_headings.update(
                        {
                            identifier: identifier
                            for identifier in re.findall(r"<a\s+id=[\"']([^\"']+)[\"']", linked.read_text(errors="replace"))
                        }
                    )
                else:
                    linked_headings = {}
                if fragment not in linked_headings and anchor(fragment) not in linked_headings:
                    failures.append({"document": str(document.relative_to(ROOT)), "target": target})
    return failures


SANDBOX_NEGATION_MARKERS = (
    "not", "no ", "never", "without", "deferred", "future", "separate", "rejected",
    "must not", "should not", "cannot", "can't", "isn't", "aren't", "does not", "do not",
    "not an", "unless", "until", "before", "out of scope", "non-goal", "would overstate",
    "later", "may", "might", "could",
)

SANDBOX_CLAIM_PATTERNS = (
    re.compile(
        r"\b(?:runs?|executes?|enforces?|provides?|offers?|guarantees?|gives?|uses?|claims?)\b"
        r"[^.!?]{0,120}\b(?:operating-system|os[-\s]?enforced|os[-\s]?level|kernel)?"
        r"[-\s]?(?:network|filesystem|resource|process)?[-\s]?sandbox(?:ed|ing)?\b",
        re.IGNORECASE,
    ),
    re.compile(r"\b(?:complete|full)\b[^.!?]{0,60}\bdescendant\s+(?:isolation|containment)\b", re.IGNORECASE),
    re.compile(
        r"\b(?:os|operating-system)[-\s]?(?:level|enforced)?[-\s]?"
        r"(?:network|filesystem|resource|process)\s+isolation\b",
        re.IGNORECASE,
    ),
)


def unsupported_sandbox_claims(text: str) -> list[str]:
    """Return positive OS-sandbox or complete-descendant-isolation claims."""
    compact = re.sub(r"\s+", " ", text)
    failures: list[str] = []
    for pattern in SANDBOX_CLAIM_PATTERNS:
        for match in pattern.finditer(compact):
            context = compact[max(0, match.start() - 160) : match.end() + 80]
            lowered = context.lower()
            if any(marker in lowered for marker in SANDBOX_NEGATION_MARKERS):
                continue
            failures.append(context.strip())
    return failures


def sandbox_wording_failures() -> list[dict[str, str]]:
    documents = [
        ROOT / "README.md", ROOT / "README.zh-CN.md", ROOT / "CONTRIBUTING.md",
        ROOT / "CODE_OF_CONDUCT.md", ROOT / "SECURITY.md", *ROOT.glob("docs/**/*.md"),
        *ROOT.glob("schema/*.md"), *ROOT.glob("openspec/changes/*/*.md"),
        *ROOT.glob("openspec/changes/*/specs/**/*.md"), *ROOT.glob("tools/harness-gate/src/**/*.rs"),
    ]
    failures = []
    for document in documents:
        try:
            source = document.read_text(errors="replace")
        except OSError:
            continue
        for claim in unsupported_sandbox_claims(source):
            failures.append({"document": str(document.relative_to(ROOT)), "claim": claim})
    return failures


def engineering_policy_failures() -> list[str]:
    """Guard stable normative anchors without duplicating the policy engine."""
    policy = ROOT / "docs" / "engineering-policy.md"
    contributing = ROOT / "CONTRIBUTING.md"
    if not policy.is_file():
        return ["missing docs/engineering-policy.md"]
    text = policy.read_text(errors="replace")
    required_phrases = (
        "Normative repository policy",
        "Collectors measure and normalize facts",
        "released Rust Harness-Gate core",
        "CRAP `<= 30`",
        "line coverage `>= 80%`",
        "region coverage `>= 80%`",
        "new debt is forbidden",
        "converted to PASS",
        "normative policy delta",
        "CI performance regressions are engineering regressions",
        "Application- and repository-specific validation logic belongs to the project",
        "Command hooks / execution gates",
        "Structured result adapters",
        "Quality collector plugins",
        "A new application validation tool or framework must not require",
        "ADR-0049",
        "Officially released plugins and required CI gates MUST NOT depend on unstable compiler APIs",
        "first-party Rust quality collector MUST be implemented entirely in Rust",
        "installed runtime MUST NOT require Python",
        "Source availability alone MUST NOT be treated as proof of measurement accuracy",
        "A macro or derive invocation alone MUST NOT justify blanket unsupported classification",
        "prefer an upstream PR",

        "Users install the documented external Rust/Cargo/coverage and project-native dependencies",
        "New signing/publication of the legacy Python/compiler-private Rust collector and its installer is suspended",

    )
    failures = [f"engineering policy missing normative anchor: {phrase}" for phrase in required_phrases if phrase not in text]
    try:
        contributing_text = contributing.read_text(errors="replace")
    except OSError:
        failures.append("missing CONTRIBUTING.md")
    else:
        if "docs/engineering-policy.md" not in contributing_text:
            failures.append("CONTRIBUTING.md does not reference docs/engineering-policy.md")
    return failures


def anchor(value: str) -> str:
    value = unicodedata.normalize("NFKD", value).lower()
    value = re.sub(r"[^\w\s-]", "", value, flags=re.UNICODE)
    return re.sub(r"[-\s]+", "-", value).strip("-")


def executable_checks() -> dict:
    """Checks that execute the locked CLI: examples, migration and schema sync."""
    command = ["cargo", "run", "--quiet", "--locked", "--manifest-path",
               str(CRATE / "Cargo.toml"), "--"]
    examples = []
    for preset in sorted([*(CRATE / "presets").glob("*.flow.toml"),
                          *(CRATE / "fixtures/preset-compat").glob("*.flow.toml")], key=lambda path: path.name):
        with tempfile.TemporaryDirectory(prefix="harness-gate-example-") as directory:
            root = Path(directory)
            preset_name = preset.name.removesuffix(".flow.toml")
            initialized = subprocess.run(
                [*command, "init", "--project-root", str(root), "--preset", preset_name,
                 "--presets-dir", str(CRATE / "fixtures/preset-compat")],
                cwd=ROOT, capture_output=True, text=True,
            )
            if 'external_value_policy' in preset.read_text():
                checked = (initialized.returncode != 0
                           and 'gateprobes-postgres migrate' in initialized.stderr
                           and not (root / '.harness-gate/flow.toml').exists())
                examples.append({'path': str(preset.relative_to(ROOT)),
                                 'quality_generated': (root / '.harness-gate/quality.toml').is_file(),
                                 'behavior': 'actionable legacy rejection',
                                 'status': 'pass' if checked else 'fail'})
                continue
            checked = initialized.returncode == 0 and subprocess.run(
                [*command, "config", "check", "--project-root", str(root)],
                cwd=ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            ).returncode == 0
            quality_expected = preset.with_name(preset_name + ".packs.json").is_file()
            quality_generated = (root / ".harness-gate/quality.toml").is_file()
            checked = checked and quality_generated == quality_expected
            examples.append({"path": str(preset.relative_to(ROOT)),
                             "quality_generated": quality_generated,
                             "status": "pass" if checked else "fail"})
    migration_fixture = ROOT / "tools" / "harness-gate" / "fixtures" / "v1-flow.toml"
    with tempfile.TemporaryDirectory(prefix="harness-gate-migration-") as directory:
        root = Path(directory)
        shutil.copy2(migration_fixture, root / "legacy.flow.toml")
        (root / ".harness-gate").mkdir()
        migration = subprocess.run(
            [*command, "--project-root", str(root), "config", "migrate", "--input", "legacy.flow.toml", "--output", ".harness-gate/flow.toml"],
            cwd=ROOT, capture_output=True, text=True,
        )
        migration_checked = (migration.returncode != 0
                             and 'gateprobes-postgres migrate' in migration.stderr
                             and (root / 'legacy.flow.toml').read_bytes() == migration_fixture.read_bytes()
                             and not any((root / '.harness-gate').iterdir()))
    workflow_fixture = ROOT / "tools" / "harness-gate" / "fixtures" / "workflow"
    with tempfile.TemporaryDirectory(prefix="harness-gate-quality-example-") as directory:
        root = Path(directory)
        config = root / ".harness-gate"
        config.mkdir()
        shutil.copy2(CRATE / "fixtures/preset-compat/rust-api.flow.toml", config / "flow.toml")
        shutil.copy2(CRATE / "presets/empty.audit.toml", config / "audit.toml")
        shutil.copy2(CRATE / "presets/default.secrets.toml", config / "secrets.toml")
        for filename in ("quality.toml", "policy.json"):
            shutil.copy2(workflow_fixture / filename, config / filename)
        quality_checked = subprocess.run(
            [*command, "config", "check", "--project-root", str(root)],
            cwd=ROOT, capture_output=True, text=True,
        ).returncode == 0
    schemas_synced = {}
    with tempfile.TemporaryDirectory(prefix="harness-gate-schema-") as directory:
        schema_root = Path(directory)
        for plane in ("flow", "quality"):
            filename = f"{plane}.schema.json"
            args = ["--quality"] if plane == "quality" else []
            schema = subprocess.run(
                [*command, "--project-root", str(schema_root), "schema", "export", *args, "--output", filename],
                cwd=ROOT, capture_output=True, text=True,
            )
            generated = schema_root / filename
            committed = ROOT / "schema" / filename
            schemas_synced[plane] = schema.returncode == 0 and generated.is_file() and generated.read_bytes() == committed.read_bytes()
    return {"examples": examples,
            "migration_path": str(migration_fixture.relative_to(ROOT)),
            "migration_checked": migration_checked,
            "schema_synced": schemas_synced["flow"],
            "quality_schema_synced": schemas_synced["quality"],
            "quality_checked": quality_checked}


def _skip_space_and_comments(text: str, index: int) -> int:
    while index < len(text):
        if text[index].isspace():
            index += 1
        elif text.startswith("//", index):
            newline = text.find("\n", index)
            index = len(text) if newline < 0 else newline + 1
        elif text.startswith("/*", index):
            close = text.find("*/", index + 2)
            if close < 0:
                raise ValueError("unterminated block comment in CONFIG_CODES")
            index = close + 2
        else:
            break
    return index


def _parse_rust_string(text: str, index: int) -> tuple[str, int]:
    if index >= len(text) or text[index] != '"':
        raise ValueError(f"expected a string literal in CONFIG_CODES near {text[index:index + 40]!r}")
    index += 1
    value = []
    while index < len(text):
        character = text[index]
        if character == "\\":
            if index + 1 >= len(text):
                raise ValueError("unterminated escape in CONFIG_CODES string")
            value.append(text[index:index + 2])
            index += 2
        elif character == '"':
            return "".join(value), index + 1
        elif character == "\n":
            raise ValueError("newline inside a CONFIG_CODES string literal")
        else:
            value.append(character)
            index += 1
    raise ValueError("unterminated string literal in CONFIG_CODES")


def parse_config_code_enum(source: str) -> tuple[list[str], list[str]]:
    """Parse the simple unit-variant syntax accepted for ConfigCode.

    The parser is deliberately fail-closed: attributes, field variants, inline
    values other than a decimal discriminant, trailing comments and any other
    unaccepted syntax are reported instead of being skipped.
    """
    lines = source.splitlines()
    start = next((index for index, line in enumerate(lines)
                  if re.search(r"\bpub\(crate\)\s+enum\s+ConfigCode\s*\{", line)), None)
    if start is None:
        return [], ["cannot find the ConfigCode enum"]
    variants: list[str] = []
    failures: list[str] = []
    closed = False
    for index in range(start + 1, len(lines)):
        line = lines[index].strip()
        if not line or line.startswith("//"):
            continue
        if line == "}":
            closed = True
            break
        if line.startswith("#"):
            failures.append(f"unsupported attribute or inline configuration on ConfigCode line {index + 1}: {line!r}")
            continue
        match = re.fullmatch(r"([A-Z]\w*)\s*(?:=\s*([0-9]+))?\s*,?", line)
        if match is None:
            failures.append(f"unsupported ConfigCode variant syntax on line {index + 1}: {line!r}")
            continue
        name = match.group(1)
        if name in variants:
            failures.append(f"duplicate ConfigCode variant {name}")
        variants.append(name)
    if not closed:
        failures.append("ConfigCode enum has no closing brace")
    if not variants:
        failures.append("ConfigCode enum has no variants")
    return variants, failures


def parse_config_codes_table(source: str) -> tuple[list[dict[str, str]], list[str]]:
    """Parse CONFIG_CODES entries and consume every non-comment character."""
    lines = source.splitlines()
    start = next((index for index, line in enumerate(lines)
                  if re.search(r"\bconst\s+CONFIG_CODES\s*:", line)), None)
    if start is None:
        return [], ["cannot find the CONFIG_CODES registry"]
    end = next((index for index in range(start + 1, len(lines)) if lines[index].strip() == "];"), None)
    if end is None:
        return [], ["CONFIG_CODES registry has no closing ];"]
    body = "\n".join(lines[start + 1:end])
    entries: list[dict[str, str]] = []
    failures: list[str] = []
    index = 0
    try:
        while True:
            index = _skip_space_and_comments(body, index)
            if index >= len(body):
                break
            if not body.startswith("ConfigCodeEntry", index):
                failures.append(f"unsupported CONFIG_CODES content near {body[index:index + 60]!r}")
                break
            index = _skip_space_and_comments(body, index + len("ConfigCodeEntry"))
            if index >= len(body) or body[index] != "{":
                failures.append("CONFIG_CODES entry has no opening brace")
                break
            index += 1
            fields: dict[str, str] = {}
            while True:
                index = _skip_space_and_comments(body, index)
                if index >= len(body):
                    failures.append("unterminated CONFIG_CODES entry")
                    break
                if body[index] == "}":
                    index += 1
                    break
                key_start = index
                while index < len(body) and (body[index].isalnum() or body[index] == "_"):
                    index += 1
                key = body[key_start:index]
                if not key:
                    failures.append(f"unsupported CONFIG_CODES field syntax near {body[index:index + 40]!r}")
                    break
                index = _skip_space_and_comments(body, index)
                if index >= len(body) or body[index] != ":":
                    failures.append(f"CONFIG_CODES field {key!r} has no colon")
                    break
                index = _skip_space_and_comments(body, index + 1)
                if key == "code":
                    prefix = "ConfigCode::"
                    if not body.startswith(prefix, index):
                        failures.append("CONFIG_CODES code must be ConfigCode::<variant>")
                        break
                    index += len(prefix)
                    name_start = index
                    while index < len(body) and (body[index].isalnum() or body[index] == "_"):
                        index += 1
                    value = body[name_start:index]
                    if not re.fullmatch(r"[A-Z]\w*", value):
                        failures.append(f"unsupported ConfigCode reference {value!r} in CONFIG_CODES")
                        break
                elif key in ("id", "title", "help"):
                    value, index = _parse_rust_string(body, index)
                    if key == "id" and not re.fullmatch(r"[A-Z0-9-]+", value):
                        failures.append(f"invalid configuration diagnostic ID {value!r}")
                else:
                    failures.append(f"unsupported CONFIG_CODES field {key!r}")
                    break
                if key in fields:
                    failures.append(f"duplicate CONFIG_CODES field {key!r}")
                    break
                fields[key] = value
                index = _skip_space_and_comments(body, index)
                if index < len(body) and body[index] == ",":
                    index += 1
                elif index < len(body) and body[index] == "}":
                    index += 1
                    break
                else:
                    failures.append(f"CONFIG_CODES field {key!r} must end with a comma")
                    break
            if failures:
                break
            missing = [key for key in ("code", "id", "title", "help") if key not in fields]
            if missing:
                failures.append(f"CONFIG_CODES entry is missing fields {missing}")
                break
            entries.append(fields)
            index = _skip_space_and_comments(body, index)
            if index < len(body):
                if body[index] != ",":
                    failures.append(f"CONFIG_CODES entries must be comma separated near {body[index:index + 40]!r}")
                    break
                index += 1
        if not failures and not entries:
            failures.append("CONFIG_CODES registry has no entries")
    except ValueError as error:
        failures.append(str(error))
    return entries, failures


def diagnostic_registry_failures(source: str, documentation: str, crate: Path) -> list[str]:
    """Bind ConfigCode variants to the registry, explanations and documentation."""
    variants, failures = parse_config_code_enum(source)
    entries, table_failures = parse_config_codes_table(source)
    failures.extend(table_failures)
    codes = [entry["code"] for entry in entries]
    ids = [entry["id"] for entry in entries]
    if set(variants) != set(codes):
        failures.append(f"registry variants differ: enum={sorted(set(variants) - set(codes))}, table={sorted(set(codes) - set(variants))}")
    if len(codes) != len(set(codes)):
        failures.append("duplicate ConfigCode entries in CONFIG_CODES")
    if len(ids) != len(set(ids)):
        failures.append("duplicate configuration diagnostic IDs")
    lines = set(documentation.splitlines())
    for entry in entries:
        code, code_id = entry["code"], entry["id"]
        heading = f"## {code_id.lower()}"
        if heading not in lines:
            failures.append(f"{code}: missing independent documentation heading {heading!r}")
        if f"**{code_id} — " not in documentation:
            failures.append(f"{code}: missing documentation entry body for {code_id}")
        if not (crate / "docs" / "errors" / f"{code_id}.md").is_file():
            failures.append(f"{code}: missing detailed guide docs/errors/{code_id}.md")
    return failures


def diagnostic_code_failures() -> list[str]:
    """Check the checked-in registry and documentation; read errors propagate."""
    return diagnostic_registry_failures(
        (CRATE / "src" / "config" / "diagnostic.rs").read_text(),
        (ROOT / "docs" / "failure-codes.md").read_text(),
        CRATE,
    )


def run(output: Path, pure: bool = False) -> int:
    link_failures = local_links()
    sandbox_failures = sandbox_wording_failures()
    policy_failures = engineering_policy_failures()
    try:
        diagnostic_failures = diagnostic_code_failures()
    except OSError as error:
        diagnostic_failures = [f"cannot read the diagnostic registry or documentation: {error}"]
    english_config = ROOT / "docs" / "configuration.md"
    chinese_config = ROOT / "docs" / "configuration.zh-CN.md"
    schema_catalog = ROOT / "schema" / "README.md"
    language_docs_valid = (
        english_config.is_file() and chinese_config.is_file() and schema_catalog.is_file()
        and "# harness-gate schema v2 configuration reference" in english_config.read_text(errors="replace").lower()
    )
    if pure:
        executable = {"examples": [], "migration_path": "tools/harness-gate/fixtures/v1-flow.toml",
                      "migration_checked": None, "schema_synced": None,
                      "quality_schema_synced": None, "quality_checked": None}
        executable_ok = True
        migration_status = "skipped"
    else:
        executable = executable_checks()
        executable_ok = (executable["schema_synced"] and executable["quality_schema_synced"]
                         and executable["quality_checked"] and executable["migration_checked"]
                         and all(item["status"] == "pass" for item in executable["examples"]))
        migration_status = "pass" if executable["migration_checked"] else "fail"
    machine_schema = ROOT / "schema" / "machine-result.schema.json"
    machine_schema_valid = False
    try:
        machine_data = json.loads(machine_schema.read_text())
        required = set(machine_data.get("required", []))
        machine_schema_valid = machine_data.get("properties", {}).get("schema_version", {}).get("const") == "1" and {
            "schema_version", "input_mode", "project_identity", "source_identity", "execution_root",
            "configuration_digest", "scope", "services", "steps", "skipped_steps", "warnings",
            "failures", "artifacts", "evidence_complete", "status",
        } <= required
    except (OSError, json.JSONDecodeError, TypeError):
        machine_schema_valid = False
    manifest_schema = ROOT / "schema" / "artifact-manifest.schema.json"
    manifest_schema_valid = False
    try:
        manifest_data = json.loads(manifest_schema.read_text())
        manifest_required = set(manifest_data.get("required", []))
        artifact_required = set(manifest_data.get("definitions", {}).get("artifact", {}).get("required", []))
        manifest_schema_valid = manifest_data.get("properties", {}).get("schema_version", {}).get("const") == "1" and {"schema_version", "invocation_id", "generated_at", "artifacts"} <= manifest_required and {"path", "kind", "size_bytes", "sha256"} <= artifact_required
    except (OSError, json.JSONDecodeError, TypeError):
        manifest_schema_valid = False
    registry_schema = ROOT / "schema" / "artifact-registry.schema.json"
    registry_schema_valid = False
    try:
        registry_data = json.loads(registry_schema.read_text())
        registry_required = set(registry_data.get("required", []))
        registry_artifact_required = set(registry_data.get("definitions", {}).get("artifact", {}).get("required", []))
        registry_schema_valid = registry_data.get("properties", {}).get("schema_version", {}).get("const") == "1" and {"schema_version", "invocation_id", "artifacts"} <= registry_required and {"invocation_id", "path", "kind", "size_bytes", "sha256"} <= registry_artifact_required
    except (OSError, json.JSONDecodeError, TypeError):
        registry_schema_valid = False
    sandbox_wording = {"failures": sandbox_failures, "status": "fail" if sandbox_failures else "pass"}
    engineering_policy = {"failures": policy_failures, "status": "fail" if policy_failures else "pass"}
    diagnostic_codes = {"failures": diagnostic_failures,
                        "status": "fail" if diagnostic_failures else "pass"}
    status_pass = (not link_failures and language_docs_valid and executable_ok
                   and machine_schema_valid and manifest_schema_valid and registry_schema_valid
                   and sandbox_wording["status"] == "pass" and engineering_policy["status"] == "pass"
                   and diagnostic_codes["status"] == "pass")
    result = {
        **metadata(tool="docs-consistency"), "mode": "pure" if pure else "full",
        "link_failures": link_failures, "examples": executable["examples"],
        "migration": {"path": executable["migration_path"], "status": migration_status},
        "language_docs_valid": language_docs_valid, "schema_synced": executable["schema_synced"],
        "quality_schema_synced": executable["quality_schema_synced"],
        "quality_example_valid": executable["quality_checked"],
        "machine_schema_valid": machine_schema_valid, "manifest_schema_valid": manifest_schema_valid,
        "registry_schema_valid": registry_schema_valid, "sandbox_wording": sandbox_wording,
        "engineering_policy": engineering_policy, "diagnostic_codes": diagnostic_codes,
        "status": "pass" if status_pass else "fail",
    }
    write_json(output, result)
    if result["status"] != "pass":
        if sandbox_failures:
            fail("unsupported OS-sandbox or descendant-isolation wording found")
        if policy_failures:
            fail("normative engineering policy anchors are missing or disconnected")
        if diagnostic_failures:
            fail("configuration diagnostic registry/documentation check failed: "
                 + "; ".join(diagnostic_failures))
        fail("documentation, examples, or schema synchronization failed")
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=QUALITY_ROOT / "docs-consistency.json")
    parser.add_argument("--pure", action="store_true",
                        help="run link/anchor/wording checks only; never invokes Cargo")
    args = parser.parse_args()
    raise SystemExit(run(args.output, pure=args.pure))
