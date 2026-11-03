# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Independent effect witnesses and negative controls for output-mode ordering."""
import base64
import dataclasses
import gzip
import hashlib
import json
from pathlib import Path
import tomllib
import unittest

import conformance as h

ROOT = Path(__file__).resolve().parents[1]


def archive(name):
    manifest = json.loads((ROOT / f'tests/research/{name}.json').read_text())
    packed = (ROOT / f'tests/research/{name}.json.gz').read_bytes()
    raw = gzip.decompress(packed)
    assert hashlib.sha256(packed).hexdigest() == manifest['sha256']
    assert hashlib.sha256(raw).hexdigest() == manifest['uncompressed_sha256']
    assert len(raw) == manifest['uncompressed_bytes']
    return json.loads(raw)


class OutputValidationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.proof = archive('output-validation-order')
        cls.history = archive('output-validation-discovery')
        cls.records = {r['id']: r for r in cls.proof['records']}

    def test_inventory_inputs_and_every_raw_observation(self):
        family = next(r for r in tomllib.loads((ROOT / 'tests/requirements.toml').read_text())['requirements']
                      if r['id'] == 'options.output-validation-order')
        self.assertEqual((family['fixture_count'], family['candidate_required']), (818, 818))
        self.assertEqual(set(family['cases']), set(self.records))
        self.assertEqual(self.proof['baseline_mismatches'], 553)
        self.assertEqual(sum(bool(r['baseline']['candidate_differences']) for r in self.records.values()), 553)
        for row in self.records.values():
            case = h.load_case(ROOT / 'tests/cases' / row['id'])
            self.assertEqual((case.args, case.environment, case.output, case.expected),
                             (tuple(row['args']), row['environment'], row['output'], row['expected']))
            self.assertEqual({str(p.relative_to(case.path)) for p in case.path.rglob('*')
                              if p.is_file() and p.name != 'case.toml'}, set(row['inputs']))
            for filename, value in row['inputs'].items():
                path = case.path / filename
                self.assertEqual((path.stat().st_mode & 0o777, path.read_bytes()),
                                 (value['mode'], bytes.fromhex(value['hex'])))
            for build in ('baseline', 'debug', 'release'):
                result = row[build]
                self.assertEqual((result['model_errors'][:2], result['oracle_repeat_differences'], result['bounds']),
                                 ([[], []], [], [False] * 3))
                if build != 'baseline':
                    self.assertEqual((result['model_errors'], result['candidate_differences']), ([[], [], []], []))
                for index, observed in enumerate(result['observations']):
                    self.assertFalse(observed['limit'])
                    self.assertFalse(observed['leftover_processes'])
                    if index < 2 or build != 'baseline' or not result['candidate_differences']:
                        self.assertEqual(base64.b64decode(observed['filesystem']['work/seen']['content']),
                                         bytes.fromhex(row['expected_seen']))

    def test_phase_matrix_and_independent_ordered_effect_anchors(self):
        matrix = [r for n, r in self.records.items() if n.startswith('output-validation-')
                  and not n.startswith(('output-validation-gnu-', 'output-validation-source-effect-'))]
        self.assertEqual(len(matrix), 640)
        for phase in ('reader', 'eval', 'include', 'secondary', 'recipe'):
            self.assertEqual(sum(r['id'].startswith('output-validation-' + phase + '-') for r in matrix), 128)
        source_effects = [r for n, r in self.records.items() if n.startswith('output-validation-source-effect-')]
        self.assertEqual(len(source_effects), 48)
        for row in source_effects:
            good = '-g1-' in row['id']
            self.assertEqual(bytes.fromhex(row['expected_seen']),
                b'anchor\nREADY\nDONE\n' + (b'NEXT-cleared\nEND\nNEXT-cleared\n' if good else b''))
        pilots = [r for n, r in self.records.items() if n.startswith('decoder-order-')]
        self.assertEqual(len(pilots), 2)
        for row in pilots:
            self.assertEqual(bytes.fromhex(row['expected_seen']),
                b'anchor\nA\nB\n' + (b'END\n' if 'later-good' in row['id'] else b''))

    def test_original_wrong_models_and_refinements_are_distinct(self):
        def original(name):
            return json.loads(bytes.fromhex(self.history['files'][name]['hex']))
        first = original('output-validation/baseline.json')['records']
        self.assertEqual(sum(any(r['model_errors'][:2]) for r in first), 128)
        before_source = original('output-validation/final-debug.json')['records']
        self.assertEqual(sum(any(r['model_errors'][:2]) for r in before_source), 48)
        selected = original('output-validation/selected-debug.json')['records']
        wrong_effects = [r for r in selected if any(base64.b64decode(o['filesystem']['work/seen']['content'])
                         != bytes.fromhex(r['expected_seen']) for o in r['observations'][:2])]
        self.assertEqual(len(wrong_effects), 24)
        for row in wrong_effects:
            self.assertNotIn(row['id'], self.records)
            self.assertIn(row['id'] + '-export-reexpansion', self.records)
        self.assertIn('rmake-live-decoder-pilots.py', self.history['scripts'])

    def test_bad_streams_status_and_missing_reordered_effects_cannot_pass(self):
        for row in list(self.records.values())[::23]:
            case = h.load_case(ROOT / 'tests/cases' / row['id'])
            good = h.Observation(bytes.fromhex(row['expected']['stdout_hex']),
                                 bytes.fromhex(row['expected']['stderr_hex']), row['expected']['exit_code'], None, {})
            self.assertEqual(h.expectation_errors(case, good), [])
            for changed in (dataclasses.replace(good, stdout=good.stdout + b'bad'),
                            dataclasses.replace(good, stderr=good.stderr + b'bad'),
                            dataclasses.replace(good, returncode=good.returncode + 1)):
                self.assertTrue(h.expectation_errors(case, changed))
        # The normal comparator independently compares complete file bytes,
        # including order and duplicates, rather than just file presence.
        a = h.Observation(b'', b'', 0, None, {'work/seen': {'content': base64.b64encode(b'A\nB\n').decode()}})
        for value in (b'A\n', b'B\nA\n', b'A\nB\nB\n'):
            b = dataclasses.replace(a, filesystem={'work/seen': {'content': base64.b64encode(value).decode()}})
            self.assertTrue(h.differences(a, b))


if __name__ == '__main__':
    unittest.main()
