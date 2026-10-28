# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect switch-origin coverage, negative spellings and current-file isolation."""
import dataclasses
from pathlib import Path
import tomllib
import unittest
import conformance
ROOT = Path(__file__).resolve().parent.parent

class SwitchOriginTests(unittest.TestCase):
    def test_alias_origin_and_scope_cases_remain_required(self):
        ledger = tomllib.loads((ROOT / 'tests/requirements.toml').read_text())
        family = next(row for row in ledger['requirements'] if row['id'] == 'cli.switch-origins')
        self.assertEqual(family['candidate_required'], family['fixture_count'])
        self.assertGreaterEqual(family['fixture_count'], 468)
        for name in family['cases']:
            self.assertEqual(conformance.load_case(ROOT / 'tests/cases' / name).candidate, 'required')
        for mode in range(2):
            for scope in ('none', 'file', 'override', 'target'):
                for phase in ('reader', 'recipe', 'secondary', 'siblings'):
                    for flag in ('iinc', 'nosilent', 's', 'e'):
                        self.assertIn(f'switch-scopes-{mode}-{scope}-{phase}-{flag}', family['cases'])

    def test_negative_spellings_cannot_disappear_from_recursive_flags(self):
        for name, spelling in (('switch-alias-10-normal', b'rRS'),
                               ('switch-alias-5-normal', b' --no-silent')):
            case = conformance.load_case(ROOT / 'tests/cases' / name)
            stdout = bytes.fromhex(case.expected['stdout_hex'])
            self.assertIn(spelling, stdout)
            good = conformance.Observation(stdout, bytes.fromhex(case.expected['stderr_hex']),
                                           case.expected['exit_code'], None, {})
            self.assertEqual(conformance.expectation_errors(case, good), [])
            self.assertTrue(conformance.expectation_errors(case,
                dataclasses.replace(good, stdout=stdout.replace(spelling, b'wrong'))))

    def test_file_local_and_inherited_metadata_have_distinct_anchors(self):
        case = conformance.load_case(ROOT / 'tests/cases/switch-scopes-0-target-siblings-iinc')
        stdout = bytes.fromhex(case.expected['stdout_hex'])
        self.assertEqual(stdout, b'state=[rR --no-print-directory]/[file]/[-rR --no-print-directory]/[environment]/[prior]/[file]\n'
                                 b'state=[rR --no-print-directory]/[file]/[legacy]/[file]/[prior]/[file]\n')
        good = conformance.Observation(stdout, b'', 0, None, {})
        self.assertEqual(conformance.expectation_errors(case, good), [])
        self.assertTrue(conformance.expectation_errors(case,
            dataclasses.replace(good, stdout=stdout.splitlines(keepends=True)[0] * 2)))

if __name__ == '__main__':
    unittest.main()
