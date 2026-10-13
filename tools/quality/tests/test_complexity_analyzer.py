from __future__ import annotations

import copy
import hashlib
import json
import platform
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from complexity_analyzer import (  # noqa: E402
    ANALYZER_NAME,
    ANALYZER_VERSION,
    RULE_NAME,
    RULE_VERSION,
    ComplexitySyntaxError,
    analyze_source,
    main,
)
from quality_evidence import series_key, validate_evidence  # noqa: E402


FIXTURE_ROOT = Path(__file__).resolve().parents[1] / "fixtures" / "complexity"
SOURCE_PATH = FIXTURE_ROOT / "controls.rs"
EXPECTED_PATH = FIXTURE_ROOT / "controls.expected.json"


def normalize(record: dict) -> dict:
    """Return a record that is comparable across checkouts and runtimes."""
    normalized = copy.deepcopy(record)
    normalized.pop("commit", None)
    normalized["series"]["toolchain"]["target"] = platform.machine()
    normalized["series"]["toolchain"]["python"] = platform.python_version()
    return normalized


class ComplexityAnalyzerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.source_text = SOURCE_PATH.read_text(encoding="utf-8")
        self.record = analyze_source(self.source_text, "controls.rs")
        self.expected = json.loads(EXPECTED_PATH.read_text())

    def test_locked_fixture_reproduces_expected_record(self) -> None:
        self.assertEqual(normalize(self.record), normalize(self.expected))

    def test_fixture_symbols_are_locked(self) -> None:
        symbols = self.record["symbols"]
        self.assertEqual(
            [symbol["qualified_name"] for symbol in symbols],
            [
                "decision",
                "classify",
                "loops",
                "query",
                "query::closure_1",
                "call_double",
                "call_double::closure_1",
                "call_double::closure_2",
                "outer",
                "with_macros",
                "main",
            ],
        )
        self.assertEqual(
            [symbol["kind"] for symbol in symbols],
            [
                "function",
                "function",
                "function",
                "function",
                "closure",
                "function",
                "closure",
                "closure",
                "function",
                "function",
                "function",
            ],
        )
        self.assertEqual(
            [symbol["metrics"]["cyclomatic_complexity"] for symbol in symbols],
            [5, 5, 5, 2, 1, 1, 2, 1, 1, 1, 1],
        )

    def test_fixture_symbol_contracts(self) -> None:
        symbols = {symbol["qualified_name"]: symbol for symbol in self.record["symbols"]}
        decision = symbols["decision"]
        self.assertEqual(
            {key: decision["raw"][key] for key in ("if", "and_and", "or_or")},
            {"if": 2, "and_and": 1, "or_or": 1},
        )
        self.assertEqual(
            decision["span"],
            {"start_line": 7, "start_column": 1, "end_line": 15, "end_column": 1},
        )
        self.assertTrue(decision["id"].startswith("controls.rs::function::decision@7:1:"))

        classify = symbols["classify"]
        self.assertEqual(
            {
                key: classify["raw"][key]
                for key in ("match", "match_arms", "guards")
            },
            {"match": 1, "match_arms": 3, "guards": 2},
        )
        self.assertTrue(classify["id"].startswith("controls.rs::function::classify@17:1:"))

        loops = symbols["loops"]
        self.assertEqual(
            {key: loops["raw"][key] for key in ("while", "for", "loop", "if")},
            {"while": 1, "for": 1, "loop": 1, "if": 1},
        )

        query = symbols["query"]
        self.assertEqual(
            {key: query["raw"][key] for key in ("question_mark", "closures")},
            {"question_mark": 1, "closures": 1},
        )
        query_closure = symbols["query::closure_1"]
        self.assertEqual(query_closure["metrics"]["cyclomatic_complexity"], 1)
        self.assertEqual(
            query_closure["span"],
            {"start_line": 42, "start_column": 33, "end_line": 45, "end_column": 5},
        )

        call_double = symbols["call_double"]
        self.assertEqual(call_double["raw"]["closures"], 2)
        self.assertEqual(symbols["call_double::closure_1"]["raw"]["if"], 1)
        self.assertEqual(
            symbols["call_double::closure_1"]["metrics"]["cyclomatic_complexity"],
            2,
        )
        self.assertEqual(
            symbols["call_double::closure_2"]["metrics"]["cyclomatic_complexity"],
            1,
        )

        outer = symbols["outer"]
        self.assertEqual(outer["raw"]["nested_functions"], 1)
        self.assertEqual(outer["metrics"]["cyclomatic_complexity"], 1)

        with_macros = symbols["with_macros"]
        self.assertEqual(with_macros["raw"]["macro_invocations_skipped"], 2)
        self.assertEqual(with_macros["metrics"]["cyclomatic_complexity"], 1)

        main_symbol = symbols["main"]
        self.assertEqual(main_symbol["metrics"]["cyclomatic_complexity"], 1)

    def test_source_digest_and_bytes_match_the_file(self) -> None:
        source = self.record["series"]["source"]
        digest = hashlib.sha256(self.source_text.encode("utf-8")).hexdigest()
        self.assertEqual(source["sha256"], digest)
        self.assertEqual(source["bytes"], len(self.source_text.encode("utf-8")))
        for symbol in self.record["symbols"]:
            self.assertEqual(symbol["source"]["sha256"], digest)
            self.assertEqual(symbol["source"]["path"], "controls.rs")

    def test_series_key_is_canonical(self) -> None:
        source = self.record["series"]["source"]
        expected = "|".join(
            (
                "complexity-evidence",
                ANALYZER_NAME,
                ANALYZER_VERSION,
                RULE_NAME,
                RULE_VERSION,
                platform.machine(),
                platform.python_version(),
                source["path"],
                source["sha256"],
                str(source["bytes"]),
            )
        )
        self.assertEqual(series_key(self.record), expected)

    def test_analyzer_is_reproducible(self) -> None:
        rerun = analyze_source(self.source_text, "controls.rs")
        self.assertEqual(normalize(self.record), normalize(rerun))

    def test_fixture_generates_valid_evidence(self) -> None:
        validate_evidence(
            self.record,
            expected_source_sha256=self.record["series"]["source"]["sha256"],
            expected_source_bytes=self.record["series"]["source"]["bytes"],
        )

    def test_rejects_non_braced_closure(self) -> None:
        source = (
            "fn add(x: i32, y: i32) -> i32 {\n"
            "    let f = |a, b| a + b;\n"
            "    f(x, y)\n"
            "}\n"
        )
        with self.assertRaises(ComplexitySyntaxError):
            analyze_source(source, "nonbraced.rs")

    def test_rejects_unsupported_expression_token(self) -> None:
        with self.assertRaises(ComplexitySyntaxError):
            analyze_source("fn broken() -> u32 {\n    .member\n}\n", "broken.rs")

    def test_cli_writes_a_valid_evidence_file(self) -> None:
        with tempfile.TemporaryDirectory(prefix="complexity-cli-") as directory:
            output = Path(directory) / "controls.json"
            status = main(
                [
                    "--source",
                    str(SOURCE_PATH),
                    "--source-root",
                    str(FIXTURE_ROOT),
                    "--output",
                    str(output),
                ]
            )
            self.assertEqual(status, 0)
            written = json.loads(output.read_text())
            validate_evidence(
                written,
                expected_source_sha256=written["series"]["source"]["sha256"],
                expected_source_bytes=written["series"]["source"]["bytes"],
            )
            self.assertEqual(normalize(written), normalize(self.record))


if __name__ == "__main__":
    unittest.main()
