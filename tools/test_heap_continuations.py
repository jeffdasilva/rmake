# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect original heap-evaluator models and their independently checked evidence."""
import base64
import dataclasses
import json
from pathlib import Path
import tomllib
import unittest
import benchmark
import conformance as h

ROOT = Path(__file__).resolve().parents[1]

class HeapContinuationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.evidence = json.loads((ROOT / 'tests/research/heap-continuations.json').read_text())
        cls.records = {r['id']: r for r in cls.evidence['records']}

    def case(self, name):
        return h.load_case(ROOT / 'tests/cases' / name)

    def output(self, name):
        return bytes.fromhex(self.records[name]['expected']['stdout_hex'])

    def observations(self, record):
        for build in ('baseline', 'debug', 'release'):
            yield from record[build]['observations'][:2 if build == 'baseline' else 3]

    def test_inventory_inputs_models_and_both_frozen_builds_agree(self):
        ledger = tomllib.loads((ROOT / 'tests/requirements.toml').read_text())
        family = next(r for r in ledger['requirements'] if r['id'] == 'functions.heap-continuations')
        self.assertEqual((family['fixture_count'], family['candidate_required']), (457, 457))
        self.assertEqual(sorted(family['cases']), sorted(self.records))
        self.assertEqual(sum(bool(r['baseline']['candidate_differences']) for r in self.records.values()), 296)
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

    def test_all_prior_function_body_research_models_are_closed_without_refinement(self):
        prior = [r for r in self.records.values() if r['id'].startswith('function-body-')]
        self.assertEqual(len(prior), 144)
        self.assertEqual(sum(bool(r['baseline']['candidate_differences']) for r in prior), 80)
        for depth in (96, 192, 512):
            for kind in ('or', 'and', 'if-false', 'intcmp-less', 'intcmp-equal', 'intcmp-greater', 'let', 'foreach', 'computed', 'substitution'):
                case = self.case(f'heap-{kind}-{depth}-reader-none')
                expected = b'VALUE=[' + b'[' * depth + b'end' + b']' * depth + b']\n'
                self.assertEqual(bytes.fromhex(case.expected['stdout_hex']), expected)
                self.assertTrue(h.expectation_errors(case, h.Observation(b'', b'', 2, None, {})))

    def test_file_effects_surround_children_in_order_without_replay(self):
        for depth in (3, 96, 192, 512):
            lines = [f'b{i}\n'.encode() for i in range(depth)] + [f'a{i}\n'.encode() for i in reversed(range(depth))]
            for phase in ('reader', 'recipe'):
                for mode in ('none', 'target'):
                    record = self.records[f'heap-file-order-{depth}-{phase}-{mode}']
                    for observation in self.observations(record):
                        self.assertEqual(base64.b64decode(observation['filesystem']['work/seen']['content']), b''.join(lines))

    def test_recursive_call_masks_lexical_scopes_and_rebindings_remain_live(self):
        for phase in ('reader', 'recipe'):
            for mode in ('none', 'target'):
                for depth in (3, 64, 192, 512):
                    self.assertEqual(self.output(f'heap-recursive-call-{depth}-{phase}-{mode}'), b'VALUE=[' + b'x' * depth + b']\nAFTER=[global]\n')
                for depth in (3, 192, 512):
                    wrap = lambda x: b'[' * depth + x + b']' * depth
                    self.assertEqual(self.output(f'heap-numeric-mask-{depth}-{phase}-{mode}'), b'MASK=[' + wrap(b'') + b']\nLEX=[' + wrap(b'shadow') + b']\nAFTER=[global]\n')
                for depth in (3, 96, 192, 512):
                    value = b'end'
                    for _ in range(depth):
                        value = b'[' + value + f']R{depth}'.encode()
                    self.assertEqual(self.output(f'heap-fresh-lookup-{depth}-{phase}-{mode}'), b'VALUE=[' + value + f']\nPICK=[R{depth}]\n'.encode())

    def test_warning_order_caller_locations_and_fatal_cycle_are_exact(self):
        for depth in (3, 192, 512):
            for phase, line in (('reader', depth + 2), ('recipe', depth + 3)):
                for mode in ('none', 'target'):
                    before = ''.join(f'Makefile:{line}: w{i}\n' for i in range(depth)).encode()
                    after = ''.join(f'Makefile:{line}: a{i}\n' for i in reversed(range(depth))).encode()
                    for cyclic in (0, 1):
                        case = self.case(f'heap-warning-{cyclic}-{depth}-{phase}-{mode}')
                        expected = before + (b"Makefile:1: *** Recursive variable 'R0' references itself (eventually).  Stop.\n" if cyclic else after)
                        self.assertEqual(bytes.fromhex(case.expected['stderr_hex']), expected)
                        self.assertEqual(case.expected['exit_code'], 2 if cyclic else 0)

    def test_raw_nul_return_does_not_skip_a_later_file_effect(self):
        for record in self.records.values():
            if record['id'].startswith('heap-nul-'):
                self.assertEqual(bytes.fromhex(record['inputs']['payload']['hex']), b'raw\0ignored\n')
                self.assertNotIn(b'ignored', bytes.fromhex(record['expected']['stdout_hex']))
                for observation in self.observations(record):
                    self.assertEqual(base64.b64decode(observation['filesystem']['work/seen']['content']), b'E\n')

    def test_scaling_models_observe_every_function_body_read(self):
        self.assertEqual(list(benchmark.function_body_workloads()), list(benchmark.selected_workloads('function-bodies')))
        for (name, source), depth in zip(benchmark.function_body_workloads(), (0, 16, 48)):
            case = self.case(name)
            self.assertEqual((case.path / 'Makefile').read_bytes(), source)
            self.assertEqual(case.args, tuple(benchmark.workload_arguments(name)))
            self.assertEqual(source.count(b':= $(F0)'), 1000)
            value = b'[' * depth + b'end' + b']' * depth
            output = b'READS=[' + b'|'.join([value] * 1000) + b"]\nmake: 'all' is up to date.\n"
            good = h.Observation(output, b'', 0, None, {})
            self.assertEqual(h.expectation_errors(case, good), [])
            self.assertTrue(h.expectation_errors(case, dataclasses.replace(good, stdout=output.replace(b'end', b'WRONG', 1))))

if __name__ == '__main__':
    unittest.main()
