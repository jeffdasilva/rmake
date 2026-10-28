# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Independent exact anchors for startup assignment effects and source identity."""
import dataclasses
import json
from pathlib import Path
import tomllib
import unittest
import conformance

ROOT = Path(__file__).resolve().parents[1]


class CliAssignmentEffectsTests(unittest.TestCase):
    def case(self, suffix):
        return conformance.load_case(ROOT / 'tests/cases' / ('cli-assignment-' + suffix))

    def out(self, suffix):
        return bytes.fromhex(self.case(suffix).expected['stdout_hex'])

    def err(self, suffix):
        return bytes.fromhex(self.case(suffix).expected['stderr_hex'])

    def test_profile_operator_and_priority_matrix_is_required(self):
        family = next(r for r in tomllib.loads((ROOT / 'tests/requirements.toml').read_text())['requirements'] if r['id'] == 'cli.assignment-effects')
        self.assertEqual(family['fixture_count'], 186)
        self.assertEqual(family['candidate_required'], 186)
        self.assertEqual(len(set(family['cases'])), 186)
        for profile in ('defaults', 'r', 'big-r', 'r-big-r'):
            for operator in ('recursive', 'simple', 'double', 'escaped', 'append', 'conditional', 'shell'):
                self.assertEqual(self.case(f'phase-{profile}-{operator}').candidate, 'required')
            for origin in ('absent', 'cli', 'env', 'env-e'):
                for operation in ('normal', 'conditional', 'override', 'undefine'):
                    self.assertEqual(self.case(f'priority-{profile}-{origin}-{operation}').candidate, 'required')
        self.assertTrue(self.out('phase-defaults-recursive').startswith(b'BEFORE=[]/[undefined]\n'))
        self.assertTrue(self.out('phase-defaults-simple').startswith(b'BEFORE=[yes]/[file]\n'))
        self.assertEqual(self.out('lazy-conditional'), b'[kept]/[undefined]\n')
        self.assertEqual(self.out('name-once'), b'NAME\n[payload]/[name]\n')

    def test_default_initialization_has_observable_phase_and_priority(self):
        self.assertEqual(self.out('default-goal-defaults-absent'), b'START=[undefined]\nFILE=[file]/[]\nALL\n')
        self.assertEqual(self.out('default-goal-defaults-eval-reset'), b'FILE=[file]/[]\nALL\n')
        self.assertEqual(self.out('default-goal-defaults-command-wins'), b'START=[command line]\nFILE=[command line]/[chosen]\nCHOSEN\n')
        self.assertEqual(self.out('shell-metadata-empty-command'), b'START=[]/[command line]/[recursive]\nFILE=[/bin/sh]/[file]/[recursive]\n')
        self.assertIn(b'FILE=[/bin/sh]/[file]/[recursive]', self.out('shell-metadata-environment-override'))
        self.assertEqual(self.out('shell-ordered-before'), b'[yes]\n')
        self.assertEqual(self.err('shell-ordered-after'), b'make: -c: No such file or directory\n')
        self.assertIn(b"variable 'CC'", self.err('warning-cc'))
        self.assertIn(b"variable '.DEFAULT_GOAL'", self.err('warning-goal'))
        self.assertNotIn(b"variable 'SHELL'", self.err('warning-shell'))
        self.assertTrue(self.err('warning-shell').endswith(b"variable 'GNUMAKEFLAGS'\n"))

    def test_constructor_errors_keep_real_file_identity_and_prior_output(self):
        self.assertEqual(self.err('anonymous-error'), b'make: *** EARLY.  Stop.\n')
        self.assertEqual(self.err('include-error'), b'early.mk:1: *** INCLUDED.  Stop.\n')
        self.assertEqual(self.err('include-parse'), b'early.mk:1: *** missing separator.  Stop.\n')
        for mode in ('none', 'line', 'target', 'recurse'):
            self.assertEqual(self.out('output-error-' + mode), b'BEFORE\n')
            self.assertEqual(self.err('output-error-' + mode), b'early.mk:1: *** INCLUDED.  Stop.\n')
        self.assertEqual(self.out('include-optional-before-search'), b'[]/[undefined]\n')

    def test_effect_scopes_raw_bytes_and_recursion_have_negative_anchors(self):
        self.assertEqual(self.out('call'), b'[]/[one/two]/[undefined]\n')
        self.assertEqual(self.out('let'), b'[]/[one/two three]/[undefined]/[undefined]\n')
        self.assertEqual(self.out('export'), b'[from-cli]\n')
        self.assertEqual(self.out('unexport'), b'[unset]\n')
        self.assertEqual(self.out('recursive-command'), b'P=[payload]/[early]\nC=[payload]/[]\n')
        self.assertEqual(self.out('restart-value'), b'[]/[payload]/[early]\n[1]/[payload]/[early]\n')
        for suffix, raw in (('raw-file', b'\xfe'), ('raw-shell-explicit-shell', b'\xff')):
            case = self.case(suffix)
            good = conformance.Observation(self.out(suffix), self.err(suffix), 0, None, {})
            self.assertEqual(conformance.expectation_errors(case, good), [])
            self.assertTrue(conformance.expectation_errors(case, dataclasses.replace(good, stdout=good.stdout.replace(raw, b'?'))))

    def test_historical_restart_file_effect_has_a_required_current_control(self):
        research = json.loads((ROOT / 'tests/research/cli-assignment-restart-effect.json').read_text())
        self.assertEqual(research['classification'], 'ordinary CLI assignment restart effect mismatch; not a safety exception or passing fixture')
        row = research['record']
        current = conformance.load_case(ROOT / 'tests/cases' / row['id'])
        self.assertEqual(current.candidate, 'required')
        self.assertEqual(current.expected, row['expected'])
        self.assertFalse((ROOT / 'tests/safety/cases' / row['id']).exists())
        self.assertEqual(row['observations'][0], row['observations'][1])
        self.assertEqual(row['model_errors'][:2], [[], []])
        self.assertEqual(bytes.fromhex(row['expected']['stdout_hex']), b'[]/[1]\n[1]/[2]\n')
        self.assertIn('filesystem', row['differences'])
        for observation in row['observations']:
            self.assertEqual(observation['returncode'], 0)
            self.assertIsNone(observation['limit'])
            self.assertFalse(observation['leftover_processes'])


if __name__ == '__main__':
    unittest.main()
