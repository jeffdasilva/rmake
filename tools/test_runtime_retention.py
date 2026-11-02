# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect independent retention controls and truthful allocation measurements."""
import contextlib
import dataclasses
import io
import json
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest
from unittest import mock

import allocations
import benchmark
import conformance as h

ROOT = Path(__file__).resolve().parents[1]


class RuntimeRetentionTests(unittest.TestCase):
    def test_inventory_and_all_raw_observations_preserve_the_models(self):
        proof = json.loads((ROOT / 'tests/research/runtime-argument-retention.json').read_text())
        family = next(r for r in tomllib.loads((ROOT / 'tests/requirements.toml').read_text())['requirements']
                      if r['id'] == 'performance.runtime-arguments')
        self.assertEqual((family['fixture_count'], family['candidate_required']), (10, 10))
        self.assertEqual(set(family['cases']), {r['id'] for r in proof['records']})
        for row in proof['records']:
            case = h.load_case(ROOT / 'tests/cases' / row['id'])
            self.assertEqual((case.args, case.environment, case.expected),
                             (tuple(row['args']), row['environment'], row['expected']))
            for name, value in row['inputs'].items():
                path = case.path / name
                self.assertEqual((path.stat().st_mode & 0o777, path.read_bytes()),
                                 (value['mode'], bytes.fromhex(value['hex'])))
            for build in ('baseline', 'debug', 'release'):
                r = row[build]
                self.assertEqual((r['model_errors'], r['candidate_differences'], r['oracle_repeat_differences'], r['bounds']),
                                 ([[], [], []], [], [], [False] * 3))

    def test_timed_inputs_observe_every_definition_goal_source_and_metadata_effect(self):
        workloads = list(benchmark.runtime_argument_workloads())
        self.assertEqual(workloads, list(benchmark.selected_workloads('runtime-arguments')))
        self.assertEqual(len(workloads), 6)
        for name, source in workloads:
            count, updates = int(name.split('-')[2]), int(name.rsplit('-', 1)[1])
            case = h.load_case(ROOT / 'tests/cases' / name)
            self.assertEqual((case.path / 'Makefile').read_bytes(), source)
            self.assertEqual(case.args, tuple(benchmark.workload_arguments(name)))
            self.assertEqual(source.count(b'MAKEFLAGS += -s'), updates)
            self.assertEqual(source.count(b'$(ARG_'), 2 * count)
            expected = bytes.fromhex(case.expected['stdout_hex'])
            self.assertEqual(expected.count(b'tick\n'), updates + 1)
            self.assertIn(b'GOALS=[all last]', expected)
            self.assertIn(b'AUX=[kept]', expected)
            good = h.Observation(expected, b'', 0, None, {})
            self.assertEqual(h.expectation_errors(case, good), [])
            for changed in (expected[5:], expected.replace(b'last', b'LOST'),
                            expected.replace(b'AUX=[kept]', b'AUX=[LOST]')):
                self.assertTrue(h.expectation_errors(case, dataclasses.replace(good, stdout=changed)))
            with tempfile.TemporaryDirectory() as temporary:
                path = Path(temporary)
                fixture = benchmark.prepare_files(path, name)
                self.assertEqual((fixture['metadata_constructions'], fixture['saved_definitions'], fixture['recipe_subprocesses']),
                                 (updates + 1, count, 0))
                for auxiliary in ('extra.mk', 'counter-$(info tick)/.keep'):
                    self.assertEqual((path / auxiliary).read_bytes(), (case.path / auxiliary).read_bytes())

    def test_raw_profiles_have_complete_matching_traces_and_repeated_counts(self):
        paths = list((ROOT / 'docs/benchmarks/runtime-retention-profiles').glob('*/report.json'))
        self.assertEqual(len(paths), 12)
        grouped = {}
        for path in paths:
            report = json.loads(path.read_text())
            summary, samples = allocations.parse_trace((path.parent / 'trace.txt').read_text())
            self.assertEqual((summary, samples), (report['summary'], report['samples']))
            case = h.load_case(ROOT / 'tests/cases' / report['workload'])
            self.assertEqual(tuple(report['arguments']), case.args)
            self.assertEqual(report['environment'], case.environment)
            self.assertIn('requested bytes are not live/peak', report['limits'])
            group = ('before' if '-before-' in path.parent.name else 'after', report['workload'])
            grouped.setdefault(group, []).append((summary, report['binary']['sha256'], report['helper_sha256']))
        self.assertEqual(len(grouped), 6)
        for values in grouped.values():
            self.assertEqual(len(values), 2)
            self.assertEqual(values[0], values[1])

    def test_profile_execution_keeps_required_auxiliary_inputs_and_exact_output(self):
        for workload in ('runtime-arguments-0-updates-0', 'runtime-arguments-512-updates-100'):
            case = h.load_case(ROOT / 'tests/cases' / workload)
            expected = bytes.fromhex(case.expected['stdout_hex'])
            for broken in (False, True):
                with self.subTest(workload=workload, broken=broken), tempfile.TemporaryDirectory() as temporary:
                    def command(argv, **kwargs):
                        if argv[0] == 'cc':
                            return subprocess.CompletedProcess(argv, 0, b'', b'')
                        self.assertEqual(tuple(argv[1:]), case.args)
                        for name in ('extra.mk', 'counter-$(info tick)/.keep'):
                            self.assertEqual((Path(kwargs['cwd']) / name).read_bytes(), (case.path / name).read_bytes())
                        Path(kwargs['env']['RMAKE_ALLOC_TRACE']).write_text('summary calls=1 requested_bytes=2 frees=1\n')
                        return subprocess.CompletedProcess(argv, 0, expected[5:] if broken else expected, b'')
                    with mock.patch.object(h, 'executable_identity', return_value={'path': '/candidate', 'sha256': 'test'}), \
                         mock.patch.object(allocations.subprocess, 'run', side_effect=command), \
                         contextlib.redirect_stdout(io.StringIO()):
                        if broken:
                            with self.assertRaises(ValueError):
                                allocations.run(Path('/candidate'), workload, Path(temporary))
                        else:
                            allocations.run(Path('/candidate'), workload, Path(temporary))

    def test_profile_cannot_report_when_an_auxiliary_source_changes(self):
        workload = 'runtime-arguments-0-updates-0'
        case = h.load_case(ROOT / 'tests/cases' / workload)
        prepare = benchmark.prepare_files
        def changed(path, name):
            result = prepare(path, name)
            (path / 'extra.mk').write_bytes(b'wrong\n')
            return result
        def command(argv, **kwargs):
            if argv[0] != 'cc':
                Path(kwargs['env']['RMAKE_ALLOC_TRACE']).write_text('summary calls=1 requested_bytes=2 frees=1\n')
            return subprocess.CompletedProcess(argv, 0, bytes.fromhex(case.expected['stdout_hex']), b'')
        with tempfile.TemporaryDirectory() as temporary, \
             mock.patch.object(h, 'executable_identity', return_value={'path': '/candidate', 'sha256': 'test'}), \
             mock.patch.object(allocations.subprocess, 'run', side_effect=command), \
             mock.patch.object(benchmark, 'prepare_files', side_effect=changed), \
             self.assertRaisesRegex(ValueError, 'auxiliary input'):
            allocations.run(Path('/candidate'), workload, Path(temporary))

    def test_warning_phase_discovery_has_no_compatibility_or_safety_credit(self):
        proof = json.loads((ROOT / 'tests/research/live-warning-phase-discovery.json').read_text())
        self.assertIn('zero conformance', proof['classification'])
        self.assertEqual(len(proof['records']), 3)
        for row in proof['records']:
            self.assertEqual(row['bounds'], [False] * 3)
            self.assertEqual(row['oracle_repeat_differences'], [])
            self.assertEqual(row['candidate_differences'], ['stderr'])
            self.assertFalse((ROOT / 'tests/cases' / row['id']).exists())
        late = next(r for r in proof['records'] if r['id'].endswith('-late'))
        self.assertEqual(late['model_errors'][:2], [[], []])
        for row in proof['records']:
            if row is not late:
                self.assertTrue(all(row['model_errors'][:2]))


if __name__ == '__main__':
    unittest.main()
