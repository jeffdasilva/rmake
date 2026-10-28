# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect independent POSIX error-mode, propagation and profile anchors."""
import dataclasses
from pathlib import Path
import tomllib
import unittest
import conformance

ROOT = Path(__file__).resolve().parent.parent

class PosixModeTests(unittest.TestCase):
    def test_profile_and_phase_cases_remain_required(self):
        ledger = tomllib.loads((ROOT / 'tests/requirements.toml').read_text())
        family = next(row for row in ledger['requirements'] if row['id'] == 'language.posix')
        self.assertEqual(family['candidate_required'], family['fixture_count'])
        self.assertGreaterEqual(family['fixture_count'], 390)
        for name in family['cases']:
            self.assertEqual(conformance.load_case(ROOT / 'tests/cases' / name).candidate,
                             'required')
        for profile in range(4):
            for phase in ('rule', 'include', 'eval', 'phony', 'dependency', 'target-variable'):
                self.assertIn(f'posix-defaults-{profile}-{phase}', family['cases'])

    def test_per_command_ignore_and_global_ignore_have_distinct_anchors(self):
        for suffix, stdout, stderr in (
                ('plain', b'reader=[]/[1]\nsecondn',
                 b'make: [Makefile:5: all] Error 1 (ignored)\n'),
                ('ignore', b'reader=[]/[1]\nrecipensecondn', b'')):
            case = conformance.load_case(ROOT / 'tests/cases' /
                                         ('posix-shell-1-global-ignore-' + suffix))
            self.assertEqual(bytes.fromhex(case.expected['stdout_hex']), stdout)
            self.assertEqual(bytes.fromhex(case.expected['stderr_hex']), stderr)
            good = conformance.Observation(stdout, stderr, 0, None, {})
            self.assertEqual(conformance.expectation_errors(case, good), [])
            self.assertTrue(conformance.expectation_errors(case,
                            dataclasses.replace(good, stdout=b'wrong')))

    def test_posix_recursive_flags_use_the_original_command_snapshot(self):
        case = conformance.load_case(ROOT / 'tests/cases/posix-recursion-1-after-custom')
        stdout = bytes.fromhex(case.expected['stdout_hex'])
        anchor = b'flags=[rR --no-print-directory -- $(-*-command-variables-*-)]/[rR --no-print-directory -- X=cli]\n'
        self.assertIn(anchor, stdout)
        self.assertIn(b'child X=[cli]/[command line] Y=[other]\n', stdout)
        good = conformance.Observation(stdout, b'', 0, None, {})
        self.assertEqual(conformance.expectation_errors(case, good), [])
        self.assertTrue(conformance.expectation_errors(case,
                        dataclasses.replace(good, stdout=stdout.replace(anchor, b''))))

if __name__ == '__main__':
    unittest.main()
