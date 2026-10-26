# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Archive anchors must reject matching wrong engines and protect input bytes."""
import ast
import contextlib
import io
import json
from pathlib import Path
import tempfile
import sys
import unittest
from unittest import mock

import archive_times as runner
import conformance
import stdio_faults as faults


class ArchiveTimeTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.cells = runner.inventory()

    def campaign(self, mutate=None):
        cell = self.cells[0]
        calls = []
        def execute(case, root, binary):
            calls.append(root)
            observation = runner.expected(cell)
            if mutate:
                mutate(observation)
            return observation
        artifacts = self.root / ('run-' + str(len(list(self.root.iterdir()))))
        with mock.patch.object(faults, 'compile_helper', return_value=(self.root / 'helper.so', {'test': True})), \
             mock.patch.object(conformance, 'execute', side_effect=execute), \
             contextlib.redirect_stdout(io.StringIO()):
            status = runner.run((cell,), Path('/oracle'), Path('/candidate'), artifacts,
                                {'oracle': {'path': '/oracle'}, 'candidate': {'path': '/candidate'}}, ['replay'])
        report = json.loads(next((artifacts / 'campaigns').glob('*.json')).read_text())
        return status, report, calls

    def test_inventory_covers_paths_modes_phases_and_original_inputs(self):
        self.assertEqual(len(self.cells), 2600)
        self.assertEqual(len({cell.id for cell in self.cells}), len(self.cells))
        self.assertEqual(self.cells, runner.inventory())
        self.assertEqual({cell.variant for cell in self.cells}, {'local', 'vpath', 'gpath', 'declared'})
        self.assertEqual({cell.mode for cell in self.cells}, {'normal', 'dry', 'query', 'touch'})
        self.assertEqual(sum(cell.intermediate for cell in self.cells), 64)
        self.assertEqual(sum(cell.hidden for cell in self.cells), 32)
        self.assertEqual(sum(cell.operation.startswith('touch-') for cell in self.cells), 28)
        self.assertEqual(sum(cell.operation.startswith('query-') for cell in self.cells), 28)
        for path in (Path(runner.__file__), Path(__file__)):
            ast.parse(path.read_text(), feature_version=(3, 11))
        for cell in self.cells:
            self.assertLessEqual(len(cell.id), 80)
            self.assertEqual(runner.expected(cell).filesystem['work/protected.txt'], faults.regular_file(runner.PROTECTED))

    def test_stat_issues_precede_fatal_headers_and_missing_target_names(self):
        for cell in self.cells:
            if cell.operation != 'stat' or cell.kind != 'eio':
                continue
            expected = runner.expected(cell)
            self.assertTrue(expected.stderr.startswith(b'make: stat: effect-probe: Input/output error\n'))
            if cell.bad:
                self.assertEqual(expected.returncode, 2)
                self.assertLess(expected.stderr.index(b'stat:'), expected.stderr.index(b'Invalid date'))
            elif cell.newer and cell.role == 'member':
                self.assertNotEqual(expected.returncode, 2)

    def test_open_failure_skips_invalid_header_and_seek_interruptions_retry(self):
        for cell in self.cells:
            expected = runner.expected(cell)
            if cell.operation == 'open':
                self.assertEqual(expected.returncode, 2)
                self.assertIn(b'No rule to make target', expected.stderr)
                self.assertNotIn(b'Invalid date', expected.stderr)
                self.assertEqual(expected.filesystem['work/fault.log'], faults.regular_file(b'interrupted\n'))
            if cell.operation == 'seek' and cell.kind == 'eintr' and not cell.bad:
                self.assertNotEqual(expected.returncode, 2)
                self.assertEqual(expected.stderr, b'')

    def test_touch_and_query_continuations_anchor_bogus_warning_and_normalization(self):
        for cell in self.cells:
            if not cell.operation.startswith(('touch-', 'query-')):
                continue
            expected = runner.expected(cell)
            self.assertEqual(expected.filesystem['work/arm.marker'], faults.regular_file(b'armed'))
            self.assertIn('normalize.py', runner.inputs(cell))
            if cell.operation.startswith('query-') or cell.kind in ('eio', 'eof'):
                self.assertIn(b'may be bogus; not deleted', expected.stderr)
                self.assertIn(b'changed=0\n', expected.stdout)
            else:
                self.assertEqual(expected.stdout.count(b'touch effect-probe(member)\n'), 2)
                self.assertIn(b'changed=1\n', expected.stdout)

    def test_materialization_keeps_faults_local_and_preserves_fixed_archive(self):
        for cell in (self.cells[0], next(c for c in self.cells if c.operation == 'touch-read-next')):
            case = runner.materialize(cell, self.root / 'inputs', self.root / 'helper.so')
            self.assertEqual(case.timeout, 5)
            self.assertEqual(case.candidate, 'required')
            self.assertEqual(case.environment['RMAKE_STDIO_FAULT'], cell.fault)
            self.assertEqual(case.environment['LD_PRELOAD'], str(self.root / 'helper.so'))
            self.assertEqual((case.path / 'protected.txt').read_bytes(), runner.PROTECTED)
            archive, member, physical = runner.names(cell)
            self.assertEqual((case.path / physical).read_bytes(), runner.archive_bytes(member, cell.bad, cell.operation.endswith('read-next')))
            self.assertFalse((case.path / 'fault.log').exists())
            if cell.operation.startswith('touch-'):
                self.assertEqual(case.environment['RMAKE_ARCHIVE_ARMED'], '1')
                self.assertEqual(case.args[-2:], (f'{archive}({member})', 'verify'))

    def test_matching_wrong_streams_status_limits_and_inputs_fail_independent_anchors(self):
        changes = [lambda o: setattr(o, 'stdout', b'wrong'),
                   lambda o: setattr(o, 'stderr', b'wrong'),
                   lambda o: setattr(o, 'returncode', 99),
                   lambda o: setattr(o, 'limit', 'timeout'),
                   lambda o: setattr(o, 'leftover_processes', True),
                   lambda o: o.filesystem.pop('work/fault.log'),
                   lambda o: o.filesystem['work/archive.a'].update(content='d3JvbmcK'),
                   lambda o: o.filesystem.pop('work/protected.txt')]
        for change in changes:
            status, report, calls = self.campaign(change)
            self.assertEqual(status, 1)
            self.assertEqual(report['failed'], 1)
            self.assertEqual(calls[0], calls[1])
            artifact = Path(report['cases'][0]['artifact'])
            self.assertEqual((artifact / faults.SOURCE.name).read_bytes(), faults.SOURCE.read_bytes())
            self.assertEqual(report['cases'][0]['replay_argv'], ['replay', '--case', self.cells[0].id])

    def test_success_retains_identity_helper_input_and_anchor_evidence(self):
        status, report, calls = self.campaign()
        self.assertEqual(status, 0)
        self.assertIs(runner.harness, sys.modules['conformance'])
        self.assertIs(faults.conformance, runner.harness)
        self.assertEqual(report['executed'], 1)
        self.assertEqual(report['helper'], {'test': True})
        self.assertEqual(calls[0], calls[1])
        self.assertEqual(report['cases'][0]['expected'], conformance.observation_json(runner.expected(self.cells[0])))

    def test_unknown_replay_case_is_rejected_before_running_an_engine(self):
        with contextlib.redirect_stderr(io.StringIO()), mock.patch.object(conformance, 'execute') as execute:
            self.assertEqual(runner.main(['--self-test', '--case', 'missing']), 2)
        execute.assert_not_called()


if __name__ == '__main__':
    unittest.main()
