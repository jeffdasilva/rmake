# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original exact anchors for eval phases, encoding and anonymous diagnostics."""
import dataclasses
from pathlib import Path
import tomllib
import unittest
import conformance

ROOT = Path(__file__).resolve().parents[1]


class CommandLineEvalTests(unittest.TestCase):
    def case(self, suffix):
        return conformance.load_case(ROOT / 'tests/cases' / ('cli-eval-' + suffix))

    def out(self, suffix):
        return bytes.fromhex(self.case(suffix).expected['stdout_hex'])

    def err(self, suffix):
        return bytes.fromhex(self.case(suffix).expected['stderr_hex'])

    def test_profile_priority_flavor_and_large_argument_controls_remain_required(self):
        family = next(row for row in tomllib.loads((ROOT / 'tests/requirements.toml').read_text())['requirements'] if row['id'] == 'cli.eval')
        self.assertEqual(family['fixture_count'], 194)
        self.assertEqual(family['candidate_required'], 194)
        self.assertEqual(len(set(family['cases'])), 194)
        for profile in ('defaults', 'r', 'big-r', 'r-big-r'):
            for flavor in ('recursive', 'simple', 'double-simple', 'escaped', 'append', 'conditional', 'shell', 'define'):
                self.assertEqual(self.case(f'flavor-{profile}-{flavor}').candidate, 'required')
            for origin in ('absent', 'cli', 'env', 'env-e'):
                for assignment in ('file', 'conditional', 'override'):
                    self.assertEqual(self.case(f'priority-{profile}-{origin}-{assignment}').expected['exit_code'], 0)
        self.assertEqual(self.out('many-1000'), b'X=[value-999]\n')
        self.assertEqual(self.case('many-1000').args.count('-E'), 1000)

    def test_anonymous_and_real_file_diagnostics_remain_distinct(self):
        self.assertEqual(self.err('anonymous-fatal'), b'make: *** E.  Stop.\n')
        self.assertEqual(self.err('include-fatal'), b'inc.mk:1: *** INCLUDED.  Stop.\n')
        self.assertEqual(self.err('recipe-failure'), b'make: *** [<builtin>: all] Error 1\n')
        self.assertEqual(self.out('recipe-trace'), b"<builtin>: update target 'all' due to: target does not exist\n:\n")
        self.assertEqual(self.err('nested-file-double'), b"Makefile:3: *** unterminated call to function 'eval': missing ')'.  Stop.\n")
        self.assertTrue(self.err('nested-eval-double').startswith(b'make: *** unterminated call'))
        self.assertEqual(self.out('bom-recipe-fold'), b'one\n')
        self.assertEqual(self.out('nested-bom-eval'), b'[one/two]\n')
        self.assertEqual(self.out('nested-bom-file'), b'[one/]\n')

    def test_metadata_restarts_and_encoded_buffers_keep_their_phase_boundaries(self):
        plain = self.out('metadata-plain')
        self.assertTrue(plain.startswith(b'E=[undefined]/[]/[undefined]/'))
        self.assertIn(b'F=[file]/[Makefile]/[automatic]/[simple]/[--eval=', plain)
        commands = self.out('metadata-commands')
        self.assertIn(b'$(-*-eval-flags-*-) -- $(MAKEOVERRIDES)', commands)
        self.assertNotIn(b'[-rR --no-print-directory $(-*-eval-flags-*-)', commands)
        self.assertEqual(self.out('restart'), b'E=[]/[]\nF=[E]/[]\nE=[1]/[]\nF=[E]/[1]\n')
        self.assertEqual(self.out('late-file-with-initial'), b'F=[initial]\nR=[initial]\n')
        self.assertEqual(self.out('append-gnumakeflags-eval'), b'X=[]\n')
        self.assertEqual(self.out('append-makeflags-eval'), b'X=[added]\n')
        self.assertEqual(self.out('recursive-command-binding'), b'E=[0]/[CLI]\nE=[1]/[CLI]\nC=[CLI]\n')
        self.assertIn(b'\\ ', self.out('serialization-0'))
        self.assertIn(b'$$', self.out('serialization-3'))

    def test_eval_big_r_and_job_priority_are_observed_after_the_right_snapshot(self):
        self.assertEqual(self.case('builtin-user-big-r-user-suffix').expected['exit_code'], 2)
        self.assertEqual(self.out('builtin-user-r-user-suffix'), b'USER\n')
        self.assertEqual(self.out('builtin-user-big-r-user-pattern'), b'USER\n')
        phase = self.out('flag-phase-big-r')
        self.assertTrue(phase.startswith(b'E=[R --no-print-directory'))
        self.assertIn(b'R=[rR --no-print-directory', phase)
        self.assertTrue(self.out('builtin-transition-defaults-big-r').startswith(b'E=[default]/[35]\nF=[default]/[35]\n'))
        self.assertEqual(self.out('jobs-2-3-4'), b'E=[-j3]\nF=[-j4]\nR=[-j3]\n')
        self.assertEqual(self.out('jobs-none-3-4'), b'E=[-j3]\nF=[-j4]\nR=[-j4]\n')

    def test_intended_effects_and_raw_streams_have_negative_anchors(self):
        self.assertEqual(self.out('dedup-effect'), b'COUNT=[1]\n')
        self.assertEqual(self.out('distinct-effect'), b'COUNT=[2]\n')
        case = self.case('no-main-created-file')
        self.assertFalse((case.path / 'Makefile').exists())
        self.assertTrue(any('$$(info CREATED)' in argument for argument in case.args))
        self.assertEqual(self.out('no-main-rule'), b'EARLY\n')
        raw = self.case('raw-file')
        good = conformance.Observation(self.out('raw-file'), b'', 0, None, {})
        self.assertEqual(conformance.expectation_errors(raw, good), [])
        self.assertTrue(conformance.expectation_errors(raw, dataclasses.replace(good, stdout=b'F=[?]\n')))
        metadata = self.case('metadata-commands')
        observation = conformance.Observation(self.out('metadata-commands'), b'', 0, None, {})
        self.assertEqual(conformance.expectation_errors(metadata, observation), [])
        self.assertTrue(conformance.expectation_errors(metadata, dataclasses.replace(observation, stdout=observation.stdout.replace(b'$(-*-eval-flags-*-)', b'--eval=wrong'))))


if __name__ == '__main__':
    unittest.main()
