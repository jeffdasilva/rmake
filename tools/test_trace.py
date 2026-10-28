# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect trace phases, rebuild identity and independently modeled streams."""
import dataclasses
from pathlib import Path
import tomllib
import unittest
import conformance

ROOT = Path(__file__).resolve().parents[1]


class TraceTests(unittest.TestCase):
    def case(self, name):
        return conformance.load_case(ROOT / 'tests/cases' / name)

    def out(self, name):
        return bytes.fromhex(self.case(name).expected['stdout_hex'])

    def test_required_models_keep_success_and_nonexecuting_boundaries(self):
        family = next(row for row in tomllib.loads((ROOT / 'tests/requirements.toml').read_text())['requirements'] if row['id'] == 'cli.trace')
        self.assertEqual(family['candidate_required'], 165)
        self.assertEqual(family['fixture_count'], 165)
        self.assertEqual(len(set(family['cases'])), 165)
        for name in family['cases']:
            case = self.case(name)
            self.assertEqual(case.candidate, 'required')
            self.assertIn('stdout_hex', case.expected)
            self.assertIn('stderr_hex', case.expected)
        self.assertEqual(self.case('trace-missing-query').expected['exit_code'], 1)
        self.assertEqual(self.case('trace-empty-expanded-query').expected['exit_code'], 0)
        self.assertEqual(self.out('trace-empty-expanded-query'), b"expanded\nMakefile:3: update target 'all' due to: target does not exist\n")
        self.assertEqual(self.out('trace-missing-touch'), b'touch all\n')
        self.assertNotIn(b'update target', self.out('trace-empty-normal'))
        self.assertNotIn(b'update target', self.out('trace-prefix-only-normal'))

    def test_logical_completion_does_not_erase_missing_phony_dependencies(self):
        self.assertIn(b'due to: missing\n', self.out('trace-existing-hidden-missing-normal'))
        self.assertIn(b'due to: unknown reasons\n', self.out('trace-existing-hidden-missing-dry'))
        self.assertIn(b'due to: unknown reasons\n', self.out('trace-existing-missing-order-dry'))
        for kind in ('order', 'hidden'):
            for state in ('absent', 'existing'):
                name = f'trace-phony-fallback-{kind}-{state}'
                self.assertIn(b'due to: missing\n', self.out(name))
                self.assertNotIn(b'target does not exist', self.out(name))
        for kind in ('explicit', 'implicit'):
            kept = self.out(f'trace-vpath-{kind}-keep')
            self.assertIn(b"update target 'results/x.out' due to:", kept)
            self.assertIn(b"printf 'done\\n'\ndone\n", kept)
            self.assertIn(b"update target 'x.out' due to:", self.out(f'trace-vpath-{kind}-discard'))

    def test_remake_suspension_live_reactivation_and_phase_order_are_anchored(self):
        self.assertNotIn(b"update target 'inc.mk'", self.out('trace-restart'))
        for flag in ('i', 'trace'):
            for initial in ('late', 'initial'):
                for state in ('virtual', 'created'):
                    out = self.out(f'trace-remake-reactivate-{flag}-{initial}-{state}')
                    self.assertEqual(b"update target 'inc.mk'" in out, initial == 'initial' or flag == 'trace')
                    self.assertEqual(b"update target 'all'" in out, initial == 'initial')
        self.assertTrue(self.out('trace-config-order').startswith(b'expand\nMakefile:4: update target'))
        self.assertTrue(self.out('trace-config-order').endswith(b'config\n:\n'))
        self.assertEqual(self.out('trace-expansion-fatal'), b'')
        self.assertIn(b'update target', self.out('trace-config-fatal'))
        self.assertTrue(self.out('trace-builtin').startswith(b'<builtin>: update target'))
        self.assertIn(b"raw\xff", self.out('trace-raw-normal'))

    def test_exact_stream_models_reject_lost_echo_wrong_identity_and_order(self):
        case = self.case('trace-two-lines-normal')
        out = self.out(case.id)
        good = conformance.Observation(out, b'', 0, None, {})
        self.assertEqual(conformance.expectation_errors(case, good), [])
        for mutation in (out.replace(b"printf 'first\\n'\n", b''),
                         out.replace(b'Makefile:4:', b'Makefile:5:'),
                         out.replace(b"target 'all'", b"target 'other'"),
                         b'\n'.join(reversed(out.rstrip(b'\n').split(b'\n'))) + b'\n'):
            self.assertTrue(conformance.expectation_errors(case, dataclasses.replace(good, stdout=mutation)))
        self.assertTrue(self.out('trace-recursion').startswith(b'Makefile:4:'))
        self.assertIn(b'child.mk:1:', self.out('trace-recursion'))
        self.assertIn(b' --trace --no-print-directory --warn-undefined-variables --no-silent', self.out('trace-metadata'))


if __name__ == '__main__':
    unittest.main()
