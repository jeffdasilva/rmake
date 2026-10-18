#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Fault-campaign anchors reject equal wrong results and retain replay evidence."""
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


class StdioFaultTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.artifacts = self.root / 'artifacts'
        self.identities = {'oracle': {'path': '/oracle'}, 'candidate': {'path': '/candidate'}}
        self.cells = faults.inventory()

    def campaign(self, *, cell=None, mutate=None):
        cell = cell or self.cells[0]
        roots = []

        def execute(case, root, binary):
            roots.append(root)
            result = faults.expected(cell, root)
            if mutate:
                mutate(result)
            return result

        with mock.patch.object(faults, 'compile_helper', return_value=(self.root / 'helper.so', {'test': True})), \
             mock.patch.object(conformance, 'execute', side_effect=execute), \
             contextlib.redirect_stdout(io.StringIO()) as output:
            status = faults.run_campaign((cell,), Path('/oracle'), Path('/candidate'),
                                          self.artifacts, self.identities, ['replay'])
        report = json.loads(next((self.artifacts / 'campaigns').glob('*.json')).read_text())
        return status, output.getvalue(), report, roots

    def test_scripts_support_python_311(self):
        for path in (Path(faults.__file__), Path(__file__)):
            ast.parse(path.read_text(), feature_version=(3, 11))

    def test_inventory_covers_each_stage_and_is_bounded(self):
        self.assertEqual(len(self.cells), 11)
        self.assertEqual(len({cell.id for cell in self.cells}), 11)
        self.assertEqual({cell.fault for cell in self.cells},
                         {'open-eintr', 'close-eio', 'read-eintr', 'read-eio',
                          'write-eio', 'realpath-eintr', 'stat-eintr'})
        self.assertEqual({cell.stage for cell in self.cells}, {None, 'read', 'write', 'close'})
        self.assertTrue(all(len(cell.makefile) < 256 for cell in self.cells))
        self.assertTrue(all(cell.trace for cell in self.cells))
        self.assertEqual(self.cells, faults.inventory())

    def test_faults_anchor_prefix_and_skip_suffix_after_failures(self):
        for cell in self.cells:
            with self.subTest(cell=cell.id):
                expected = faults.expected(cell, self.root)
                self.assertEqual(expected.filesystem['work/prefix.marker'], faults.regular_file(b'before\n'))
                self.assertEqual(expected.filesystem['work/effect-probe'], faults.regular_file(cell.final))
                self.assertEqual('work/suffix.marker' in expected.filesystem, cell.stage is None)
                self.assertEqual(expected.returncode, 2 if cell.stage else 0)
                self.assertEqual(expected.filesystem['work/fault.log'], faults.regular_file(cell.trace))
        partial = next(cell for cell in self.cells if cell.id == 'write-eio')
        self.assertEqual(partial.final, b'changed')
        append = next(cell for cell in self.cells if cell.id == 'close-eio-append')
        self.assertEqual(append.final, faults.INITIAL + b'changed\n')

    def test_materialize_preserves_original_inputs_and_scopes_injection(self):
        cell = self.cells[0]
        library = self.root / 'helper.so'
        case = faults.materialize(cell, self.root / 'inputs', library)
        self.assertEqual(case.environment, {'LD_PRELOAD': str(library), 'RMAKE_STDIO_FAULT': cell.fault})
        self.assertEqual((case.path / 'effect-probe').read_bytes(), faults.INITIAL)
        self.assertEqual((case.path / 'Makefile').read_bytes(), cell.makefile)
        self.assertEqual((case.path / 'protected.txt').read_bytes(), faults.PROTECTED)
        self.assertFalse((case.path / 'prefix.marker').exists())
        self.assertEqual(case.args, ('-rR', '--no-print-directory'))
        self.assertEqual(case.timeout, 5)

    def test_success_uses_same_workspace_and_retains_expectations(self):
        status, output, report, roots = self.campaign()
        self.assertEqual(status, 0)
        self.assertEqual(roots[0], roots[1])
        self.assertIn('passed=1, failed=0', output)
        self.assertEqual(report['identities'], self.identities)
        expected = report['cases'][0]['expected']
        self.assertEqual(expected['stdout_hex'], self.cells[0].stdout.hex())
        self.assertIn('work/protected.txt', expected['filesystem'])

    def test_equal_wrong_outputs_or_status_cannot_pass(self):
        for field, value in (('stdout', b'wrong'), ('stderr', b'wrong'), ('returncode', 99),
                             ('limit', 'timeout'), ('leftover_processes', True)):
            with self.subTest(field=field):
                self.artifacts = self.root / field
                status, _, report, _ = self.campaign(mutate=lambda result: setattr(result, field, value))
                self.assertEqual(status, 1)
                self.assertEqual(report['failed'], 1)

    def test_missing_injection_duplicate_close_or_changed_files_fail(self):
        cell = next(cell for cell in self.cells if cell.id == 'close-eio-write')
        mutations = [
            lambda result: result.filesystem.pop('work/fault.log'),
            lambda result: result.filesystem.update({'work/fault.log': faults.regular_file(b'closed-once\nduplicate-close\n')}),
            lambda result: result.filesystem.pop('work/prefix.marker'),
            lambda result: result.filesystem.update({'work/suffix.marker': faults.regular_file(b'after\n')}),
            lambda result: result.filesystem['work/protected.txt'].update(mode=0o600),
        ]
        for index, mutate in enumerate(mutations):
            with self.subTest(index=index):
                self.artifacts = self.root / str(index)
                status, _, _, _ = self.campaign(cell=cell, mutate=mutate)
                self.assertEqual(status, 1)

    def test_failure_saves_raw_bytes_and_helper_source_for_replay(self):
        status, _, report, _ = self.campaign(mutate=lambda result: setattr(result, 'stdout', b'bad\xff'))
        self.assertEqual(status, 1)
        record = report['cases'][0]
        artifact = Path(record['artifact'])
        self.assertEqual((artifact / 'oracle.stdout').read_bytes(), b'bad\xff')
        self.assertEqual((artifact / 'candidate.stdout').read_bytes(), b'bad\xff')
        self.assertEqual((artifact / 'input/Makefile').read_bytes(), self.cells[0].makefile)
        self.assertEqual((artifact / faults.SOURCE.name).read_bytes(), faults.SOURCE.read_bytes())
        self.assertEqual(record['replay_argv'], ['replay', '--case', self.cells[0].id])

    def test_path_expectation_uses_exact_execution_root_without_normalization(self):
        cell = next(cell for cell in self.cells if cell.id == 'realpath-eintr')
        result = faults.expected(cell, self.root)
        self.assertEqual(result.stdout, b'begin\npath=<' + bytes(self.root / 'work/effect-probe') + b'>\n')

    def test_unknown_case_stops_before_oracle_or_compilation(self):
        with mock.patch.object(faults.oracle, 'verify') as verify, \
             mock.patch.object(faults, 'compile_helper') as compile_helper, \
             contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(faults.main(['--self-test', '--case', 'does-not-exist']), 2)
        verify.assert_not_called()
        compile_helper.assert_not_called()

    def test_compile_failure_and_process_limits_fail_closed(self):
        for index, failure in enumerate(((b'', b'compile error', 1, None, False),
                                         (b'', b'', 0, 'timeout', False),
                                         (b'', b'', 0, None, True))):
            with mock.patch.object(conformance, 'capture', return_value=failure), \
                 mock.patch.object(faults.shutil, 'which', return_value='/usr/bin/cc'), \
                 self.assertRaises(conformance.HarnessError):
                faults.compile_helper(self.root / str(index))


if __name__ == '__main__':
    unittest.main()
