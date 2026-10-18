#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Original whitespace expectation, failure-anchor, and replay regressions."""
import ast
from collections import Counter
import contextlib
import dataclasses
import hashlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import conformance
import whitespace_matrix as matrix


class WhitespaceMatrixTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.artifacts = self.root / 'artifacts'
        self.cells = matrix.inventory()
        self.by_id = {cell.id: cell for cell in self.cells}
        self.identities = {'oracle': {'path': '/oracle'}, 'candidate': {'path': '/candidate'}}

    def campaign(self, cells=None, mutate=None, max_failures=20):
        cells = cells or (self.cells[0],)
        roots = []

        def execute(case, root, binary):
            roots.append(root)
            result = matrix.expected(self.by_id[case.id])
            if mutate:
                mutate(result)
            return result

        with mock.patch.object(conformance, 'execute', side_effect=execute), \
             contextlib.redirect_stdout(io.StringIO()) as output:
            status = matrix.run_campaign(cells, Path('/oracle'), Path('/candidate'),
                                         self.artifacts, self.identities, ['replay'], max_failures)
        report = json.loads(next((self.artifacts / 'campaigns').glob('*.json')).read_text())
        return status, output.getvalue(), report, roots

    def test_scripts_support_python_311(self):
        for path in (Path(matrix.__file__), Path(__file__)):
            ast.parse(path.read_text(), feature_version=(3, 11))

    def test_inventory_covers_six_c_separators_and_three_negative_controls(self):
        self.assertEqual(len(self.cells), 321)
        self.assertEqual(len(self.by_id), 321)
        original = self.cells[:297]
        self.assertEqual(Counter(cell.separator for cell in original),
                         {separator: 33 for separator in matrix.SEPARATORS})
        self.assertEqual({cell.family for cell in original}, {cell.family for cell in self.cells[:33]})
        self.assertEqual(Counter(cell.family for cell in self.cells[297:]),
                         {'span-wordlist': 10, 'span-let': 8, 'span-patsubst': 6})
        self.assertTrue(all(len(cell.makefile) < 512 for cell in original))
        self.assertTrue(all(len(cell.makefile) < 65536 for cell in self.cells))
        self.assertTrue(all(len(cell.stdout) + len(cell.stderr) < 512 for cell in self.cells))
        self.assertEqual(self.cells, matrix.inventory())
        self.assertEqual(sum(cell.error is not None for cell in self.cells), 3)

    def test_original_297_programs_remain_unchanged_in_version_two(self):
        self.assertEqual(matrix.VERSION, 2)
        self.assertEqual(hashlib.sha256(b''.join(cell.makefile for cell in self.cells[:297])).hexdigest(),
                         'f1f2983431ca1573632bb25875975f8954ebf9a53ebe9017b1e82e1906e56395')

    def test_span_wordlist_boundaries_preserve_delimiters_and_eager_tail_effects(self):
        values = {'prefix': b'first\v\vsecond', 'middle': b'second\r\nthird',
                  'tail': b'third\f fourth', 'empty': b'', 'whitespace-only': b'',
                  'large-prefix': b'w0000\r\nw0001\r\nw0002',
                  'large-middle': b'w2047\r\nw2048\r\nw2049',
                  'large-tail': b'w4093\r\nw4094\r\nw4095', 'large-outside': b''}
        for name, value in values.items():
            with self.subTest(name=name):
                self.assertEqual(self.by_id['span-wordlist-' + name].value, value)
        eager = self.by_id['span-wordlist-prefix-eager-tail']
        self.assertEqual(eager.stdout, b'tail-read\n[first]\n')
        self.assertEqual(matrix.expected(eager).filesystem['work/tail.marker'],
                         matrix.regular_file(b'tail\n'))
        self.assertIn(b'\r$(EMPTY)\n', self.by_id['span-wordlist-middle'].expression)

    def test_span_let_remainders_duplicate_bindings_restoration_and_effect_order(self):
        self.assertEqual(self.by_id['span-let-mixed-remainder'].value, b'[first][second][third\f fourth  ]')
        self.assertEqual(self.by_id['span-let-duplicate-binding'].value, b'[second\tthird  ]')
        self.assertEqual(self.by_id['span-let-restores-binding'].value, b'inner|outer|undefined')
        self.assertEqual(self.by_id['span-let-simple-remainder'].value, b'simple:first second  :first second  ')
        self.assertEqual(self.by_id['span-let-large-remainder'].value,
                         b'w0001\r\nw0002\r\nw0003|4095|w4095')
        eager = self.by_id['span-let-eager-order']
        self.assertEqual(eager.stdout, b'names\nvalues\nbody\n[[first][second]]\n')
        for name in ('names', 'values', 'body'):
            self.assertEqual(matrix.expected(eager).filesystem['work/' + name + '.marker'],
                             matrix.regular_file(name.encode() + b'\n'))
        self.assertEqual(self.by_id['span-let-zero-names-eager'].stdout, b'values\nbody\n[done]\n')

    def test_span_literal_patsubst_preserves_delimiters_and_negative_controls(self):
        self.assertEqual(self.by_id['span-patsubst-literal-delimiters'].value,
                         b' \tDOG\vDOG\r\nbob\fDOG  ')
        self.assertEqual(self.by_id['span-patsubst-literal-controls'].value,
                         b' \tcatfish\vcat\x1ccat\x85cat\xa0cat\r\n')
        self.assertEqual(self.by_id['span-patsubst-literal-delete'].value, b' \t\v\r\nbob\f  ')
        self.assertEqual(self.by_id['span-patsubst-literal-replacement-delimiters'].value,
                         b' A\tB\vA\tB ')
        self.assertEqual(self.by_id['span-patsubst-literal-percent'].value, b' R\vcat\fR ')
        self.assertEqual(self.by_id['span-patsubst-literal-large'].value, b'replacement|4096|w4095')

    def test_wordlist_and_let_retain_delimiters_without_normalizing(self):
        for separator in matrix.SEPARATORS[:6]:
            suffix = separator.hex()
            self.assertEqual(self.by_id['word-wordlist-' + suffix].value,
                             b'b.c' + separator * 2 + b'a.h')
            self.assertEqual(self.by_id['let-final-remainder-' + suffix].value,
                             b'[a][b' + separator * 2 + b'c' + separator + b']')
            self.assertEqual(self.by_id['let-one-remainder-' + suffix].value,
                             b'[a' + separator + b'b' + separator + b']')
            self.assertEqual(self.by_id['foreach-name-' + suffix].value, b'[a] [b]')

    def test_control_and_nonascii_bytes_are_data(self):
        for separator in matrix.SEPARATORS[6:]:
            suffix = separator.hex()
            self.assertEqual(self.by_id['word-words-' + suffix].value, b'1')
            self.assertEqual(self.by_id['if-raw-' + suffix].value, b'TRUE')
            self.assertEqual(self.by_id['foreach-name-' + suffix].value, b'[]')
            self.assertIn(separator, self.by_id['word-strip-' + suffix].value)

    def test_raw_and_expanded_whitespace_have_distinct_truth_and_return_values(self):
        for separator in matrix.SEPARATORS[:6]:
            suffix = separator.hex()
            self.assertEqual(self.by_id['if-raw-' + suffix].value, b'FALSE')
            self.assertEqual(self.by_id['if-expanded-' + suffix].value, separator + b'TRUE' + separator)
            self.assertEqual(self.by_id['or-raw-' + suffix].value, b'LAST')
            self.assertEqual(self.by_id['or-expanded-' + suffix].value, separator)
            self.assertEqual(self.by_id['and-raw-' + suffix].value, b'')
            self.assertEqual(self.by_id['and-expanded-' + suffix].value, separator + b'LAST' + separator)

    def test_effects_and_errors_have_exact_files_and_line_anchors(self):
        passed = matrix.expected(self.by_id['and-fatal-effects-0b'])
        failed = matrix.expected(self.by_id['and-fatal-effects-85'])
        self.assertEqual(passed.stdout, b'[]\n')
        self.assertNotIn('work/visited.txt', passed.filesystem)
        self.assertIn('work/suffix.marker', passed.filesystem)
        self.assertEqual(failed.returncode, 2)
        self.assertEqual(failed.stderr, b'Makefile:10: *** visited.  Stop.\n')
        self.assertEqual(failed.filesystem['work/visited.txt'], matrix.regular_file(b'yes\n'))
        self.assertNotIn('work/suffix.marker', failed.filesystem)
        self.assertEqual(failed.filesystem['work/prefix.marker'], matrix.regular_file(b'before\n'))
        self.assertEqual(self.by_id['if-branch-effects-09'].stdout, b'branch-false\n[FALSE]\n')
        self.assertEqual(self.by_id['if-branch-effects-a0'].stdout, b'branch-true\n[TRUE]\n')

    def test_materialize_preserves_raw_bytes_and_has_no_inherited_environment(self):
        cell = self.by_id['word-strip-a0']
        case = matrix.materialize(cell, self.root / 'inputs')
        self.assertEqual((case.path / 'Makefile').read_bytes(), cell.makefile)
        self.assertEqual((case.path / 'protected.txt').read_bytes(), matrix.PROTECTED)
        self.assertEqual(case.environment, {})
        self.assertEqual(case.args, ('-rR', '--no-print-directory'))
        self.assertEqual(case.timeout, 5)
        self.assertEqual((case.path / 'Makefile').stat().st_mode & 0o777, 0o644)

    def test_success_retains_identity_workload_and_explicit_limits(self):
        status, output, report, roots = self.campaign()
        self.assertEqual(status, 0)
        self.assertEqual(roots[0], roots[1])
        self.assertIn('passed=1, failed=0, unexecuted=0', output)
        self.assertEqual(report['identities'], self.identities)
        self.assertEqual(report['inventory_cells'], 321)
        self.assertEqual(len(report['workload_sha256']), 64)
        self.assertEqual(report['cases'][0]['expected_stdout_hex'], self.cells[0].stdout.hex())
        self.assertTrue(any('crashes' in item and 'never counted as a pass' in item
                            for item in report['limitations']))

    def test_equal_wrong_outputs_status_limits_and_signals_never_pass(self):
        for field, value in (('stdout', b'bad'), ('stderr', b'bad'), ('returncode', 99),
                             ('returncode', -11), ('limit', 'timeout'), ('leftover_processes', True)):
            with self.subTest(field=field, value=value):
                self.artifacts = self.root / (field + str(value))
                status, _, _, _ = self.campaign(mutate=lambda result: setattr(result, field, value))
                self.assertEqual(status, 1)

    def test_matching_filesystem_mutations_never_pass(self):
        mutations = [
            lambda result: result.filesystem.pop('work/prefix.marker'),
            lambda result: result.filesystem['work/protected.txt'].update(mode=0o600),
            lambda result: result.filesystem.update({'work/extra': matrix.regular_file(b'new')}),
        ]
        for index, mutate in enumerate(mutations):
            with self.subTest(index=index):
                self.artifacts = self.root / str(index)
                status, _, _, _ = self.campaign(mutate=mutate)
                self.assertEqual(status, 1)

    def test_failure_retains_exact_streams_source_and_replay(self):
        cell = self.by_id['word-strip-a0']
        status, _, report, _ = self.campaign((cell,), mutate=lambda result: setattr(result, 'stdout', b'bad\xff'))
        self.assertEqual(status, 1)
        record = report['cases'][0]
        artifact = Path(record['artifact'])
        self.assertEqual((artifact / 'oracle.stdout').read_bytes(), b'bad\xff')
        self.assertEqual((artifact / 'candidate.stdout').read_bytes(), b'bad\xff')
        self.assertEqual((artifact / 'input/Makefile').read_bytes(), cell.makefile)
        self.assertEqual((artifact / 'whitespace_matrix.py').read_bytes(), Path(matrix.__file__).read_bytes())
        self.assertEqual(record['replay_argv'], ['replay', '--case', cell.id])

    def test_failure_cap_reports_unexecuted_cells(self):
        status, _, report, roots = self.campaign(self.cells[:3],
                                                mutate=lambda result: setattr(result, 'stdout', b'bad'),
                                                max_failures=1)
        self.assertEqual(status, 1)
        self.assertEqual(report['executed'], 1)
        self.assertEqual(report['unexecuted'], 2)
        self.assertEqual(len(roots), 2)

    def test_bad_selection_or_limits_stop_before_execution(self):
        bad_cells = ((), (self.cells[0], self.cells[0]),
                     (dataclasses.replace(self.cells[0], value=b'wrong'),))
        for cells in bad_cells:
            with self.subTest(cells=cells), mock.patch.object(conformance, 'execute') as execute, \
                 self.assertRaises(conformance.HarnessError):
                matrix.run_campaign(cells, Path('/oracle'), Path('/candidate'), self.artifacts, {}, [])
            execute.assert_not_called()
        for arguments in (['--case', 'missing'], ['--max-failures', '0'], ['--max-failures', '322']):
            with mock.patch.object(matrix.oracle, 'verify') as verify, \
                 contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(matrix.main(['--self-test', *arguments]), 2)
            verify.assert_not_called()


if __name__ == '__main__':
    unittest.main()
