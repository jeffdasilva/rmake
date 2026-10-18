# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Prevent the requirement inventory and executable fixtures from drifting apart."""

from collections import Counter
from pathlib import Path
import tomllib
import unittest

import conformance

ROOT = Path(__file__).resolve().parent.parent


class CorpusTests(unittest.TestCase):
    def test_every_case_is_mapped_once_to_its_declared_requirement(self):
        ledger = tomllib.loads((ROOT / "tests/requirements.toml").read_text())
        requirements = ledger["requirements"]
        self.assertEqual(ledger["target_version"], "4.4.1")
        ids = [item["id"] for item in requirements]
        self.assertEqual(len(ids), len(set(ids)))
        mapped = Counter()
        cases = {path.name: conformance.load_case(path)
                 for path in (ROOT / "tests/cases").iterdir() if path.is_dir()}
        self.assertGreaterEqual(len(cases), 100)
        for requirement in requirements:
            self.assertEqual(requirement["fixture_count"], len(requirement["cases"]))
            for case_id in requirement["cases"]:
                mapped[case_id] += 1
                self.assertIn(case_id, cases)
                self.assertEqual(cases[case_id].requirement, requirement["id"])
        self.assertEqual(set(cases), set(mapped))
        self.assertTrue(all(count == 1 for count in mapped.values()))

    def test_every_fixture_has_independent_expectations(self):
        for path in (ROOT / "tests/cases").iterdir():
            if path.is_dir():
                with self.subTest(case=path.name):
                    case = conformance.load_case(path)
                    self.assertIn("exit_code", case.expected)
                    self.assertIn("stdout", case.expected)


if __name__ == "__main__":
    unittest.main()
