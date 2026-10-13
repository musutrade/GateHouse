from __future__ import annotations

import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from complexity_analyzer import analyze_source  # noqa: E402
from quality_evidence import (  # noqa: E402
    EvidenceError,
    assert_compatible_series,
    main,
    series_key,
    validate_evidence,
    validate_schema,
)


FIXTURE_ROOT = Path(__file__).resolve().parents[1] / "fixtures" / "complexity"
SOURCE_PATH = FIXTURE_ROOT / "controls.rs"
EXPECTED_PATH = FIXTURE_ROOT / "controls.expected.json"
SOURCE_SHA256 = "e049647d1177f5bbbb2bd2f73cf11169722544a571b16fbc3aef823a393e3bb8"
SOURCE_BYTES = 1771


def generated_record() -> dict:
    source_text = SOURCE_PATH.read_text(encoding="utf-8")
    return analyze_source(source_text, "controls.rs")


def symbol_by_name(record: dict, name: str) -> dict:
    return next(symbol for symbol in record["symbols"] if symbol["qualified_name"] == name)


class QualityEvidenceSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        self.record = generated_record()

    def test_valid_generated_record_passes(self) -> None:
        source = self.record["series"]["source"]
        validate_evidence(
            self.record,
            expected_source_sha256=source["sha256"],
            expected_source_bytes=source["bytes"],
        )
        self.assertEqual(validate_schema(self.record), [])

    def test_locked_fixture_passes(self) -> None:
        fixture = json.loads(EXPECTED_PATH.read_text())
        validate_evidence(
            fixture,
            expected_source_sha256=SOURCE_SHA256,
            expected_source_bytes=SOURCE_BYTES,
        )

    def test_unknown_root_field_is_rejected(self) -> None:
        invalid = copy.deepcopy(self.record)
        invalid["unlocked_extension"] = True
        self.assertTrue(validate_schema(invalid))
        with self.assertRaises(EvidenceError):
            validate_evidence(invalid)

    def test_missing_raw_key_fails_closed(self) -> None:
        invalid = copy.deepcopy(self.record)
        del invalid["symbols"][0]["raw"]["if"]
        with self.assertRaises(EvidenceError):
            validate_evidence(invalid)

    def test_negative_raw_count_fails_closed(self) -> None:
        invalid = copy.deepcopy(self.record)
        invalid["symbols"][0]["raw"]["if"] = -1
        with self.assertRaises(EvidenceError):
            validate_evidence(invalid)

    def test_noncanonical_symbol_id_fails_closed(self) -> None:
        invalid = copy.deepcopy(self.record)
        invalid["symbols"][0]["id"] = invalid["symbols"][0]["id"][:-12] + "0" * 12
        with self.assertRaises(EvidenceError):
            validate_evidence(invalid)

    def test_duplicate_symbol_id_fails_closed(self) -> None:
        invalid = copy.deepcopy(self.record)
        invalid["symbols"].append(copy.deepcopy(invalid["symbols"][0]))
        with self.assertRaises(EvidenceError):
            validate_evidence(invalid)

    def test_guards_may_not_exceed_match_arms(self) -> None:
        invalid = copy.deepcopy(self.record)
        classify = symbol_by_name(invalid, "classify")
        classify["raw"]["guards"] = classify["raw"]["match_arms"] + 1
        with self.assertRaises(EvidenceError):
            validate_evidence(invalid)

    def test_match_arms_may_not_be_below_match(self) -> None:
        invalid = copy.deepcopy(self.record)
        classify = symbol_by_name(invalid, "classify")
        classify["raw"]["match_arms"] = classify["raw"]["match"] - 1
        with self.assertRaises(EvidenceError):
            validate_evidence(invalid)

    def test_match_arms_require_a_match(self) -> None:
        invalid = copy.deepcopy(self.record)
        classify = symbol_by_name(invalid, "classify")
        classify["raw"]["match"] = 0
        with self.assertRaises(EvidenceError):
            validate_evidence(invalid)

    def test_cyclomatic_complexity_must_recompute_from_raw(self) -> None:
        invalid = copy.deepcopy(self.record)
        decision = symbol_by_name(invalid, "decision")
        decision["metrics"]["cyclomatic_complexity"] += 1
        with self.assertRaises(EvidenceError):
            validate_evidence(invalid)

    def test_invalid_source_digest_fails_closed(self) -> None:
        invalid = copy.deepcopy(self.record)
        invalid["series"]["source"]["sha256"] = "not-a-sha256"
        with self.assertRaises(EvidenceError):
            validate_evidence(invalid)

    def test_absolute_source_path_fails_closed(self) -> None:
        for path in ("/tmp/controls.rs", "dir/../controls.rs", ".hidden.rs"):
            with self.subTest(path=path):
                invalid = copy.deepcopy(self.record)
                invalid["series"]["source"]["path"] = path
                with self.assertRaises(EvidenceError):
                    validate_evidence(invalid)

    def test_mismatched_expected_source_digest_fails_closed(self) -> None:
        with self.assertRaises(EvidenceError):
            validate_evidence(
                copy.deepcopy(self.record),
                expected_source_sha256="0" * 64,
                expected_source_bytes=self.record["series"]["source"]["bytes"],
            )

    def test_mismatched_expected_source_bytes_fails_closed(self) -> None:
        with self.assertRaises(EvidenceError):
            validate_evidence(
                copy.deepcopy(self.record),
                expected_source_sha256=self.record["series"]["source"]["sha256"],
                expected_source_bytes=self.record["series"]["source"]["bytes"] + 1,
            )

    def test_mismatched_expected_commit_fails_closed(self) -> None:
        invalid = copy.deepcopy(self.record)
        invalid["commit"] = "a" * 40
        with self.assertRaises(EvidenceError):
            validate_evidence(invalid, expected_commit="b" * 40)


class QualityEvidenceSeriesTests(unittest.TestCase):
    def setUp(self) -> None:
        self.record = generated_record()

    def test_compatible_series_returns_shared_key(self) -> None:
        key = assert_compatible_series([self.record, copy.deepcopy(self.record)])
        self.assertEqual(key, series_key(self.record))

    def test_series_key_rejects_separator_and_newlines(self) -> None:
        bad_names = ("a|b", "a\nb", "a\rb")
        for name in bad_names:
            with self.subTest(name=name):
                invalid = copy.deepcopy(self.record)
                invalid["series"]["analyzer"]["name"] = name
                with self.assertRaises(EvidenceError):
                    series_key(invalid)

    def test_series_key_rejects_empty_components(self) -> None:
        invalid = copy.deepcopy(self.record)
        invalid["series"]["analyzer"]["version"] = ""
        with self.assertRaises(EvidenceError):
            series_key(invalid)

    def test_incompatible_series_are_rejected(self) -> None:
        mutations = {
            "kind": lambda record: record.update(kind="coverage-evidence"),
            "analyzer.name": lambda record: record["series"]["analyzer"].update(
                name="other-analyzer"
            ),
            "analyzer.version": lambda record: record["series"]["analyzer"].update(
                version="999.0.0"
            ),
            "rule.version": lambda record: record["series"]["rule"].update(version="2"),
            "toolchain.target": lambda record: record["series"]["toolchain"].update(
                target="aarch64"
            ),
            "toolchain.python": lambda record: record["series"]["toolchain"].update(
                python="3.13.0"
            ),
            "source.path": lambda record: record["series"]["source"].update(
                path="other.rs"
            ),
            "source.sha256": lambda record: record["series"]["source"].update(
                sha256="a" * 64
            ),
            "source.bytes": lambda record: record["series"]["source"].update(
                bytes=record["series"]["source"]["bytes"] + 1
            ),
        }
        for name, mutate in mutations.items():
            with self.subTest(field=name):
                incompatible = copy.deepcopy(self.record)
                mutate(incompatible)
                with self.assertRaises(EvidenceError):
                    assert_compatible_series([self.record, incompatible])


class QualityEvidenceCliTests(unittest.TestCase):
    def setUp(self) -> None:
        self.record = generated_record()

    def test_validate_cli_rejects_invalid_record(self) -> None:
        invalid = copy.deepcopy(self.record)
        invalid["symbols"][0]["id"] = "not-canonical"
        with tempfile.TemporaryDirectory(prefix="quality-evidence-cli-") as directory:
            path = Path(directory) / "invalid.json"
            path.write_text(json.dumps(invalid))
            self.assertEqual(main(["validate", "--record", str(path)]), 1)

    def test_compare_series_cli_accepts_and_rejects(self) -> None:
        compatible = copy.deepcopy(self.record)
        incompatible = copy.deepcopy(self.record)
        incompatible["series"]["source"]["bytes"] += 1
        with tempfile.TemporaryDirectory(prefix="quality-evidence-cli-") as directory:
            first = Path(directory) / "first.json"
            second = Path(directory) / "second.json"
            third = Path(directory) / "third.json"
            first.write_text(json.dumps(self.record))
            second.write_text(json.dumps(compatible))
            third.write_text(json.dumps(incompatible))
            self.assertEqual(
                main(["compare-series", "--record", str(first), "--record", str(second)]),
                0,
            )
            self.assertEqual(
                main(["compare-series", "--record", str(first), "--record", str(third)]),
                1,
            )


if __name__ == "__main__":
    unittest.main()
