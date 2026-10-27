#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Original diagnostic matrix expectation, mutation, and replay regressions."""
import ast
import contextlib
import copy
import dataclasses
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import conformance
import diagnostic_matrix as matrix
import oracle


def observation(cell):
    return conformance.Observation(cell.stdout, cell.stderr, cell.status, None,
                                   matrix.expected_files(cell))


class DiagnosticMatrixTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.artifacts = self.root / 'artifacts'
        self.cell = matrix.inventory()[0]
        self.reference = observation(self.cell)
        self.identities = {'oracle': {'path': '/oracle', 'sha256': 'oracle-digest',
                                      'profile': {'verified': True}},
                           'candidate': {'path': '/candidate', 'sha256': 'candidate-digest'}}

    def run_campaign(self, effects, **options):
        arguments = {'cells': (self.cell,), 'reference': Path('/oracle'),
                     'candidate': Path('/candidate'), 'identities': self.identities,
                     'artifacts': self.artifacts, 'profile': Path('/profile.json'),
                     'invocation': ['python3', 'tools/diagnostic_matrix.py', '--candidate', '/candidate']}
        arguments.update(options)
        with mock.patch.object(conformance, 'execute', side_effect=effects) as execute, \
             contextlib.redirect_stdout(io.StringIO()) as output:
            status = matrix.run_campaign(**arguments)
        return status, output.getvalue(), execute

    def report(self, artifacts=None):
        return json.loads(next(((artifacts or self.artifacts) / 'campaigns').glob('*.json')).read_text())

    def test_matrix_and_tests_parse_with_python_311_grammar(self):
        for source in (Path(matrix.__file__), Path(__file__)):
            ast.parse(source.read_text(), feature_version=(3, 11))

    def test_inventory_is_bounded_distinct_and_repeatable(self):
        cells = matrix.inventory()
        self.assertEqual(len(cells), 218)
        self.assertEqual(len({cell.case_id for cell in cells}), 218)
        self.assertEqual(len({(cell.makefile, cell.args) for cell in cells}), 218)
        self.assertEqual(cells, matrix.inventory())
        self.assertTrue(all(len(cell.makefile) < 8192 for cell in cells))
        self.assertTrue(all(len(cell.stdout) + len(cell.stderr) < 8192 for cell in cells))
        self.assertTrue(all('boundary' not in cell.family for cell in cells))
        self.assertEqual(len({cell.family for cell in cells}), 15)

    def test_every_cell_has_independent_exact_expectations(self):
        for cell in matrix.inventory():
            with self.subTest(case=cell.case_id):
                self.assertEqual(matrix.anchor_errors(cell, observation(cell)), [])
                self.assertIs(type(cell.status), int)
                self.assertIs(type(cell.stdout), bytes)
                self.assertIs(type(cell.stderr), bytes)
                for field, bad in [('stdout', cell.stdout + b'bad'),
                                   ('stderr', cell.stderr + b'bad'),
                                   ('returncode', 99)]:
                    wrong = observation(cell)
                    setattr(wrong, field, bad)
                    self.assertTrue(matrix.anchor_errors(cell, wrong))

    def test_raw_payloads_and_warning_locations_have_explicit_anchors(self):
        cells = matrix.inventory()
        raw = next(cell for cell in cells if cell.stdout == b'\xff\xfe\n')
        self.assertIn(b'X\xff\xfeX', raw.makefile)
        self.assertEqual(raw.args, ('-rR', '--no-print-directory'))
        multiline = next(cell for cell in cells if cell.stderr == b'Makefile:5: line1\nline2\n')
        self.assertIn(b'Xline1\nline2X', multiline.makefile)
        self.assertTrue(any(cell.stderr == b'make: context\n' for cell in cells))
        self.assertTrue(any(cell.status == 2 and b'*** stopped.' in cell.stderr for cell in cells))

    def test_export_traversal_has_independent_global_target_and_pattern_anchors(self):
        cells=[cell for cell in matrix.inventory() if cell.family=='export-traversal']
        self.assertEqual(len(cells),12)
        self.assertTrue(any(cell.stdout==b'B\nA\nC\n' for cell in cells))
        self.assertTrue(any(cell.stderr==b'Makefile:1: Z\nMakefile:3: Y\nMakefile:2: X\n' for cell in cells))
        for cell in cells:
            good=observation(cell)
            self.assertEqual(matrix.anchor_errors(cell,good),[])
            changed=dataclasses.replace(good, stdout=good.stdout[::-1]) if cell.stdout else dataclasses.replace(good, stderr=good.stderr[::-1])
            self.assertTrue(matrix.anchor_errors(cell,changed))

    def test_materialized_manifest_replays_raw_bytes_and_expected_outputs(self):
        for cell in (self.cell, next(c for c in matrix.inventory() if c.stdout == b'\xff\xfe\n')):
            case = matrix.materialize(cell, self.root)
            self.assertEqual((case.path / 'Makefile').read_bytes(), cell.makefile)
            self.assertEqual(case.args, cell.args)
            self.assertEqual(case.candidate, 'required')
            self.assertEqual(case.output, cell.output)
            self.assertEqual(case.expected['stdout_hex'], cell.stdout.hex())
            self.assertEqual(case.expected['stderr_hex'], cell.stderr.hex())
            self.assertEqual(case.expected['exit_code'], cell.status)
            self.assertEqual((case.path / 'protected.txt').stat().st_mode & 0o777, 0o644)
            self.assertEqual(json.loads((case.path / 'matrix.json').read_text()), cell.metadata())

    def test_filesystem_anchors_detect_mutation_even_if_both_engines_agree(self):
        mutations = [lambda files: files.pop('work/protected.txt'),
                     lambda files: files['work/protected.txt'].update(mode=0o600),
                     lambda files: files['work/protected.txt'].update(type='symlink'),
                     lambda files: files['work/protected.txt'].update(hardlinks=['work/alias']),
                     lambda files: files.update({'work/extra': matrix.regular_file(b'new')}),
                     lambda files: files['work/Makefile'].update(content='changed')]
        for index, mutate in enumerate(mutations):
            with self.subTest(mutation=index):
                bad = observation(self.cell)
                mutate(bad.filesystem)
                status, _, _ = self.run_campaign([bad, copy.deepcopy(bad)], artifacts=self.artifacts / str(index))
                self.assertEqual(status, 1)

    def test_authored_expected_files_distinguish_touch_from_skipped_execution(self):
        touched = [c for c in matrix.inventory() if c.created]
        self.assertTrue(touched)
        for cell in touched:
            good = observation(cell)
            self.assertEqual(good.filesystem['work/all'], matrix.regular_file(b''))
            del good.filesystem['work/all']
            self.assertIn('authored filesystem expectation failed', matrix.anchor_errors(cell, good))

    def test_equal_wrong_streams_and_status_are_not_self_test_passes(self):
        for field, value in [('stdout', b'wrong\xff'), ('stderr', b'wrong\xfe'), ('returncode', 77)]:
            with self.subTest(field=field):
                bad = observation(self.cell)
                setattr(bad, field, value)
                status, _, _ = self.run_campaign([bad, copy.deepcopy(bad)], artifacts=self.artifacts / field)
                self.assertEqual(status, 1)

    def test_merged_order_mutation_is_detected(self):
        cell = next(c for c in matrix.inventory() if c.family == 'child-output-order')
        good = observation(cell)
        wrong = copy.deepcopy(good)
        wrong.stdout = b'child1\n' + wrong.stdout.replace(b'child1\n', b'')
        status, _, _ = self.run_campaign([good, wrong], cells=(cell,))
        self.assertEqual(status, 1)

    def test_limits_and_leftover_children_never_pass_even_when_equal(self):
        for field, value in [('limit', 'timeout'), ('leftover_processes', True)]:
            bad = observation(self.cell)
            setattr(bad, field, value)
            status, _, _ = self.run_campaign([bad, copy.deepcopy(bad)], artifacts=self.artifacts / field)
            self.assertEqual(status, 1)

    def test_success_report_retains_workload_identities_and_exact_expectations(self):
        status, output, execute = self.run_campaign([self.reference, copy.deepcopy(self.reference)])
        self.assertEqual(status, 0)
        self.assertIn('passed=1, failed=0, unexecuted=0', output)
        self.assertEqual(execute.call_args_list[0].args[1], execute.call_args_list[1].args[1])
        report = self.report()
        self.assertEqual(report['identities'], self.identities)
        self.assertEqual(report['inventory_cells'], 218)
        self.assertEqual(report['matrix_source_sha256'], conformance.sha256_file(Path(matrix.__file__)))
        self.assertEqual(report['cells'][0]['expected_stdout_hex'], self.cell.stdout.hex())
        self.assertEqual(report['selected_cases'], [self.cell.case_id])
        self.assertEqual(report['timeout_seconds'], 5)
        self.assertEqual(report['environment']['LC_ALL'], 'C')
        self.assertEqual(len(report['workload_sha256']), 64)
        explicit = self.root / 'explicit.json'
        self.run_campaign([self.reference, self.reference], report_path=explicit)
        self.assertEqual(json.loads(explicit.read_text())['workload_sha256'], report['workload_sha256'])

    def test_failure_keeps_raw_streams_input_and_standalone_replay_metadata(self):
        wrong = observation(self.cell)
        wrong.stdout = b'wrong\xff\n'
        status, _, _ = self.run_campaign([self.reference, wrong])
        self.assertEqual(status, 1)
        artifact = Path(self.report()['cells'][0]['artifact'])
        self.assertEqual((artifact / 'input/Makefile').read_bytes(), self.cell.makefile)
        self.assertEqual((artifact / 'candidate.stdout').read_bytes(), wrong.stdout)
        self.assertEqual((artifact / 'oracle.stdout').read_bytes(), self.cell.stdout)
        self.assertEqual((artifact / 'diagnostic_matrix.py').read_bytes(), Path(matrix.__file__).read_bytes())
        metadata = json.loads((artifact / 'matrix.json').read_text())
        self.assertEqual(metadata['replay_argv'][-2:], ['--case', self.cell.case_id])
        self.assertIn('/profile.json', metadata['replay_argv'])
        self.assertEqual(json.loads((artifact / 'report.json').read_text())['identities'], self.identities)
        replay_parent = self.root / 'replay'
        replay_parent.mkdir()
        import shutil
        shutil.copytree(artifact / 'input', replay_parent / self.cell.case_id)
        self.assertEqual(conformance.load_case(replay_parent / self.cell.case_id).id, self.cell.case_id)

    def test_candidate_capture_exception_keeps_completed_oracle_output(self):
        status, _, _ = self.run_campaign([self.reference, conformance.HarnessError('snapshot limit')])
        self.assertEqual(status, 1)
        artifact = Path(self.report()['cells'][0]['artifact'])
        report = json.loads((artifact / 'report.json').read_text())
        self.assertEqual(report['errors'], ['snapshot limit'])
        self.assertEqual(report['observations']['oracle']['returncode'], 0)
        self.assertEqual((artifact / 'oracle.stdout').read_bytes(), self.cell.stdout)
        self.assertFalse((artifact / 'candidate.stdout').exists())

    def test_oracle_capture_exception_still_has_input_and_report(self):
        status, _, execute = self.run_campaign([OSError('failed capture')])
        self.assertEqual(status, 1)
        self.assertEqual(execute.call_count, 1)
        artifact = Path(self.report()['cells'][0]['artifact'])
        report = json.loads((artifact / 'report.json').read_text())
        self.assertEqual(report['observations'], {})
        self.assertEqual((artifact / 'input/Makefile').read_bytes(), self.cell.makefile)

    def test_failure_budget_reports_unexecuted_and_full_workload(self):
        wrong = observation(self.cell)
        wrong.stderr = b'bad'
        status, output, execute = self.run_campaign([self.reference, wrong], cells=matrix.inventory()[:5], max_failures=1)
        self.assertEqual(status, 1)
        self.assertEqual(execute.call_count, 2)
        self.assertIn('4 cells unexecuted', output)
        report = self.report()
        self.assertEqual((report['executed'], report['passed'], report['failed'], report['unexecuted']), (1, 0, 1, 4))
        self.assertEqual(len(report['selected_cases']), 5)
        self.assertEqual(len(report['workload_sha256']), 64)

    def test_invalid_budgets_and_tampered_or_duplicate_cells_are_rejected(self):
        for value in (0, -1, 219, True, 1.5):
            with self.assertRaises(conformance.HarnessError):
                matrix.validate_limit(value)
        for cells in ((), (self.cell, self.cell), (dataclasses.replace(self.cell, makefile=b'changed'),)):
            with self.assertRaises(conformance.HarnessError):
                self.run_campaign([], cells=cells)

    def test_replay_selection_is_exact_and_stable(self):
        last = matrix.inventory()[-1]
        self.assertEqual(matrix.select_cells([last.case_id, self.cell.case_id, self.cell.case_id]), (self.cell, last))
        with self.assertRaisesRegex(conformance.HarnessError, 'unknown case'):
            matrix.select_cells(['message-missing'])

    def test_invalid_selection_and_oracle_identity_failure_prevent_execution(self):
        for arguments in (['--max-failures', '0'], ['--case', 'missing']):
            with mock.patch.object(oracle, 'verify') as verify, contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(matrix.main(['--self-test', *arguments]), 2)
            verify.assert_not_called()
        with mock.patch.object(oracle, 'verify', side_effect=oracle.OracleError('digest mismatch')), \
             mock.patch.object(matrix, 'run_campaign') as run, contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(matrix.main(['--self-test']), 2)
        run.assert_not_called()

    def test_self_test_and_candidate_cli_preserve_verified_profile(self):
        for self_test in (True, False):
            mode = ['--self-test'] if self_test else ['--candidate', '/candidate']
            identity = {'path': '/verified/make', 'sha256': 'verified'}
            with mock.patch.object(oracle, 'verify', return_value={'verified': True}) as verify, \
                 mock.patch.object(conformance, 'executable_identity', return_value=identity), \
                 mock.patch.object(matrix, 'run_campaign', return_value=0) as run:
                self.assertEqual(matrix.main([*mode, '--case', self.cell.case_id]), 0)
            verify.assert_called_once()
            call = run.call_args.kwargs
            self.assertEqual(call['self_test'], self_test)
            self.assertEqual(call['identities']['oracle']['profile'], {'verified': True})
            self.assertEqual(call['cells'], (self.cell,))
            replay = matrix.replay_arguments(self.cell, Path('/make'), Path('/candidate'), Path('/profile'), self_test)
            self.assertIn('--self-test' if self_test else '--candidate', replay)


if __name__ == '__main__':
    unittest.main()
