# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect original live arity models, skipped effects and continuation order."""
import base64
import json
from pathlib import Path
import tomllib
import unittest
import conformance as h

ROOT = Path(__file__).resolve().parents[1]


class LiveOptionArityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.proof = json.loads((ROOT / 'tests/research/live-option-arity.json').read_text())
        cls.records = {row['id']: row for row in cls.proof['records']}

    def test_inventory_inputs_and_frozen_observations_are_qualified(self):
        ledger = tomllib.loads((ROOT / 'tests/requirements.toml').read_text())
        family = next(row for row in ledger['requirements'] if row['id'] == 'options.live-arity')
        self.assertEqual((family['candidate_required'], family['fixture_count']), (770, 770))
        self.assertEqual(set(family['cases']), set(self.records))
        self.assertEqual(self.proof['baseline_mismatches'], 700)
        for row in self.records.values():
            case = h.load_case(ROOT / 'tests/cases' / row['id'])
            self.assertEqual((case.args, case.environment, case.expected),
                             (tuple(row['args']), row['environment'], row['expected']))
            for name, value in row['inputs'].items():
                path = case.path / name
                self.assertEqual((path.stat().st_mode & 0o777, path.read_bytes()),
                                 (value['mode'], bytes.fromhex(value['hex'])))
            for build in ('baseline', 'debug', 'release'):
                observed = row[build]
                self.assertEqual((observed['model_errors'][:2], observed['oracle_repeat_differences'], observed['bounds']),
                                 ([[], []], [], [False] * 3))
                if build != 'baseline':
                    self.assertEqual((observed['candidate_differences'], observed['model_errors']), ([], [[], [], []]))

    def test_phase_and_operand_matrix_is_complete(self):
        for phase in ('reader', 'eval', 'include', 'recipe', 'secondary'):
            rows = [row for name, row in self.records.items()
                    if name.endswith('-' + phase) or name.endswith('-' + phase + '-payload')]
            self.assertEqual(len(rows), 140)
            self.assertEqual(sum('-payload' in row['id'] for row in rows), 70)
        self.assertEqual(sum('-noarg-' in name for name in self.records), 484)
        self.assertEqual(sum('-missing-long-' in name for name in self.records), 154)
        self.assertEqual(sum('-missing-short-' in name for name in self.records), 66)
        self.assertEqual(sum('-missing-cluster-' in name for name in self.records), 66)

    def test_rejected_options_preserve_builtins_origins_and_file_order(self):
        for row in self.records.values():
            active_payload = row['id'].endswith('-payload')
            value = b'after' if active_payload else b'before'
            expected = b'anchor\nEND:' + value + b'\n'
            self.assertEqual(bytes.fromhex(row['expected_seen']), expected)
            output = bytes.fromhex(row['expected']['stdout_hex'])
            self.assertIn(b'CC=[default]', output)
            self.assertIn(b'O=[file]', output)
            self.assertIn(b'F=[simple]' if active_payload else b'F=[recursive]', output)
            for build in ('debug', 'release'):
                for observed in row[build]['observations']:
                    self.assertEqual(base64.b64decode(observed['filesystem']['work/seen']['content']), expected)

    def test_inactive_controls_and_valid_cluster_letters_are_distinct(self):
        for name, row in self.records.items():
            inactive = name.endswith('-inactive')
            self.assertEqual(bool(row['baseline']['candidate_differences']), not inactive)
            output = bytes.fromhex(row['expected']['stdout_hex'])
            if '-missing-cluster-' in name:
                self.assertIn(b'MFLAGS=[-r ' if inactive else b'MFLAGS=[-rs ', output)
            if '-noarg-no-silent-' in name:
                self.assertIn(b'MFLAGS=[-rs ', output)
            if '-noarg-stop-' in name or '-noarg-no-keep-going-' in name:
                self.assertIn(b'MFLAGS=[-kr ', output)

    def test_exact_output_models_reject_lost_effects_and_changed_metadata(self):
        for name in ('live-arity-noarg-no-builtin-variables-literal-reader-payload',
                     'live-arity-missing-cluster-si-secondary-payload'):
            row = self.records[name]
            case = h.load_case(ROOT / 'tests/cases' / name)
            output = bytes.fromhex(row['expected']['stdout_hex'])
            self.assertEqual(h.expectation_errors(case, h.Observation(output, b'', 0, None, {})), [])
            for changed in (output.replace(b'END:', b'LOST:'),
                            output.replace(b'CC=[default]', b'CC=[undefined]')):
                self.assertTrue(h.expectation_errors(case, h.Observation(changed, b'', 0, None, {})))


if __name__ == '__main__':
    unittest.main()
