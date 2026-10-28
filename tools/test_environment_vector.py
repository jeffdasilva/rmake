# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect raw vector observations, activation phases and safety classification."""
import dataclasses
import json
from pathlib import Path
import tomllib
import unittest
import conformance

ROOT = Path(__file__).resolve().parent.parent

class EnvironmentVectorTests(unittest.TestCase):
    def test_raw_vector_and_phase_coverage_remains_required(self):
        ledger = tomllib.loads((ROOT / 'tests/requirements.toml').read_text())
        family = next(row for row in ledger['requirements']
                      if row['id'] == 'variables.environment-vector')
        self.assertEqual(family['candidate_required'], family['fixture_count'])
        self.assertGreaterEqual(family['fixture_count'], 278)
        for case_id in family['cases']:
            self.assertEqual(conformance.load_case(ROOT / 'tests/cases' / case_id).candidate,
                             'required')
        for header in ('special', 'bare', 'special-unexport', 'bare-unexport', 'phony', 'dependency'):
            for phase in ('normal', 'eval-unexport', 'eval-export', 'secondary', 'restart'):
                self.assertIn(f'env-vector-phase-{header}-{phase}', family['cases'])

    def test_order_and_binary_names_are_independent_exact_anchors(self):
        case = conformance.load_case(ROOT / 'tests/cases/env-vector-edge-recipe-binary-name')
        stdout = bytes.fromhex(case.expected['stdout_hex'])
        self.assertIn(b'raw\xff\n', stdout)
        good = conformance.Observation(stdout, bytes.fromhex(case.expected['stderr_hex']),
                                       case.expected['exit_code'], None, {})
        self.assertEqual(conformance.expectation_errors(case, good), [])
        for bad_stdout in (b'\n'.join(reversed(stdout.rstrip(b'\n').split(b'\n'))) + b'\n',
                           stdout.replace(b'raw\xff', b'raw?')):
            self.assertNotEqual(bad_stdout, stdout)
            self.assertIn('oracle disagrees with expected.stdout_hex',
                          conformance.expectation_errors(case,
                              dataclasses.replace(good, stdout=bad_stdout)))

    def test_deleting_an_active_export_is_safety_research(self):
        record = json.loads((ROOT / 'tests/research/gnu-441-environment-deletion.json').read_text())
        self.assertEqual(record['classification'],
                         'oracle environment safety research; not passing conformance fixtures')
        self.assertEqual(len(record['records']), 3)
        for row in record['records']:
            self.assertFalse((ROOT / 'tests/cases' / row['id']).exists())
            self.assertTrue(all(run['status'] == -11 for run in row['oracle_runs']))
            self.assertEqual(row['candidate']['status'], 0)
            self.assertFalse(row['candidate']['limit'] or row['candidate']['leftovers'])

if __name__ == '__main__':
    unittest.main()
