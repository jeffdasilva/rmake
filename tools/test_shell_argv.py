# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect shell replay anchors and retain unsafe oracle research separately."""

import dataclasses
import json
from pathlib import Path
import tomllib
import unittest

import conformance

ROOT = Path(__file__).resolve().parent.parent


class ShellArgvTests(unittest.TestCase):
    def test_closed_shell_families_remain_required(self):
        ledger = tomllib.loads((ROOT / 'tests/requirements.toml').read_text())
        for family in ledger['requirements']:
            if family['id'] in ('process.invocation', 'recipes.oneshell'):
                self.assertEqual(family['candidate_required'], family['fixture_count'])
                for name in family['cases']:
                    self.assertEqual(conformance.load_case(ROOT / 'tests/cases' / name).candidate,
                                     'required')
        for name in ('fs-file-oneshell-quoted-read', 'oneshell-custom-shell-quoted-flags',
                     'oneshell-quoted-direct-tail-replay',
                     'process-shell-literal-newline-quoted-flags',
                     'process-shell-literal-newline-simple-flags'):
            self.assertEqual(conformance.load_case(ROOT / 'tests/cases' / name).candidate,
                             'required')

    def test_execution_and_dry_run_echo_have_distinct_byte_anchors(self):
        anchors = {
            'shell-argv-tail-0-c-0': (b'first\nsecond\n', 2),
            'shell-argv-tail-0-c-1': (b"printf '%s\\n' 'first\nsecond'\n", 0),
            'shell-argv-forced-0-dry': (b"printf '%s\\n' 'first\nfirst\nsecond\nsecond'\n", 2),
        }
        for name, (stdout, status) in anchors.items():
            with self.subTest(case=name):
                case = conformance.load_case(ROOT / 'tests/cases' / name)
                self.assertEqual(bytes.fromhex(case.expected['stdout_hex']), stdout)
                self.assertEqual(case.expected['exit_code'], status)
                observation = conformance.Observation(
                    stdout, bytes.fromhex(case.expected['stderr_hex']), status, None, {})
                self.assertEqual(conformance.expectation_errors(case, observation), [])
                wrong = dataclasses.replace(observation, stdout=stdout + b'extra\n')
                self.assertIn('oracle disagrees with expected.stdout_hex',
                              conformance.expectation_errors(case, wrong))

    def test_allocator_aborts_remain_raw_research_without_conformance_credit(self):
        record = json.loads((ROOT / 'tests/research/gnu-441-shell-argv-aborts.json').read_text())
        self.assertEqual(record['classification'],
                         'oracle allocator-abort research, not passing conformance fixtures')
        self.assertEqual(len(record['records']), 5)
        for row in record['records']:
            self.assertFalse((ROOT / 'tests/cases' / row['id']).exists())
            self.assertEqual(len(row['oracle_runs']), 2)
            self.assertTrue(all(run['exit_status'] == -6 for run in row['oracle_runs']))
            self.assertIn(row['candidate']['exit_status'], (0, 2))
            for observation in [*row['oracle_runs'], row['candidate']]:
                self.assertIsNone(observation['limit'])
                self.assertFalse(observation['leftover_processes'])
                bytes.fromhex(observation['stdout_hex'])
                bytes.fromhex(observation['stderr_hex'])


if __name__ == '__main__':
    unittest.main()
