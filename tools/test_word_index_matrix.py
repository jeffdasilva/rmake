#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Original numeric-index anchors, side effects, and replay regressions."""
import ast
from collections import Counter
import contextlib
import dataclasses
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import conformance
import word_index_matrix as matrix


class WordIndexMatrixTests(unittest.TestCase):
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

    def test_inventory_is_bounded_distinct_and_covers_all_three_index_roles(self):
        self.assertEqual(len(self.cells), 220)
        self.assertEqual(len(self.by_id), 220)
        self.assertEqual(len(matrix.indices()), 58)
        self.assertEqual(Counter(cell.family for cell in self.cells),
                         {'index-word': 58, 'index-start': 58, 'index-stop': 58,
                          'validation-order': 14, 'decimal-grammar': 2,
                          'eager-effects': 21, 'source-context': 9})
        self.assertTrue(all(len(cell.makefile) < 1024 for cell in self.cells))
        self.assertTrue(all(len(cell.stdout) + len(cell.stderr) < 512 for cell in self.cells))
        self.assertEqual(self.cells, matrix.inventory())

    def test_signed64_boundaries_and_canonical_domain_errors_are_distinct(self):
        self.assertEqual(self.by_id['word-i64-max'].stdout, b'[]\n')
        self.assertEqual(self.by_id['stop-i64-max'].stdout, b'[alpha  beta\tgamma]\n')
        self.assertIn(b"first argument to 'word' function must be greater than 0",
                      self.by_id['word-i64-min'].stderr)
        self.assertIn(b"'-9223372036854775808'", self.by_id['start-i64-min'].stderr)
        self.assertNotIn(b'out of range', self.by_id['start-i64-min'].stderr)
        self.assertIn(b"'9223372036854775808' out of range", self.by_id['word-i64-over'].stderr)
        self.assertIn(b"'-9223372036854775809' out of range", self.by_id['word-i64-under'].stderr)
        self.assertIn(b"'0'", self.by_id['start-padded-minus-zero'].stderr)
        self.assertEqual(self.by_id['stop-minus-zero'].stdout, b'[]\n')

    def test_overflow_prefix_wins_before_bad_suffix_and_raw_bytes_are_preserved(self):
        error = self.by_id['word-overflow-before-suffix'].stderr
        self.assertIn(b"' \t9223372036854775808tail\v ' out of range", error)
        self.assertNotIn(b'out of range', self.by_id['word-valid-prefix-invalid-suffix'].stderr)
        self.assertNotIn(b'out of range', self.by_id['word-invalid-before-overflow'].stderr)
        self.assertIn(b"'\xff2\xfe'", self.by_id['word-raw-byte'].stderr)
        self.assertIn(b"'2'3'", self.by_id['stop-quote'].stderr)
        self.assertIn(b"'-1'", self.by_id['start-padded-negative'].stderr)
        self.assertNotIn(b'-0001', self.by_id['start-padded-negative'].stderr)

    def test_six_c_separators_accept_padding_but_nonascii_controls_do_not(self):
        for separator in (b' ', b'\t', b'\n', b'\r', b'\v', b'\f'):
            suffix = separator.hex()
            self.assertEqual(self.by_id['word-surrounded-' + suffix].stdout, b'[beta]\n')
            self.assertIn(b'empty value', self.by_id['word-empty-' + suffix].stderr)
        for suffix in ('control-1c', 'nonascii-85', 'nonascii-a0'):
            self.assertEqual(self.by_id['word-' + suffix].status, 2)
            self.assertIn(b'invalid first argument', self.by_id['word-' + suffix].stderr)

    def test_leading_zeros_are_decimal_and_do_not_overflow_by_length(self):
        self.assertEqual(self.by_id['word-long-zero-prefix'].stdout, b'[beta]\n')
        self.assertEqual(self.by_id['decimal-word-leading-zero'].stdout, b'[w10]\n')
        self.assertEqual(self.by_id['decimal-wordlist-leading-zero'].stdout, b'[w8 w9 w10]\n')

    def test_error_order_and_eager_effects_have_independent_file_anchors(self):
        case = self.by_id['effect-wordlist-all-before-validation']
        self.assertEqual(case.stdout, b'first\nsecond\nlist\n')
        self.assertIn(b"invalid first argument", case.stderr)
        files = matrix.expected(case).filesystem
        for name in ('first', 'second', 'list'):
            self.assertEqual(files['work/' + name + '.marker'], matrix.regular_file(name.encode() + b'\n'))
        self.assertNotIn('work/suffix.marker', files)
        nested = self.by_id['effect-wordlist-second-nested']
        self.assertEqual(nested.stdout, b'first\nsecond\n')
        self.assertIn(b'*** nested.', nested.stderr)
        self.assertNotIn('work/list.marker', matrix.expected(nested).filesystem)
        self.assertIn(b"'0'", self.by_id['pair-zero-before-invalid'].stderr)
        self.assertNotIn(b"'bad'", self.by_id['pair-zero-before-invalid'].stderr)

    def test_provided_arguments_expand_before_missing_arity_and_surplus_is_eager(self):
        for function in ('word', 'wordlist'):
            case = self.by_id['effect-' + function + '-provided-before-arity']
            self.assertEqual(case.stdout, b'first\n')
            self.assertIn(b'insufficient number of arguments (1)', case.stderr)
            self.assertIn('work/first.marker', matrix.expected(case).filesystem)
            self.assertIn(b'*** nested.', self.by_id['effect-' + function + '-nested-before-arity'].stderr)
        self.assertEqual(self.by_id['effect-call-word-surplus'].stdout, b'extra\n[beta]\n')
        self.assertIn(b'*** extra.', self.by_id['effect-call-word-surplus-error'].stderr)

    def test_source_locations_and_recipe_expansion_preserve_effects(self):
        for name in ('recursive-definition', 'nested-definition', 'recipe-definition',
                     'command-line-recursive', 'environment-recursive', 'target-definition', 'call-body'):
            self.assertTrue(self.by_id['context-' + name].stderr.startswith(b'Makefile:2: ***'))
        direct = self.by_id['context-recipe-direct']
        self.assertTrue(direct.stderr.startswith(b'Makefile:4: ***'))
        self.assertEqual(direct.stdout, b'expanded\n')
        self.assertIn('work/expanded.marker', matrix.expected(direct).filesystem)
        immediate = self.by_id['context-command-line-immediate']
        self.assertTrue(immediate.stderr.startswith(b'make: ***'))
        self.assertNotIn('work/prefix.marker', matrix.expected(immediate).filesystem)

    def test_materialize_retains_raw_inputs_and_explicit_environment_only(self):
        for cell in (self.by_id['word-raw-byte'], self.by_id['context-environment-recursive']):
            case = matrix.materialize(cell, self.root / 'inputs')
            self.assertEqual((case.path / 'Makefile').read_bytes(), cell.makefile)
            self.assertEqual(case.environment, dict(cell.environment))
            self.assertEqual(case.args, cell.args)
            self.assertEqual(case.timeout, 5)
            self.assertEqual((case.path / 'Makefile').stat().st_mode & 0o777, 0o644)

    def test_success_retains_identity_source_workload_and_same_alias_root(self):
        status, output, report, roots = self.campaign()
        self.assertEqual(status, 0)
        self.assertEqual(roots[0], roots[1])
        self.assertIn('passed=1, failed=0, unexecuted=0', output)
        self.assertEqual(report['identities'], self.identities)
        self.assertEqual(report['inventory_cells'], 220)
        self.assertEqual(len(report['workload_sha256']), 64)
        self.assertEqual(report['cases'][0]['expected_stderr_hex'], self.cells[0].stderr.hex())

    def test_equal_wrong_output_status_limits_and_crashes_cannot_pass(self):
        for field, value in (('stdout', b'bad'), ('stderr', b'bad'), ('returncode', 99),
                             ('returncode', -11), ('limit', 'timeout'), ('leftover_processes', True)):
            with self.subTest(field=field, value=value):
                self.artifacts = self.root / (field + str(value))
                status, _, _, _ = self.campaign(mutate=lambda result: setattr(result, field, value))
                self.assertEqual(status, 1)

    def test_equal_filesystem_mutations_cannot_pass(self):
        mutations = [lambda result: result.filesystem.pop('work/prefix.marker'),
                     lambda result: result.filesystem['work/protected.txt'].update(mode=0o600),
                     lambda result: result.filesystem.update({'work/extra': matrix.regular_file(b'new')})]
        for index, mutate in enumerate(mutations):
            with self.subTest(index=index):
                self.artifacts = self.root / str(index)
                status, _, _, _ = self.campaign(mutate=mutate)
                self.assertEqual(status, 1)

    def test_failure_retains_raw_streams_input_and_replay(self):
        cell = self.by_id['word-raw-byte']
        status, _, report, _ = self.campaign((cell,), mutate=lambda result: setattr(result, 'stderr', b'bad\xff'))
        self.assertEqual(status, 1)
        record = report['cases'][0]
        artifact = Path(record['artifact'])
        self.assertEqual((artifact / 'oracle.stderr').read_bytes(), b'bad\xff')
        self.assertEqual((artifact / 'candidate.stderr').read_bytes(), b'bad\xff')
        self.assertEqual((artifact / 'input/Makefile').read_bytes(), cell.makefile)
        self.assertEqual(record['replay_argv'], ['replay', '--case', cell.id])
        self.assertEqual((artifact / 'word_index_matrix.py').read_bytes(), Path(matrix.__file__).read_bytes())

    def test_failure_cap_reports_unexecuted_cases(self):
        status, _, report, roots = self.campaign(self.cells[:3],
                                                mutate=lambda result: setattr(result, 'stdout', b'bad'),
                                                max_failures=1)
        self.assertEqual(status, 1)
        self.assertEqual(report['executed'], 1)
        self.assertEqual(report['unexecuted'], 2)
        self.assertEqual(len(roots), 2)

    def test_invalid_selection_or_limits_do_not_execute(self):
        for cells in ((), (self.cells[0], self.cells[0]),
                      (dataclasses.replace(self.cells[0], stdout=b'wrong'),)):
            with self.subTest(cells=cells), mock.patch.object(conformance, 'execute') as execute, \
                 self.assertRaises(conformance.HarnessError):
                matrix.run_campaign(cells, Path('/oracle'), Path('/candidate'), self.artifacts, {}, [])
            execute.assert_not_called()
        for arguments in (['--case', 'missing'], ['--max-failures', '0'], ['--max-failures', '221']):
            with mock.patch.object(matrix.oracle, 'verify') as verify, contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(matrix.main(['--self-test', *arguments]), 2)
            verify.assert_not_called()


if __name__ == '__main__':
    unittest.main()
