# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Independent search-stage witnesses and deliberately corrupted controls."""
import base64
import dataclasses
import hashlib
import json
from pathlib import Path
import tomllib
import unittest
import conformance as h
from test_runtime_home import archive
ROOT = Path(__file__).resolve().parents[1]


class SecondarySearchTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.proof = archive('secondary-include-search')
        cls.history = archive('secondary-include-discovery')
        cls.records = {r['id']: r for r in cls.proof['records']}

    def test_all_inputs_models_bounds_repeats_and_ordered_witnesses(self):
        family = next(r for r in tomllib.loads((ROOT / 'tests/requirements.toml').read_text())['requirements'] if r['id'] == 'options.secondary-include-search')
        self.assertEqual((family['fixture_count'], family['candidate_required']), (474, 474))
        self.assertEqual(set(family['cases']), set(self.records))
        self.assertEqual(self.proof['baseline_mismatches'], 240)
        self.assertEqual(sum(bool(r['baseline']['candidate_differences']) for r in self.records.values()), 240)
        for row in self.records.values():
            case = h.load_case(ROOT / 'tests/cases' / row['id'])
            self.assertEqual((case.args, case.environment, case.expected), (tuple(row['args']), row['environment'], row['expected']))
            files = {str(p.relative_to(case.path)): {'mode': p.stat().st_mode & 0o777, 'hex': p.read_bytes().hex()} for p in case.path.rglob('*') if p.is_file() and p.name != 'case.toml'}
            self.assertEqual(files, row['inputs'])
            for build in ('baseline', 'debug', 'release'):
                r = row[build]
                self.assertEqual((r['model_errors'][:2], r['oracle_repeat_differences'], r['bounds']), ([[], []], [], [False] * 3))
                if build != 'baseline':
                    self.assertEqual((r['model_errors'], r['candidate_differences']), ([[], [], []], []))
                for i, o in enumerate(r['observations']):
                    self.assertFalse(o['limit'])
                    self.assertFalse(o['leftover_processes'])
                    if i < 2 or build != 'baseline' or not r['candidate_differences']:
                        self.assertEqual(base64.b64decode(o['filesystem']['work/seen']['content']), bytes.fromhex(row['expected_seen']))

    def test_phase_boundaries_have_independent_effect_anchors(self):
        for phase in ('reader', 'eval', 'include', 'secondary', 'recipe', 'gnu'):
            matrix = [r for n, r in self.records.items() if n.startswith('secondary-search-' + phase + '-')]
            self.assertEqual(len(matrix), 64)
            for r in matrix:
                seen = bytes.fromhex(r['expected_seen'])
                self.assertEqual(seen.count(b'HOME\n'), 1 if phase == 'gnu' or '-simple-' in r['id'] else (4 if '-r3-' in r['id'] else 2))
                self.assertEqual(seen.count(b'INCLUDE-'), int(phase != 'gnu'))
        for prefix, count in (('home-secondary-search-', 16), ('secondary-search-path-', 16), ('secondary-search-nested-', 40), ('secondary-search-parent-', 12), ('secondary-search-fatal-', 4), ('secondary-search-invalid-sync-', 2)):
            self.assertEqual(sum(n.startswith(prefix) for n in self.records), count)
        for n, r in self.records.items():
            if n.startswith('secondary-search-fatal-'):
                self.assertEqual((r['expected']['exit_code'], bytes.fromhex(r['expected_seen'])), (2, b'anchor\nHOME\nA\nHOME\n'))
            if n.startswith('secondary-search-invalid-sync-'):
                self.assertEqual(bytes.fromhex(r['expected_seen']), b'anchor\nHOME\nA\n')

    def test_rejected_hypotheses_keep_raw_failures_without_acceptance_credit(self):
        def original(name): return json.loads(bytes.fromhex(self.history['files'][name]['hex']))
        self.assertEqual(sum(any(r['model_errors'][:2]) for r in original('matrix-first-prototype.json')['records']), 96)
        failed = original('controls-prototype.json')['records']
        self.assertEqual(sum(bool(r['candidate_differences']) for r in failed), 10)
        self.assertTrue(all(not any(r['model_errors'][:2]) and not any(r['bounds']) for r in failed))
        crashes = original('parent-prototype.json')['records']
        hangs = original('parent-refined-prototype.json')['records']
        self.assertEqual(len(crashes), 8)
        self.assertEqual(len(hangs), 8)
        self.assertTrue(all([o['returncode'] for o in r['observations'][:2]] == [-11, -11] for r in crashes))
        self.assertTrue(all(r['bounds'] == [True, True, False] for r in hangs))
        self.assertFalse(any(p.endswith('.so') for p in self.history['files']))
        self.assertEqual(len(self.history['source_files']), 5)
        for value in self.history['source_files'].values():
            self.assertEqual(hashlib.sha256(bytes.fromhex(value['hex'])).hexdigest(), value['sha256'])
        for build in ('debug', 'release'):
            selected = self.history['binaries']['/tmp/rmake-secondary-search-selected-' + build]
            self.assertIn(selected, self.proof['identities'][build].values())


    def test_wrong_stream_status_and_effect_order_cannot_pass(self):
        for r in list(self.records.values())[::17]:
            case = h.load_case(ROOT / 'tests/cases' / r['id'])
            good = h.Observation(bytes.fromhex(r['expected']['stdout_hex']), bytes.fromhex(r['expected']['stderr_hex']), r['expected']['exit_code'], None, {})
            self.assertEqual(h.expectation_errors(case, good), [])
            for changed in (dataclasses.replace(good, stdout=good.stdout+b'bad'), dataclasses.replace(good, stderr=good.stderr+b'bad'), dataclasses.replace(good, returncode=good.returncode+1)):
                self.assertTrue(h.expectation_errors(case, changed))
        good = h.Observation(b'', b'', 0, None, {'seen': b'HOME\nA\nHOME\nINCLUDE\n'.hex()})
        for value in (b'HOME\nHOME\nA\nINCLUDE\n', b'HOME\nA\nINCLUDE\n', b'HOME\nA\nHOME\n'):
            self.assertTrue(h.differences(good, dataclasses.replace(good, filesystem={'seen': value.hex()})))


if __name__ == '__main__':
    unittest.main()
