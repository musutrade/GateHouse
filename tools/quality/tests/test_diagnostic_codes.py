from __future__ import annotations

from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import docs_consistency as gate

SOURCE = gate.CRATE / "src" / "config" / "diagnostic.rs"
DOCUMENTATION = gate.ROOT / "docs" / "failure-codes.md"
CLOSING_VARIANT = "    HgpluginVersion,\n}"


class DiagnosticRegistryParserTests(unittest.TestCase):
    def setUp(self):
        self.source = SOURCE.read_text()
        self.documentation = DOCUMENTATION.read_text()

    def failures(self, source=None):
        return gate.diagnostic_registry_failures(
            self.source if source is None else source, self.documentation, gate.CRATE
        )

    def test_checked_in_registry_and_documentation_pass(self):
        self.assertEqual(self.failures(), [])

    def test_plain_variant_without_table_entry_fails(self):
        source = self.source.replace(CLOSING_VARIANT, "    HgpluginVersion,\n    HgcfgProbeGap,\n}")
        failures = self.failures(source)
        self.assertTrue(any("registry variants differ" in failure for failure in failures), failures)
        self.assertTrue(any("HgcfgProbeGap" in failure for failure in failures), failures)

    def test_explicit_discriminant_without_table_entry_fails(self):
        source = self.source.replace(CLOSING_VARIANT, "    HgpluginVersion,\n    HgcfgProbeGap = 100,\n}")
        failures = self.failures(source)
        self.assertTrue(any("registry variants differ" in failure for failure in failures), failures)
        self.assertTrue(any("HgcfgProbeGap" in failure for failure in failures), failures)

    def test_supported_explicit_discriminant_and_table_field_parse(self):
        source = self.source.replace(CLOSING_VARIANT, "    HgpluginVersion,\n    HgcfgProbeGap = 100,\n}")
        variants, failures = gate.parse_config_code_enum(source)
        self.assertEqual(failures, [])
        self.assertIn("HgcfgProbeGap", variants)
        entries, failures = gate.parse_config_codes_table(source)
        self.assertEqual(failures, [])
        self.assertIn("HgpluginVersion", [entry["code"] for entry in entries])

    def test_comment_pseudo_declarations_are_not_registry_starts(self):
        commented = (
            "// pub(crate) enum ConfigCode {\n"
            "/*\n"
            "pub(crate) enum ConfigCode {\n"
            "*/\n"
            "// const CONFIG_CODES: &[ConfigCodeEntry] = &[\n"
            + self.source
        )
        self.assertEqual(self.failures(commented), [])

    def test_inline_enum_and_table_headers_are_rejected(self):
        inline_enum = self.source.replace(
            "pub(crate) enum ConfigCode {", "pub(crate) enum ConfigCode { HgcfgProbeGap,", 1
        )
        self.assertIn("cannot find the ConfigCode enum", self.failures(inline_enum))

        header = "const CONFIG_CODES: &[ConfigCodeEntry] = &["
        inline_table = self.source.replace(
            header,
            header + " ConfigCodeEntry { code: ConfigCode::HgcfgVersion, id: \"X\", title: \"t\", help: \"h\" },",
            1,
        )
        self.assertIn("cannot find the CONFIG_CODES registry", self.failures(inline_table))

    def test_table_header_requires_the_exact_type_and_initializer(self):
        header = "const CONFIG_CODES: &[ConfigCodeEntry] = &["
        cases = {
            "wrong type": "const CONFIG_CODES: &[OtherEntry] = &[",
            "missing initializer": "const CONFIG_CODES: &[ConfigCodeEntry]",
            "trailing content": header + " // entries",
            "array length": "const CONFIG_CODES: &[ConfigCodeEntry; 39] = &[",
        }
        for label, replacement in cases.items():
            with self.subTest(label=label):
                source = self.source.replace(header, replacement, 1)
                self.assertIn("cannot find the CONFIG_CODES registry", self.failures(source))

    def test_unsupported_enum_syntax_is_rejected(self):
        cases = {
            "attribute": "    #[cfg(test)]\n    HgcfgProbeGap,",
            "field variant": "    HgcfgProbeGap { value: u8 },",
            "expression discriminant": "    HgcfgProbeGap = 1 + 2,",
            "trailing comment": "    HgcfgProbeGap, // gap",
            "inline multiple variants": "    HgcfgProbeGap, HgpluginStore,",
        }
        for label, variant in cases.items():
            with self.subTest(label=label):
                source = self.source.replace(CLOSING_VARIANT, f"    HgpluginVersion,\n{variant}\n}}")
                _, failures = gate.parse_config_code_enum(source)
                self.assertTrue(failures, label)

    def test_empty_enum_and_table_fail(self):
        start = self.source.index("pub(crate) enum ConfigCode {")
        end = self.source.index("\n}", start) + 2
        empty_enum = self.source[:start] + "pub(crate) enum ConfigCode {\n}" + self.source[end:]
        self.assertTrue(any("no variants" in failure for failure in self.failures(empty_enum)))

        table_start = self.source.index("const CONFIG_CODES")
        table_end = self.source.index("\n];", table_start) + 3
        empty_table = self.source[:table_start] + "const CONFIG_CODES: &[ConfigCodeEntry] = &[\n];" + self.source[table_end:]
        self.assertTrue(any("no entries" in failure for failure in self.failures(empty_table)))

    def test_duplicate_and_missing_table_entries_fail(self):
        first_entry = self.source[self.source.index("    ConfigCodeEntry {"):self.source.index("    },", self.source.index("    ConfigCodeEntry {")) + 6]
        table_start = self.source.index("const CONFIG_CODES")
        body_start = self.source.index("= &[", table_start) + 4
        duplicated = self.source[:body_start] + "\n" + first_entry + self.source[body_start:]
        failures = self.failures(duplicated)
        self.assertTrue(any("duplicate ConfigCode entries" in failure for failure in failures), failures)

        removed = self.source.replace(first_entry, "", 1)
        failures = self.failures(removed)
        self.assertTrue(any("registry variants differ" in failure for failure in failures), failures)

    def test_unconsumed_or_incomplete_table_content_fails(self):
        table_start = self.source.index("const CONFIG_CODES")
        body_start = self.source.index("= &[", table_start) + 4
        cases = {
            "garbage": "\n    garbage\n",
            "unknown field": "\n    ConfigCodeEntry { code: ConfigCode::HgpluginVersion, id: \"X\", title: \"t\", help: \"h\", extra: \"x\" },\n",
            "missing field": "\n    ConfigCodeEntry { code: ConfigCode::HgpluginVersion, id: \"X\", title: \"t\" },\n",
            "missing comma": "\n    ConfigCodeEntry { code: ConfigCode::HgpluginVersion id: \"X\", title: \"t\", help: \"h\" },\n",
        }
        for label, content in cases.items():
            with self.subTest(label=label):
                source = self.source[:body_start] + content + self.source[body_start:]
                _, failures = gate.parse_config_codes_table(source)
                self.assertTrue(failures, label)

    def test_missing_documentation_heading_body_and_guide_fail(self):
        heading = "## hgcfg-version\n"
        self.assertIn(heading, self.documentation)
        failures = gate.diagnostic_registry_failures(self.source, self.documentation.replace(heading, "", 1), gate.CRATE)
        self.assertTrue(any("missing independent documentation heading" in failure for failure in failures), failures)

        body = "**HGCFG-VERSION — "
        failures = gate.diagnostic_registry_failures(self.source, self.documentation.replace(body, "", 1), gate.CRATE)
        self.assertTrue(any("missing documentation entry body" in failure for failure in failures), failures)

        with tempfile.TemporaryDirectory() as directory:
            failures = gate.diagnostic_registry_failures(self.source, self.documentation, Path(directory))
        self.assertTrue(any("missing detailed guide" in failure for failure in failures), failures)

    def test_unreadable_registry_or_documentation_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(gate, "CRATE", Path(directory)):
                with self.assertRaises(FileNotFoundError):
                    gate.diagnostic_code_failures()
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(gate, "ROOT", Path(directory)):
                with self.assertRaises(FileNotFoundError):
                    gate.diagnostic_code_failures()


if __name__ == "__main__":
    unittest.main()
