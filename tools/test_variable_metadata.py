# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect metadata profile coverage, effect anchors and oracle safety evidence."""

import dataclasses
import json
from pathlib import Path
import tomllib
import unittest

import conformance

ROOT = Path(__file__).resolve().parent.parent


class VariableMetadataTests(unittest.TestCase):
    def test_all_metadata_cases_remain_required(self):
        ledger = tomllib.loads((ROOT / 'tests/requirements.toml').read_text())
        family = next(row for row in ledger['requirements'] if row['id'] == 'variables.metadata')
        self.assertEqual(family['fixture_count'], family['candidate_required'])
        self.assertGreaterEqual(family['fixture_count'], 265)
        for name in family['cases']:
            self.assertEqual(conformance.load_case(ROOT / 'tests/cases' / name).candidate, 'required')
        for profile in range(4):
            for phase in ('reader', 'recipe'):
                self.assertIn(f'metadata-names-growth-{profile}-2048-{phase}', family['cases'])

    def test_recursive_metadata_effect_and_raw_value_have_separate_anchors(self):
        case = conformance.load_case(ROOT / 'tests/cases/metadata-names-cache-effects-reader')
        stdout = bytes.fromhex(case.expected['stdout_hex'])
        anchor = b'\neffect\neffect\nafter=[custom]:[custom]:[$(info effect)custom]\n'
        self.assertIn(anchor, stdout)
        good = conformance.Observation(stdout, bytes.fromhex(case.expected['stderr_hex']),
                                       case.expected['exit_code'], None, {})
        self.assertEqual(conformance.expectation_errors(case, good), [])
        for replacement in (b'\neffect\nafter=[custom]:[custom]:[$(info effect)custom]\n',
                            b'\neffect\neffect\neffect\nafter=[custom]:[custom]:[$(info effect)custom]\n'):
            bad = dataclasses.replace(good, stdout=stdout.replace(anchor, replacement))
            self.assertIn('oracle disagrees with expected.stdout_hex',
                          conformance.expectation_errors(case, bad))

    def test_default_goal_signal_is_research_without_equivalence_credit(self):
        record = json.loads((ROOT / 'tests/research/gnu-441-metadata-signal.json').read_text())
        self.assertEqual(record['classification'],
                         'oracle metadata safety research; not passing conformance fixtures')
        self.assertEqual(len(record['records']), 1)
        row = record['records'][0]
        self.assertFalse((ROOT / 'tests/cases' / row['id']).exists())
        self.assertTrue(all(run['status'] == -11 for run in row['oracle_runs']))
        self.assertEqual(row['candidate']['status'], 0)
        self.assertIsNone(row['candidate']['limit'])
        self.assertFalse(row['candidate']['leftovers'])


if __name__ == '__main__':
    unittest.main()
