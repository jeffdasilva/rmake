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

    def test_m4_rule_families_have_required_cases_without_pending_exemptions(self):
        ledger = tomllib.loads((ROOT / "tests/requirements.toml").read_text())
        families = {item["id"]: item for item in ledger["requirements"]
                    if item["id"].startswith(("rules.", "archives.", "resolution."))}
        self.assertTrue({"rules.explicit", "rules.implicit", "rules.suffix-metadata",
                         "rules.grouped", "rules.intermediate-policy", "rules.libraries",
                         "rules.extra-prerequisites", "rules.file-records",
                         "rules.recursive-home", "archives.observation-order",
                         "resolution.vpath-records", "resolution.implicit-secondary"}
                        <= families.keys())
        for name, family in families.items():
            with self.subTest(requirement=name):
                self.assertGreater(family["fixture_count"], 0)
                self.assertEqual(family["candidate_required"], family["fixture_count"])
                for case_id in family["cases"]:
                    case = conformance.load_case(ROOT / "tests/cases" / case_id)
                    self.assertEqual(case.candidate, "required")

    def test_every_fixture_has_independent_expectations(self):
        for path in (ROOT / "tests/cases").iterdir():
            if path.is_dir():
                with self.subTest(case=path.name):
                    case = conformance.load_case(path)
                    self.assertIn("exit_code", case.expected)
                    self.assertEqual(
                        len({"stdout", "stdout_hex"}.intersection(case.expected)), 1,
                        "exactly one independent stdout expectation is required",
                    )


if __name__ == "__main__":
    unittest.main()
