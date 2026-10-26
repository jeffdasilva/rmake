# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Independent anchors must reject equal wrong -L timestamp fault outcomes."""
import ast
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import conformance
import stdio_faults as faults
import symlink_times as runner


class SymlinkTimeTests(unittest.TestCase):
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
            status = runner.run_campaign((cell,), Path('/oracle'), Path('/candidate'), artifacts,
                                         {'oracle': {'path': '/oracle'}, 'candidate': {'path': '/candidate'}},
                                         ['replay'])
        report = json.loads(next((artifacts / 'campaigns').glob('*.json')).read_text())
        return status, report, calls

    def test_inventory_covers_operations_errors_and_four_modes(self):
        self.assertEqual(len(self.cells), 112)
        self.assertEqual(len({cell.id for cell in self.cells}), 112)
        self.assertEqual(self.cells, runner.inventory())
        self.assertEqual({cell.status for cell in self.cells}, {0, 1, 2})
        self.assertEqual({cell.flags for cell in self.cells}, {(), ('-n',), ('-q',), ('-t',)})
        self.assertIn('readlink-enoent-query', {cell.id for cell in self.cells})
        self.assertIn('stat-enotdir-normal', {cell.id for cell in self.cells})
        for path in (Path(runner.__file__), Path(__file__)):
            ast.parse(path.read_text(), feature_version=(3, 11))

    def test_materialization_keeps_injection_local_and_preserves_real_symlink(self):
        case = runner.materialize(self.cells[0], self.root / 'inputs', self.root / 'helper.so')
        self.assertEqual(case.args, runner.ARGS)
        self.assertEqual(case.timeout, 5)
        self.assertEqual(case.candidate, 'required')
        self.assertEqual(case.environment, {'LD_PRELOAD': str(self.root / 'helper.so'),
                                           'RMAKE_STDIO_FAULT': self.cells[0].fault})
        self.assertEqual((case.path / 'effect-probe').readlink(), Path('input'))
        self.assertEqual((case.path / 'input').read_bytes(), b'input\n')
        self.assertEqual((case.path / 'out').read_bytes(), b'old\n')
        self.assertEqual((case.path / 'setup.py').read_bytes(), runner.SETUP)
        self.assertFalse((case.path / 'fault.log').exists())

    def test_default_stat_cells_disable_link_walk_and_anchor_retry_and_missing(self):
        cells = [cell for cell in self.cells if not cell.symlinks and not cell.library]
        self.assertEqual(len(cells), 20)
        for cell in cells:
            case = runner.materialize(cell, self.root / 'default', self.root / 'helper.so')
            self.assertNotIn('-L', case.args)
            self.assertEqual(case.args, runner.ARGS[:-1] + cell.flags)
            self.assertEqual(runner.expected(cell).filesystem['work/fault.log'],
                             faults.regular_file(b'symlink-fault\n'))
            if 'eintr' in cell.id:
                self.assertEqual(cell.status, 0)
                self.assertEqual(cell.stderr, b'')
            else:
                self.assertEqual(cell.status, 2)
                self.assertIn(b'No rule to make target', cell.stderr)
        self.assertEqual(sum(cell.symlinks and not cell.library for cell in self.cells), 52)

    def test_library_fault_programs_preserve_flags_and_protected_input_anchors(self):
        cells = [cell for cell in self.cells if cell.library]
        self.assertEqual(len(cells), 40)
        self.assertEqual(sum(cell.symlinks for cell in cells), 20)
        for cell in cells:
            case = runner.materialize(cell, self.root / 'libraries', self.root / 'helper.so')
            self.assertEqual((case.path / 'Makefile').read_bytes(), runner.LIBRARY_SOURCE)
            self.assertEqual('-L' in case.args, cell.symlinks)
            self.assertEqual(runner.expected(cell).filesystem['work/Makefile'],
                             faults.regular_file(runner.LIBRARY_SOURCE))
            self.assertEqual(runner.expected(cell).filesystem['work/protected.txt'],
                             faults.regular_file(runner.PROTECTED))
            if cell.status == 2:
                self.assertIn(b"target '-leffect-probe'", cell.stderr)

    def test_success_uses_same_workspace_and_retains_helper_and_anchors(self):
        status, report, calls = self.campaign()
        self.assertEqual(status, 0)
        self.assertEqual(calls[0], calls[1])
        self.assertEqual(report['executed'], 1)
        self.assertEqual(report['helper'], {'test': True})
        self.assertEqual(report['cases'][0]['expected'], conformance.observation_json(runner.expected(self.cells[0])))

    def test_equal_wrong_streams_status_limits_and_children_fail(self):
        for key, value in [('stdout', b'wrong'), ('stderr', b'wrong'), ('returncode', 99),
                           ('limit', 'timeout'), ('leftover_processes', True)]:
            with self.subTest(key=key):
                status, report, _ = self.campaign(lambda o: setattr(o, key, value))
                self.assertEqual(status, 1)
                self.assertEqual(report['failed'], 1)

    def test_journal_links_and_protected_content_cannot_silently_change(self):
        changes = [lambda o: o.filesystem.pop('work/fault.log'),
                   lambda o: o.filesystem['work/fault.log'].update(content='d3JvbmcK'),
                   lambda o: o.filesystem['work/effect-probe'].update(target='wrong'),
                   lambda o: o.filesystem['work/input'].update(content='d3JvbmcK'),
                   lambda o: o.filesystem.pop('work/protected.txt')]
        for change in changes:
            status, report, _ = self.campaign(change)
            self.assertEqual(status, 1)
            artifact = Path(report['cases'][0]['artifact'])
            self.assertEqual((artifact / faults.SOURCE.name).read_bytes(), faults.SOURCE.read_bytes())
            self.assertTrue((artifact / 'symlink.json').is_file())
            self.assertEqual(report['cases'][0]['replay_argv'], ['replay', '--case', self.cells[0].id])
        self.assertEqual(runner.expected(self.cells[0]).filesystem['work/effect-probe']['target'], 'input')

    def test_unknown_case_is_rejected_before_any_engine_runs(self):
        with contextlib.redirect_stderr(io.StringIO()), mock.patch.object(conformance, 'execute') as execute:
            self.assertEqual(runner.main(['--self-test', '--case', 'missing']), 2)
        execute.assert_not_called()
