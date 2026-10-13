"""Guard the open ecosystem boundary of generic workflow/report composition."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[3]
VERIFY = ROOT / "tools/harness-gate/src/verify"
ECOSYSTEM_SWITCH = re.compile(r'\bmatch\s+[^{};]*\b\w*(?:ecosystem|language)\w*\b[^{};]*\{', re.I)


class VerifyQualityArchitectureTests(unittest.TestCase):
    def test_generic_verify_and_reporting_have_no_closed_ecosystem_dispatch(self):
        # Literal dispatch and enum variants are both forbidden here. Ecosystem
        # identifiers may flow through JSON/metadata without host interpretation.
        language_literal = re.compile(r'"(?:rust|angular|go|vue|java|react|typescript|javascript|python)"', re.I)
        closed_enum = re.compile(r'\benum\s+\w*(?:Ecosystem|Language)\w*\s*\{', re.I)
        ecosystem_variant = re.compile(r'\b(?:Ecosystem|Language)\w*::\w+')
        language_identifier = re.compile(r'\b(?:Rust|Angular|Go|Vue|Java|React|TypeScript|JavaScript|Python)\b')
        paths = sorted(VERIFY.rglob("*.rs")) + sorted(
            (ROOT / "tools/harness-gate/src/config/quality").rglob("*.rs")) + [
            ROOT / "tools/harness-gate/quality-core/project_report.rs"] + [
            p for p in (ROOT / "tools/harness-gate/src/preset").glob("*.rs")
            if p.stem != "catalog"]
        for path in paths:
            if "tests" in path.parts or path.stem == "tests":
                continue
            source = path.read_text().split("#[cfg(test)]")[0]
            source = re.sub(r'//[^\n]*|/\*.*?\*/', '', source, flags=re.S)
            with self.subTest(path=path.relative_to(ROOT)):
                self.assertIsNone(language_literal.search(source))
                self.assertIsNone(closed_enum.search(source))
                self.assertIsNone(ecosystem_variant.search(source))
                self.assertIsNone(language_identifier.search(source))
                self.assertIsNone(ECOSYSTEM_SWITCH.search(source))

    def test_generic_preset_plumbing_does_not_own_quality_semantics(self):
        for name in ("composition", "initialize", "migration", "filesystem"):
            source = (ROOT / f"tools/harness-gate/src/preset/{name}.rs").read_text()
            with self.subTest(module=name):
                self.assertNotRegex(source, r'"(?:risk\.crap|coverage\.[a-z]+|contract\.[a-z_]+|required|ratchet|limit|supported|unsupported)"')
                self.assertNotRegex(source, r'(?i)rust-api|angular-only|angular-rust-postgres')

    def test_switch_guard_rejects_arbitrary_ecosystem_cases(self):
        for source in (
            'match metadata["ecosystem"].as_str() { "nebula" => approve() }',
            'match component.ecosystem { Custom => approve() }',
            'match component.ecosystem_id { "anything" => collect() }',
            'match language { SomethingNew => collect() }',
        ):
            with self.subTest(source=source):
                self.assertIsNotNone(ECOSYSTEM_SWITCH.search(source))


if __name__ == "__main__":
    unittest.main()
