# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original byte/origin anchors and decoder research classification checks."""

import dataclasses
import json
from pathlib import Path
import tomllib
import unittest

import conformance

ROOT = Path(__file__).resolve().parent.parent


class FlagPayloadTests(unittest.TestCase):
    def test_assignment_phase_and_operator_matrix_remains_required(self):
        ledger = tomllib.loads((ROOT / 'tests/requirements.toml').read_text())
        family = next(row for row in ledger['requirements']
                      if row['id'] == 'variables.flag-payloads')
        self.assertEqual(family['candidate_required'], family['fixture_count'])
        for phase in ('set', 'simple', 'append', 'override', 'gnu', 'eval'):
            for origin in ('undefined', 'file', 'simple', 'command', 'env', 'env-e', 'override'):
                for operator in ('recursive', 'simple', 'double', 'immediate', 'append',
                                 'conditional', 'shell'):
                    name = f'flag-payload-{phase}-{origin}-{operator}'
                    self.assertIn(name, family['cases'])
                    self.assertEqual(conformance.load_case(ROOT / 'tests/cases' / name).candidate,
                                     'required')

    def test_raw_dollars_and_control_bytes_are_not_normalized(self):
        for name, anchor in (('flag-payload-bytes-323', b'X=[a$b]/[a$b]'),
                             ('flag-payload-bytes-328', b'left\rright'),
                             ('flag-payload-bytes-329', b'left\x0cright')):
            case = conformance.load_case(ROOT / 'tests/cases' / name)
            stdout = bytes.fromhex(case.expected['stdout_hex'])
            self.assertIn(anchor, stdout)
            good = conformance.Observation(stdout, bytes.fromhex(case.expected['stderr_hex']),
                                           case.expected['exit_code'], None, {})
            self.assertEqual(conformance.expectation_errors(case, good), [])
            bad = dataclasses.replace(good, stdout=stdout.replace(anchor, b'wrong'))
            self.assertIn('oracle disagrees with expected.stdout_hex',
                          conformance.expectation_errors(case, bad))

    def test_decoder_safety_conflicts_are_not_conformance_passes(self):
        record = json.loads((ROOT / 'tests/research/gnu-441-flag-decoder.json').read_text())
        self.assertEqual(record['classification'],
                         'oracle decoder safety research; not passing conformance fixtures')
        self.assertEqual(len(record['records']), 6)
        for row in record['records']:
            self.assertFalse((ROOT / 'tests/cases' / row['id']).exists())
            self.assertEqual(len(row['oracle_runs']), 2)
            if row['classification'] == 'oracle nontermination':
                self.assertTrue(all(run['limit'] for run in row['oracle_runs']))
            self.assertIsNone(row['candidate']['limit'])
            self.assertFalse(row['candidate']['leftovers'])


if __name__ == '__main__':
    unittest.main()
