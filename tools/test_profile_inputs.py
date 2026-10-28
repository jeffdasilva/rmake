# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect original startup/profile and reference-boundary observations."""
import dataclasses
from pathlib import Path
import tomllib
import unittest
import conformance

ROOT = Path(__file__).resolve().parent.parent


class ProfileInputTests(unittest.TestCase):
    def case(self, name):
        return conformance.load_case(ROOT / 'tests/cases' / name)

    def stdout(self, name):
        return bytes.fromhex(self.case(name).expected['stdout_hex'])

    def test_required_coverage_reaches_valid_lookup_inputs(self):
        ledger = tomllib.loads((ROOT / 'tests/requirements.toml').read_text())
        for family_id, minimum in (('profiles.guile-disabled', 504), ('cli.flag-paths', 168),
                                   ('variables.reference-boundaries', 120)):
            family = next(row for row in ledger['requirements'] if row['id'] == family_id)
            self.assertEqual(family['candidate_required'], family['fixture_count'])
            self.assertGreaterEqual(family['fixture_count'], minimum)
            for name in family['cases']:
                case = self.case(name)
                self.assertEqual(case.candidate, 'required')
                if family_id == 'profiles.guile-disabled':
                    fatal = any(kind in name for kind in
                                ('-name-fatal-', '-value-fatal-', '-call-arg-fatal-'))
                    self.assertEqual(case.expected['exit_code'], 2 if fatal else 0, name)
                    self.assertNotIn(b'missing separator', bytes.fromhex(case.expected['stderr_hex']))
        for profile in range(4):
            for phase in ('reader', 'recipe', 'secondary', 'export', 'eval', 'restart'):
                for effect in ('name-eval', 'name-file', 'name-shell', 'name-effect', 'bound-effect'):
                    self.assertEqual(self.case(f'guile-disabled-{profile}-{effect}-{phase}')
                                     .expected['exit_code'], 0)
            for source in ('make', 'gnu', 'split'):
                self.assertEqual(self.case(f'flag-path-start-{profile}-both-{source}').candidate,
                                 'required')

    def test_guile_free_lookup_preserves_values_and_effect_order(self):
        for kind, expected in (
                ('name-eval', b'feature=[]\nresult=[EVAL]\n'),
                ('name-effect', b'feature=[]\nname-effect\nresult=[FOUND]\n'),
                ('bound-effect', b'feature=[]\nvalue-effect\nresult=[VALUE]\n'),
                ('scheme', b'feature=[]\nresult=[)]\n')):
            case = self.case(f'guile-disabled-0-{kind}-reader')
            self.assertEqual(bytes.fromhex(case.expected['stdout_hex']), expected)
            good = conformance.Observation(expected, b'', 0, None, {})
            self.assertEqual(conformance.expectation_errors(case, good), [])
            self.assertTrue(conformance.expectation_errors(case, dataclasses.replace(
                good, stdout=b'feature=[]\nresult=[]\n')))
        self.assertEqual(self.stdout('guile-disabled-0-name-shell-export'),
                         b'feature=[]\nresult=[SHELL]\n')

    def test_call_parameters_and_private_scope_are_ordinary_variables(self):
        self.assertEqual(self.stdout('guile-disabled-0-call-bound-recipe'),
                         b'feature=[]\nresult=[[guile|alpha|beta]]\n')
        self.assertEqual(self.stdout('guile-disabled-scope-0-private'),
                         b'child=[GLOBAL:ARG]\nparent=[PARENT:ARG]\nother=[GLOBAL:ARG]\n')
        self.assertEqual(self.stdout('guile-disabled-scope-0-inherit'),
                         b'child=[PARENT:ARG]\nparent=[PARENT:ARG]\nother=[GLOBAL:ARG]\n')
        self.assertIn(b'global=[GLOBAL:AFTER]\n',
                      self.stdout('guile-disabled-scope-0-foreach'))

    def test_startup_paths_and_late_ignored_options_have_distinct_anchors(self):
        startup = self.stdout('flag-path-start-0-both-make')
        self.assertIn(b'files=[flags.mk driver.mk]', startup)
        self.assertIn(b'cwd=[sub] value=[FLAG]\nSELECTED\n', startup)
        self.assertNotIn(b'-C', startup)
        late = self.stdout('flag-path-file-0-gnumakeflags-reader-both')
        self.assertEqual(late,
                         b'phase=[] flags=[rR --no-print-directory] '
                         b'gnu=[-C never-enter -f never-read.mk] files=[Makefile]\n'
                         b'phase=[] flags=[rR --no-print-directory] gnu=[] files=[Makefile]\n')
        self.assertNotIn(b'never-enter',
                         self.stdout('flag-path-file-0-makeflags-reader-both'))

    def test_simple_and_unmatched_names_keep_different_consumption_boundaries(self):
        for index, expected in ((0, b'result=[KEY0)TAIL]\n'),
                                (10, b'result=[KEY5]\n'), (16, b'result=[KEY5]\n')):
            case = self.case(f'reference-boundary-0-simple-{index}')
            self.assertEqual(bytes.fromhex(case.expected['stdout_hex']), expected)
            good = conformance.Observation(expected, b'', 0, None, {})
            self.assertEqual(conformance.expectation_errors(case, good), [])
            self.assertTrue(conformance.expectation_errors(case, dataclasses.replace(
                good, stdout=expected + b'must-not-run\n')))

    def test_unterminated_functions_and_variables_have_distinct_diagnostics(self):
        for index, ending in ((26, b')'), (27, b'}')):
            case = self.case(f'reference-boundary-0-simple-{index}')
            stderr = bytes.fromhex(case.expected['stderr_hex'])
            self.assertIn(b"unterminated call to function 'subst': missing '" + ending + b"'", stderr)
            good = conformance.Observation(b'', stderr, 2, None, {})
            self.assertEqual(conformance.expectation_errors(case, good), [])
            self.assertTrue(conformance.expectation_errors(case, dataclasses.replace(
                good, stderr=stderr.replace(b'call to function', b'variable reference'))))
        variable = self.case('reference-boundary-0-simple-24')
        self.assertIn(b'unterminated variable reference',
                      bytes.fromhex(variable.expected['stderr_hex']))


if __name__ == '__main__':
    unittest.main()
