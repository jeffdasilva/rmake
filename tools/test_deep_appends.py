# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect original deep inherited-append models and frozen observations."""
import base64
import dataclasses
import json
from pathlib import Path
import tomllib
import unittest
import benchmark
import conformance as h
ROOT = Path(__file__).resolve().parents[1]

class DeepAppendTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.evidence = json.loads((ROOT / 'tests/research/deep-appends.json').read_text())
        cls.records = {r['id']: r for r in cls.evidence['records']}

    def case(self, name):
        return h.load_case(ROOT / 'tests/cases' / name)

    def observations(self, record):
        for build in ('baseline', 'debug', 'release'):
            yield from record[build]['observations'][:2 if build == 'baseline' else 3]

    def test_inventory_preserves_models_inputs_and_frozen_qualification(self):
        ledger = tomllib.loads((ROOT / 'tests/requirements.toml').read_text())
        family = next(r for r in ledger['requirements'] if r['id'] == 'variables.deep-appends')
        self.assertEqual((family['fixture_count'], family['candidate_required']), (495, 495))
        self.assertEqual(sorted(family['cases']), sorted(self.records))
        self.assertEqual(sum(bool(r['baseline']['candidate_differences']) for r in self.records.values()), 144)
        for record in self.records.values():
            case = self.case(record['id'])
            self.assertEqual((case.args, case.environment, case.expected), (tuple(record['args']), record['environment'], record['expected']))
            for name, contents in record['inputs'].items():
                path = case.path / name
                self.assertEqual((path.stat().st_mode & 0o777, path.read_bytes()), (contents['mode'], bytes.fromhex(contents['hex'])))
            for build in ('baseline', 'debug', 'release'):
                check = record[build]
                self.assertEqual(check['model_errors'][:2], [[], []])
                self.assertEqual(check['oracle_repeat_differences'], [])
                self.assertEqual(check['bounds'], [False, False, False])
                if build != 'baseline':
                    self.assertEqual((check['candidate_differences'], check['model_errors']), ([], [[], [], []]))

    def test_each_native_model_has_an_identical_heap_entry_control(self):
        native = [r for r in self.records.values() if r['id'].startswith('append-depth-') and r['id'].endswith('-h0')]
        self.assertEqual(len(native), 246)
        for record in native:
            other = self.records[record['id'][:-1] + '1']
            output = bytes.fromhex(record['expected']['stdout_hex'])
            first, rest = output.split(b'\n', 1)
            value = first[len(b'VALUE=['):-1]
            self.assertEqual(bytes.fromhex(other['expected']['stdout_hex']), b'VALUE=[' + b'[' * 192 + value + b']' * 192 + b']\n' + rest)
            for entry in (record, other):
                case = self.case(entry['id'])
                self.assertTrue(h.expectation_errors(case, h.Observation(b'', b'', 2, None, {})))

    def test_append_file_effects_run_outer_first_once_without_private_leaks(self):
        for record in self.records.values():
            if not record['id'].startswith('append-depth-'):
                continue
            parts = record['id'].split('-')
            depth, kind, private = int(parts[2]), parts[3], parts[-2] == 'p1'
            expected = b''.join(f'e{i}\n'.encode() for i in range(depth) if not (private and i == depth // 2))
            for observation in self.observations(record):
                if kind == 'effect':
                    self.assertEqual(base64.b64decode(observation['filesystem']['work/seen']['content']), expected)
                else:
                    self.assertNotIn('work/seen', observation['filesystem'])

    def test_empty_suffixes_keep_every_append_separator(self):
        for record in self.records.values():
            parts = record['id'].split('-')
            if not record['id'].startswith('append-depth-') or parts[3] != 'empty':
                continue
            depth, private, heap = int(parts[2]), parts[-2] == 'p1', parts[-1] == 'h1'
            value = b'base' + b' ' * (depth - int(private))
            if heap:
                value = b'[' * 192 + value + b']' * 192
            self.assertEqual(bytes.fromhex(record['expected']['stdout_hex']), b'VALUE=[' + value + b']\n')

    def test_all_automatic_suffixes_use_the_leaf_file_context(self):
        for record in self.records.values():
            parts = record['id'].split('-')
            if not record['id'].startswith('append-depth-') or parts[3] != 'automatic':
                continue
            depth, private, heap = int(parts[2]), parts[-2] == 'p1', parts[-1] == 'h1'
            value = b'base' + b''.join(f' e{i}-n{depth}'.encode() for i in range(depth) if not (private and i == depth // 2))
            if heap:
                value = b'[' * 192 + value + b']' * 192
            self.assertEqual(bytes.fromhex(record['expected']['stdout_hex']), b'VALUE=[' + value + b']\n')

    def test_all_three_evaluation_sites_and_both_output_modes_are_required(self):
        names = [n for n in self.records if n.startswith('append-depth-')]
        for phase in ('recipe', 'secondary', 'export'):
            for mode in ('none', 'target'):
                selected = [n for n in names if f'-{phase}-{mode}-' in n]
                self.assertEqual(len(selected), 82)
                self.assertTrue(all(self.case(n).candidate == 'required' for n in selected))

    def test_scaling_observes_every_valid_preceding_append_read(self):
        self.assertEqual(list(benchmark.deep_append_workloads()), list(benchmark.selected_workloads('deep-appends')))
        for (name, source), depth in zip(benchmark.deep_append_workloads(), (0, 32, 96)):
            case = self.case(name)
            self.assertEqual((case.path / 'Makefile').read_bytes(), source)
            self.assertEqual(case.args, tuple(benchmark.workload_arguments(name)))
            self.assertEqual(source.count(b'$(V)'), 200)
            value = b'base' + b''.join(f' e{i}'.encode() for i in range(depth))
            output = b'READS=[' + b'|'.join([value] * 200) + b']\n'
            good = h.Observation(output, b'', 0, None, {})
            self.assertEqual(h.expectation_errors(case, good), [])
            self.assertTrue(h.expectation_errors(case, dataclasses.replace(good, stdout=output.replace(b'base', b'WRONG', 1))))

if __name__ == '__main__':
    unittest.main()
