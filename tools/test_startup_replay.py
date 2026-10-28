# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original restart state, effect and stream-order model anchors."""
import dataclasses
from pathlib import Path
import tomllib
import unittest
import conformance

ROOT = Path(__file__).resolve().parents[1]


class StartupReplayTests(unittest.TestCase):
    def case(self, suffix):
        return conformance.load_case(ROOT / 'tests/cases' / ('startup-replay-' + suffix))

    def out(self, suffix):
        return bytes.fromhex(self.case(suffix).expected['stdout_hex'])

    def err(self, suffix):
        return bytes.fromhex(self.case(suffix).expected['stderr_hex'])

    def test_required_profile_matrix_and_exact_historical_reproducer(self):
        family = next(r for r in tomllib.loads((ROOT / 'tests/requirements.toml').read_text())['requirements'] if r['id'] == 'cli.startup-replay')
        self.assertEqual(family['fixture_count'], 67)
        self.assertEqual(family['candidate_required'], 67)
        self.assertEqual(len(set(family['cases'])), 67)
        for profile in ('defaults', 'r', 'big-r', 'r-big-r'):
            for operation in ('file-effect', 'restart-value', 'append', 'append-simple', 'conditional', 'name-effects', 'computed-name', 'include'):
                self.assertEqual(self.case(profile + '-' + operation).candidate, 'required')
        old = conformance.load_case(ROOT / 'tests/cases/cli-assignment-restart-effect')
        self.assertEqual(old.candidate, 'required')
        self.assertEqual(bytes.fromhex(old.expected['stdout_hex']), b'[]/[1]\n[1]/[2]\n')

    def test_replay_reconstructs_names_conditionals_and_effect_counts(self):
        self.assertEqual(self.out('defaults-append'), b'[]/[piece]/[1]\n[1]/[piece]/[2]\n')
        self.assertEqual(self.out('defaults-conditional'), b'[]/[piece]/[1]\n[1]/[piece]/[3]\n')
        self.assertEqual(self.out('defaults-name-effects'), b'[]/[value]/[once]/[1]\n[1]/[value]/[once]/[2]\n')
        self.assertEqual(self.out('defaults-computed-name'), b'[]/[BEFORE]/[command line]/[undefined]\n[1]/[AFTER]/[undefined]/[command line]\n')
        self.assertEqual(self.out('two-restarts'), b'[]/[]/[1]\n[1]/[1]/[2]\n[2]/[2]/[3]\n')
        for origin in ('cli', 'environment', 'environment-override'):
            self.assertEqual(self.out('conditional-' + origin), b'[]/[first]/[1]/[]\n[1]/[first]/[2]/[]\n')

    def test_replay_observes_fresh_files_wildcards_and_include_search(self):
        self.assertEqual(self.out('fresh-file'), b'[]/[first]\n[1]/[second]\n')
        self.assertEqual(self.out('fresh-shell'), b'[]/[first]\n[1]/[second]\n')
        self.assertEqual(self.out('fresh-wildcard'), b'[]/[a.in]\n[1]/[a.in b.in]\n')
        self.assertEqual(self.out('include-dirs'), b'[]/[before]\n[1]/[after]\n')
        case = self.case('raw-file')
        good = conformance.Observation(self.out('raw-file'), b'', 0, None, {})
        self.assertEqual(good.stdout, b'[]/[\xff]\n[1]/[\xfe]\n')
        self.assertEqual(conformance.expectation_errors(case, good), [])
        self.assertTrue(conformance.expectation_errors(case, dataclasses.replace(good, stdout=b'[]/[\xff]\n[1]/[\xff]\n')))

    def test_errors_and_warnings_keep_identity_and_merged_order(self):
        for mode in ('none', 'line', 'target', 'recurse'):
            self.assertEqual(self.err('error-' + mode), b'make: *** SECOND.  Stop.\n')
            self.assertEqual(self.out('error-' + mode), b'FIRST\n[]\n')
            merged = self.case('merged-order-' + mode)
            self.assertEqual(merged.output, 'merged')
            self.assertEqual(self.out('merged-order-' + mode), b'FIRST\nmake: WARN\nREAD\nFIRST\nmake: WARN\nREAD\n')
        self.assertEqual(self.err('include-error'), b'early.mk:1: *** SECOND.  Stop.\n')
        self.assertEqual(self.err('startup-warning-without-assignments'), b"make: warning: undefined variable 'MAKEFLAGS'\n" * 2)

    def test_imported_depth_early_shell_and_eval_have_separate_phases(self):
        self.assertEqual(self.out('imported-level'), b'[]/[7]/[12]/[1]\n[1]/[12]/[12]/[2]\n')
        self.assertEqual(self.out('early-shell-level'), b'[]/[1]/[7]\n[1]/[1]/[7]\n')
        self.assertEqual(self.out('shell-unbound'), b'[]/[]/[127]\n[1]/[]/[127]\n')
        self.assertEqual(self.err('shell-unbound'), b'make: -c: No such file or directory\n' * 2)
        self.assertEqual(self.out('recursive-command'), b'P=[]/[]\nP=[1]/[1]\nC=[1]\n')
        self.assertEqual(self.out('cli-before-eval'), b'E=[]/[]\nF=[]/[]\nE=[1]/[1]\nF=[1]/[1]\n')


if __name__ == '__main__':
    unittest.main()
