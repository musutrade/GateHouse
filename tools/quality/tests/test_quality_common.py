from __future__ import annotations

import contextlib
import io
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from quality_common import fail, read_json, sha256, stats, write_json


class QualityCommonTests(unittest.TestCase):
    def test_stats_reports_stable_summary(self) -> None:
        self.assertEqual(
            stats([4.0, 1.0, 3.0]),
            {"count": 3, "min": 1.0, "max": 4.0, "median": 3.0},
        )

    def test_stats_rejects_empty_samples(self) -> None:
        with self.assertRaises(ValueError):
            stats([])

    def test_json_round_trip_and_digest(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "nested" / "evidence.json"
            write_json(path, {"status": "pass", "values": [1, 2]})
            self.assertEqual(read_json(path), {"status": "pass", "values": [1, 2]})
            self.assertEqual(len(sha256(path)), 64)

    def test_fail_is_a_blocking_exit(self) -> None:
        output = io.StringIO()
        with contextlib.redirect_stderr(output), self.assertRaises(SystemExit) as raised:
            fail("fixture failure")
        self.assertEqual(raised.exception.code, 1)
        self.assertIn("fixture failure", output.getvalue())


if __name__ == "__main__":
    unittest.main()
