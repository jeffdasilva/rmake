# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect exact warning phases, disabled controls and visible startup gaps."""
import dataclasses
import json
from pathlib import Path
import tomllib
import unittest
import conformance

ROOT = Path(__file__).resolve().parents[1]


class UndefinedWarningTests(unittest.TestCase):
    def case(self, name):
        return conformance.load_case(ROOT / 'tests/cases' / name)

    def streams(self, name):
        case = self.case(name)
        return bytes.fromhex(case.expected['stdout_hex']), bytes.fromhex(case.expected['stderr_hex'])

    def test_required_cases_reach_intended_phases_and_keep_silent_controls(self):
        ledger = tomllib.loads((ROOT / 'tests/requirements.toml').read_text())
        family = next(row for row in ledger['requirements'] if row['id'] == 'cli.undefined-warnings')
        self.assertEqual(family['fixture_count'], 358)
        self.assertEqual(family['candidate_required'], 358)
        for name in family['cases']:
            case = self.case(name)
            self.assertEqual(case.candidate, 'required')
            self.assertEqual(case.expected['exit_code'], 2 if name == 'warn-internal-shell' else 0)
            if name.startswith('warn-disabled-'):
                self.assertEqual(case.expected['stderr_hex'], '')
        for profile in range(3):
            for source in ('cli', 'make', 'gnu', 'env-make', 'env-gnu', 'nested', 'override', 'ignored-e'):
                for phase in ('reader', 'recipe', 'secondary'):
                    self.assertIn(f'warn-flow-{profile}-{source}-{phase}', family['cases'])

    def test_policy_changes_decoder_order_and_exempt_names_are_observed_exactly(self):
        _, nested = self.streams('warn-source-nested')
        self.assertIn(b"Makefile:3: warning: undefined variable 'LATE'\n", nested)
        self.assertNotIn(b"variable 'EARLY'", nested)
        _, inherited = self.streams('warn-source-env-gnu')
        self.assertTrue(inherited.startswith(b"make: warning: undefined variable 'MAKEFLAGS'\n"))
        self.assertEqual(self.streams('warn-extra-gnu-before-missing-make')[1],
            b"make: warning: undefined variable 'MAKEFLAGS'\nMakefile:5: warning: undefined variable 'ABSENT'\n")
        names = set()
        for path in (ROOT / 'tests/cases').glob('warn-exempt-*'):
            source = (path / 'Makefile').read_bytes()
            names.add(source.split(b'undefine ', 1)[1].split(b'\n', 1)[0])
            self.assertEqual(bytes.fromhex(self.case(path.name).expected['stderr_hex']),
                             b"make: warning: undefined variable 'GNUMAKEFLAGS'\n")
        self.assertEqual(len(names), 10)

    def test_raw_names_locations_and_warning_order_have_negative_anchors(self):
        case = self.case('warn-undefined-raw-name-reader')
        out, err = self.streams(case.id)
        self.assertIn(b"Makefile:3: warning: undefined variable 'raw\xff'\n", err)
        good = conformance.Observation(out, err, 0, None, {})
        self.assertEqual(conformance.expectation_errors(case, good), [])
        for mutation in ({'stderr': err.replace(b'\xff', b'?')},
                         {'stderr': b'\n'.join(reversed(err.rstrip(b'\n').split(b'\n'))) + b'\n'},
                         {'stderr': err.replace(b'Makefile:3:', b'Makefile:4:')},
                         {'returncode': 2}):
            self.assertTrue(conformance.expectation_errors(case, dataclasses.replace(good, **mutation)))
        self.assertIn(b"make: warning: undefined variable 'ABSENT'\n",
                      self.streams('warn-extra-secondary-no-recipe')[1])
        self.assertIn(b"child.mk:1: warning: undefined variable 'ABSENT'\n",
                      self.streams('warn-recursion')[1])

    def test_numeric_lexical_scopes_and_automatic_variables_keep_their_values(self):
        for phase in ('reader', 'recipe'):
            self.assertEqual(self.streams('warn-disabled-call-let-numeric-' + phase),
                             (b'result=[x/OUTER]\n', b''))
        self.assertEqual(self.streams('warn-extra-call-foreach-numeric')[0], b'result=[x/a x/b]\n')
        self.assertEqual(self.streams('warn-extra-call-numeric-eval')[0], b'result=[x/OUTER]\n')
        self.assertEqual(self.streams('warn-auto-40-recipe')[0], b'result=[all]\n')
        self.assertIn(b"variable '@'", self.streams('warn-auto-40-reader')[1])
        self.assertNotIn(b"variable '@'", self.streams('warn-auto-40-recipe')[1])
        self.assertNotIn(b"variable 'HOME'", self.streams('warn-extra-home-local-eval')[1])

    def test_historical_startup_mismatches_have_current_required_controls(self):
        record = json.loads((ROOT / 'tests/research/gnu-441-warning-startup.json').read_text())
        self.assertEqual(record['classification'],
                         'ordinary startup-order mismatches; not safety exceptions or passing fixtures')
        self.assertEqual(len(record['records']), 2)
        for row in record['records']:
            self.assertFalse((ROOT / 'tests/cases' / row['id']).exists())
            self.assertFalse((ROOT / 'tests/safety/cases' / row['id']).exists())
            self.assertEqual(row['observations']['oracle']['returncode'], 0)
            self.assertEqual(row['observations']['oracle'], row['observations']['repeat'])
            self.assertNotEqual(row['observations']['oracle'], row['observations']['candidate'])
        for name in ('cli-assignment-warning-cc', 'cli-assignment-warning-goal'):
            self.assertEqual(conformance.load_case(ROOT / 'tests/cases' / name).candidate, 'required')


if __name__ == '__main__':
    unittest.main()
