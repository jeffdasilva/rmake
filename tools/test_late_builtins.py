# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect successful phase reachability, priority and suffix-effect anchors."""
import dataclasses
from pathlib import Path
import tomllib
import unittest
import conformance

ROOT = Path(__file__).resolve().parents[1]


class LateBuiltinTests(unittest.TestCase):
    def case(self, name):
        return conformance.load_case(ROOT / 'tests/cases' / name.replace('-R', '-big-r'))

    def output(self, name):
        return bytes.fromhex(self.case(name).expected['stdout_hex'])

    def test_original_phase_origin_and_suffix_matrix_remains_required(self):
        ledger = tomllib.loads((ROOT / 'tests/requirements.toml').read_text())
        family = next(row for row in ledger['requirements'] if row['id'] == 'cli.builtin-transitions')
        self.assertEqual(family['fixture_count'], 204)
        self.assertEqual(family['candidate_required'], 204)
        for start in ('normal', 'r', 'R'):
            for variable in ('makeflags', 'gnumakeflags'):
                for form in ('append', 'simple', 'set', 'override', 'eval', 'include', 'preload'):
                    for disabled in ('rules', 'variables'):
                        name = f'late-builtins-{start}-{variable}-{form}-{disabled}'.replace('-R', '-big-r')
                        self.assertIn(name, family['cases'])
                        case = self.case(name)
                        self.assertEqual(case.candidate, 'required')
                        self.assertEqual(case.expected['exit_code'], 0)
                        output = bytes.fromhex(case.expected['stdout_hex'])
                        self.assertIn(b'secondary=', output)
                        self.assertIn(b'recipe=', output)
        for name in family['cases']:
            self.assertEqual(self.case(name).candidate, 'required')

    def test_late_removal_reaches_secondary_and_recipe_after_intact_reader_values(self):
        name = 'late-builtins-normal-makeflags-append-variables'
        case = self.case(name)
        stdout = self.output(name)
        lines = stdout.splitlines()
        self.assertEqual(len(lines), 4)
        for line in lines[:2]:
            self.assertIn(b'CC=[cc]/default/recursive', line)
            self.assertIn(b'suffix=[35]/default', line)
        for line in lines[2:]:
            self.assertIn(b'CC=[]/undefined/undefined', line)
            self.assertIn(b'CXX=[]/undefined', line)
            self.assertIn(b'lib=[]/undefined', line)
            self.assertIn(b'suffix=[0]/default', line)
            self.assertIn(b'shell=default/default,make=default,gnu=override,metadata-count=4', line)
        good = conformance.Observation(stdout, b'', 0, None, {})
        self.assertEqual(conformance.expectation_errors(case, good), [])
        for change in ({'returncode': 2}, {'stderr': b'early syntax error'},
                       {'stdout': b'\n'.join(lines[:2]) + b'\n'},
                       {'stdout': stdout.replace(b'CC=[]/undefined/undefined', b'CC=[cc]/default/recursive')}):
            self.assertTrue(conformance.expectation_errors(case, dataclasses.replace(good, **change)))

    def test_origins_posix_and_user_suffix_declarations_have_distinct_results(self):
        for variable in ('makeflags', 'gnumakeflags'):
            for phase in ('recipe', 'shell'):
                self.assertEqual(self.output(f'late-builtins-export-{variable}-default-{phase}'), b'env=[]\n')
                self.assertEqual(self.output(f'late-builtins-export-{variable}-file-{phase}'), b'env=[AFTER]\n')
                self.assertEqual(self.output(f'late-builtins-export-{variable}-environment-e-{phase}'), b'env=[ENV]\n')
        self.assertIn(b'CC=[file]/file/recursive', self.output('late-builtins-normal-R-file'))
        self.assertIn(b'CC=[command]/command line/recursive', self.output('late-builtins-normal-R-command'))
        self.assertIn(b'CC=[local]/file/recursive', self.output('late-builtins-normal-R-target-cc'))
        # -e makes the existing MAKEFLAGS binding prevail over a file append.
        last = self.output('late-builtins-normal-R-environment-e').splitlines()[-1]
        self.assertIn(b'CC=[env]/environment override/recursive,CXX=[g++]/default', last)
        self.assertIn(b'suffix=[35]/default', last)
        self.assertIn(b'CC=[c99]/default/simple', self.output('late-builtins-R-R-posix'))
        self.assertIn(b'CC=[]/undefined/undefined', self.output('late-builtins-normal-R-posix').splitlines()[-1])
        self.assertIn(b'suffix=[0]/default', self.output('late-builtins-normal-r-suffix-append'))
        self.assertTrue(self.output('late-builtins-normal-r-suffix-append').endswith(b';stem=[probe]\n'))
        self.assertTrue(self.output('late-builtins-normal-r-file').endswith(b';stem=[]\n'))

    def test_restart_and_postread_effects_preserve_the_original_phase_boundary(self):
        for flag in ('r', 'R'):
            for phase in ('recipe', 'secondary'):
                self.assertIn(b'CC=[cc]/default/recursive', self.output(f'late-builtins-after-read-{flag}-{phase}'))
            for remake in ('', 'remake-'):
                output = self.output(f'late-builtins-restart-{remake}{flag}-posix0')
                reads = [line.split(b';', 1)[1] for line in output.splitlines() if line.startswith(b'read-restart=')]
                self.assertEqual(len(reads), 2)
                self.assertEqual(reads[0], reads[1])
                self.assertIn(b'suffix=[35]/default', reads[1])
        # The custom recipe survives only if user suffix declarations retain .c/.o.
        self.assertEqual(self.case('late-builtins-suffix-R-untouched-custom').expected['exit_code'], 2)
        case = self.case('late-builtins-suffix-R-append-custom')
        self.assertEqual(case.expected['exit_code'], 0)
        self.assertIn(b'custom', (case.path / 'Makefile').read_bytes())
        reference = conformance.Observation(b'', b'', 0, None, {'work/probe.o': {'content': 'Y3VzdG9tCg=='}})
        self.assertIn('filesystem', conformance.differences(reference, dataclasses.replace(reference, filesystem={})))


if __name__ == '__main__':
    unittest.main()
