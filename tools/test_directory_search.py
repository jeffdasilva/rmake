# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""The directory-search gate rejects agreement on incorrectly observed effects."""
import ast
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import conformance
import directory_search as runner
import stdio_faults as faults


class DirectorySearchTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.cells = runner.inventory()
        self.identities = {'oracle': {'path': '/oracle'}, 'candidate': {'path': '/candidate'}}

    def campaign(self, cell, mutate=None):
        calls = []
        def execute(case, root, binary):
            calls.append(root)
            observed = runner.expected(cell)
            if mutate:
                mutate(observed)
            return observed
        artifacts = self.root / ('artifacts-' + str(len(list(self.root.iterdir()))))
        with mock.patch.object(faults, 'compile_helper', return_value=(self.root / 'helper.so', {'test': True})), \
             mock.patch.object(conformance, 'execute', side_effect=execute), \
             contextlib.redirect_stdout(io.StringIO()) as output:
            status = runner.run_campaign((cell,), Path('/oracle'), Path('/candidate'),
                                         artifacts, self.identities, ['replay'])
        report = json.loads(next((artifacts / 'campaigns').glob('*.json')).read_text())
        return status, report, calls, output.getvalue()

    def test_inventory_is_original_bounded_and_reproducible(self):
        self.assertEqual(len(self.cells), 176)
        self.assertEqual(len({cell.id for cell in self.cells}), len(self.cells))
        self.assertEqual(self.cells, runner.inventory())
        self.assertTrue(all(len(cell.source) < 2048 for cell in self.cells))
        self.assertEqual({cell.status for cell in self.cells}, {0, 1, 2})
        self.assertTrue(all(cell.trace is not None or cell.id.startswith('io-') for cell in self.cells))
        for key in ('stream-partial', 'stream-complete', 'stream-limit-and-exec',
                    'io-open-eacces', 'io-read-eintr', 'io-read-tail-eio',
                    'io-group-completion-keeps-local-observation'):
            self.assertIn(key, {cell.id for cell in self.cells})

    def test_scripts_are_python_311_compatible(self):
        for path in (Path(runner.__file__), Path(__file__)):
            ast.parse(path.read_text(), feature_version=(3, 11))

    def test_materialization_scopes_injection_and_preserves_link_and_bytes(self):
        cell = next(cell for cell in self.cells if cell.id == 'vpath-alias-b-cached-normal')
        case = runner.materialize(cell, self.root / 'inputs', self.root / 'helper.so')
        self.assertEqual(case.args, runner.ARGS)
        self.assertEqual(case.timeout, 5)
        self.assertEqual(case.candidate, 'required')
        self.assertEqual(case.environment, {'LD_PRELOAD': str(self.root / 'helper.so'),
                                           'RMAKE_STDIO_FAULT': 'wildcard-alias'})
        self.assertEqual((case.path / 'Makefile').read_bytes(), cell.source)
        self.assertEqual((case.path / 'd/a.t').read_bytes(), b'old\n')
        self.assertEqual((case.path / 'link').readlink(), Path('d'))
        self.assertFalse((case.path / 'fault.log').exists())
        self.assertFalse((case.path / 'd/b.t').exists())

    def test_success_compares_same_workspace_and_retains_independent_anchors(self):
        cell = self.cells[0]
        status, report, calls, output = self.campaign(cell)
        self.assertEqual(status, 0)
        self.assertEqual(calls[0], calls[1])
        self.assertEqual(report['identities'], self.identities)
        self.assertEqual(report['executed'], 1)
        self.assertEqual(report['cases'][0]['stdout_hex'], cell.stdout.hex())
        self.assertEqual(report['helper'], {'test': True})
        self.assertIn('pass=1, fail=0', output)

    def test_equal_wrong_streams_status_limits_and_leftovers_fail(self):
        for field, value in (('stdout', b'wrong'), ('stderr', b'wrong'), ('returncode', 99),
                             ('limit', 'timeout'), ('leftover_processes', True)):
            with self.subTest(field=field):
                status, report, _, _ = self.campaign(self.cells[0], lambda obs: setattr(obs, field, value))
                self.assertEqual(status, 1)
                self.assertEqual(report['failed'], 1)

    def test_equal_wrong_journal_link_and_nested_contents_fail_without_mutating_inventory(self):
        cell = next(cell for cell in self.cells if cell.id == 'vpath-alias-b-cached-normal')
        original = runner.inventory()
        mutations = [
            lambda obs: obs.filesystem.pop('work/fault.log'),
            lambda obs: obs.filesystem['work/link'].update(target='wrong'),
            lambda obs: obs.filesystem['work/d/b.t'].update(content='d3JvbmcK'),
            lambda obs: obs.filesystem.pop('work/protected.txt'),
        ]
        for mutate in mutations:
            status, report, _, _ = self.campaign(cell, mutate)
            self.assertEqual(status, 1)
            artifact = Path(report['cases'][0]['artifact'])
            self.assertTrue((artifact / 'directory.json').is_file())
            self.assertEqual((artifact / faults.SOURCE.name).read_bytes(), faults.SOURCE.read_bytes())
            self.assertEqual(report['cases'][0]['replay_argv'], ['replay', '--case', cell.id])
        self.assertEqual(self.cells, original)

    def test_partial_and_limit_proofs_cannot_be_omitted_or_replaced(self):
        for name in ('stream-partial', 'stream-limit-and-exec'):
            cell = next(cell for cell in self.cells if cell.id == name)
            status, _, _, _ = self.campaign(cell, lambda obs: obs.filesystem.update(
                {'work/fault.log': faults.regular_file(b'unseen\n')}))
            self.assertEqual(status, 1)
        cell = next(cell for cell in self.cells if cell.id == 'io-group-skips-unused-peer')
        status, _, _, _ = self.campaign(cell, lambda obs: obs.filesystem.update(
            {'work/fault.log': faults.regular_file(b'read-fault\n')}))
        self.assertEqual(status, 1)

    def test_unknown_case_fails_before_running_any_engine(self):
        with contextlib.redirect_stderr(io.StringIO()) as output, \
             mock.patch.object(conformance, 'execute') as execute:
            self.assertEqual(runner.main(['--self-test', '--case', 'not-a-case']), 2)
        execute.assert_not_called()
        self.assertIn('unknown directory-search cases', output.getvalue())
