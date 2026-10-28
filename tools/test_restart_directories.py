# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect independent namespace, invocation, and byte diagnostic observations."""
import dataclasses
import itertools
import json
from pathlib import Path
import tomllib
import unittest
import conformance

ROOT = Path(__file__).resolve().parent.parent


class RestartDirectoryTests(unittest.TestCase):
    def case(self, name):
        return conformance.load_case(ROOT / 'tests/cases' / name)

    def test_namespace_profiles_and_invocation_origins_remain_required(self):
        ledger = tomllib.loads((ROOT / 'tests/requirements.toml').read_text())
        family = next(row for row in ledger['requirements'] if row['id'] == 'recursion.restart-cwd')
        self.assertEqual(family['candidate_required'], family['fixture_count'])
        self.assertGreaterEqual(family['fixture_count'], 348)
        for name in family['cases']:
            self.assertEqual(self.case(name).candidate, 'required')
        for variant, style, level, jobs, topology in itertools.product(
                ('plain', 'sub', 'nested', 'retarget', 'broken', 'origfile'),
                ('normal', 'changed', 'custom-curdir'), (0, 2),
                ('serial', 'pipe', 'pipe3', 'fifo'), ('separate', 'merged')):
            name = f'restart-namespace-{variant}-{style}-l{level}-{jobs}-{topology}'
            self.assertIn(name, family['cases'])
            self.assertEqual(self.case(name).output, topology)
        for profile in range(4):
            for origin in ('default', 'env-make', 'env-command', 'env-make-e', 'env-command-e',
                           'cli-make', 'cli-command', 'file-make', 'file-command',
                           'simple-make', 'simple-command', 'undef-command'):
                self.assertIn(f'make-command-meta-{profile}-{origin}', family['cases'])

    def test_relative_invocation_retains_recursive_default_and_reaches_child(self):
        for mode in ('plain', 'sub', 'nested', 'symlink'):
            for level in (0, 1):
                case = self.case(f'make-command-relative-{mode}-{level}')
                stdout = bytes.fromhex(case.expected['stdout_hex'])
                self.assertEqual(stdout, b'make=[recursive]/[default]/[$(MAKE_COMMAND)]/[make] '
                                         b'command=[simple]/[default]\n'
                                         b'command-path=absolute\nCHILD\n')
                good = conformance.Observation(stdout, b'', 0, None, {})
                self.assertEqual(conformance.expectation_errors(case, good), [])
                for wrong in (stdout.replace(b'[recursive]', b'[simple]'),
                              stdout.replace(b'CHILD\n', b'')):
                    self.assertTrue(conformance.expectation_errors(
                        case, dataclasses.replace(good, stdout=wrong)))

    def test_no_c_and_retargeted_c_have_distinct_namespace_anchors(self):
        plain = self.case('restart-namespace-plain-normal-l0-serial-separate')
        changed = self.case('restart-namespace-retarget-normal-l0-serial-separate')
        self.assertEqual(bytes.fromhex(plain.expected['stdout_hex']),
                         b'phase=old:work::old\nphase=old:saved:1:old\nOLD:old\n')
        self.assertEqual(bytes.fromhex(changed.expected['stdout_hex']),
                         b'phase=old:sub::old\nphase=new:other:1:new:new.in\nNEW:new\n')
        custom = self.case('restart-namespace-retarget-custom-curdir-l0-serial-separate')
        self.assertIn(b'phase=new:client:1:new:new.in\n',
                      bytes.fromhex(custom.expected['stdout_hex']))
        # CURDIR can be hidden by a CLI binding; the generated file also observes
        # abspath's actual namespace and is compared by the differential runner.
        self.assertIn(b'$(file >where,$(abspath .))', (custom.path / 'sub/next.mk').read_bytes())

    def test_raw_failure_bytes_and_open_cli_usage_gap_are_preserved(self):
        case = self.case('make-command-path-space-missing')
        stderr = bytes.fromhex(case.expected['stderr_hex'])
        self.assertIn(b'dir space\xff: No such file or directory', stderr)
        good = conformance.Observation(b'', stderr, 2, None, {})
        self.assertEqual(conformance.expectation_errors(case, good), [])
        self.assertTrue(conformance.expectation_errors(case, dataclasses.replace(
            good, stderr=stderr.replace(b'\xff', b'?'))))
        failure = self.case('restart-namespace-origfile-normal-l0-pipe3-separate')
        self.assertEqual(failure.expected['exit_code'], 2)
        self.assertIn(b'Couldn\'t change back to original directory.  Stop.\n',
                      bytes.fromhex(failure.expected['stderr_hex']))
        self.assertIn(b'should be 3!\n', bytes.fromhex(failure.expected['stderr_hex']))
        research = json.loads((ROOT / 'tests/research/cli-empty-directory.json').read_text())
        self.assertEqual(research['classification'],
                         'open CLI usage-output mismatch; not passing conformance')
        self.assertFalse((ROOT / 'tests/cases/make-command-path-empty').exists())
        self.assertEqual(research['oracle_runs'][0]['stderr'],
                         research['oracle_runs'][1]['stderr'])
        self.assertNotEqual(research['oracle_runs'][0]['stderr'], research['candidate']['stderr'])


if __name__ == '__main__':
    unittest.main()
