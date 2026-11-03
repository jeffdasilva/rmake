# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Keep exact shuffle acceptance separate from historical and random evidence."""
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


class ShuffleProofTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.proof = archive('shuffle-order')
        cls.history = archive('shuffle-research')
        cls.records = {r['id']: r for r in cls.proof['records']}

    def test_inventory_inputs_models_bounds_and_all_frozen_witnesses(self):
        family = next(r for r in tomllib.loads((ROOT / 'tests/requirements.toml').read_text())['requirements']
                      if r['id'] == 'options.shuffle')
        self.assertEqual((family['fixture_count'], family['candidate_required']), (1995, 1995))
        self.assertEqual(set(family['cases']), set(self.records))
        self.assertEqual(self.proof['baseline_mismatches'], 1995)
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
                self.assertEqual(bool(result['candidate_differences']), build == 'baseline')
                if build != 'baseline':
                    self.assertEqual(result['model_errors'], [[], [], []])
                for index, observation in enumerate(result['observations']):
                    self.assertFalse(observation['leftover_processes'])
                    if index < 2 or build != 'baseline':
                        self.assertEqual(base64.b64decode(observation['filesystem']['work/seen']['content']),
                                         bytes.fromhex(row['expected_seen']))

    def test_graph_mode_source_and_byte_matrix_remains_complete(self):
        names = [n for n in self.records if n.startswith('shuffle-matrix-')]
        self.assertEqual(len(names), 1368)
        for source in ('cli', 'gnu', 'make'):
            self.assertEqual(sum(f'-{source}-b' in n for n in names), 456)
        for n in names:
            raw = b''.join(bytes.fromhex(x['hex']) for x in self.records[n]['inputs'].values())
            self.assertEqual(b'\xff' in raw, '-b1' in n)
        self.assertEqual(sum('mixed-reverse' in n for n in names), 114)
        self.assertEqual(sum('slot-removal' in n for n in names), 36)
        self.assertEqual(sum(n.startswith('shuffle-parallel-') for n in self.records), 288)
        self.assertEqual(sum(n.startswith('shuffle-append-') for n in self.records), 32)
        self.assertEqual(sum(n.startswith('shuffle-invalid-') for n in self.records), 30)

    def test_automatic_declarations_and_active_requested_modes_have_exact_models(self):
        for n, row in self.records.items():
            out = bytes.fromhex(row['expected']['stdout_hex'])
            if n.startswith('shuffle-parallel-'):
                self.assertEqual(out, b'all PLUS=[a b c]\n')
                if n.endswith(('-wait', '-serial')):
                    self.assertEqual(bytes.fromhex(row['expected_seen']), b'anchor\na\nb\nc\nall\n')
            if n.startswith('shuffle-failure-'):
                mode = n.split('-')[2]
                self.assertEqual(b' shuffle=' in out, mode != 'none')
                if mode != 'none':
                    self.assertTrue(out.endswith(b' shuffle=' + mode.encode() + b'\n'))

    def test_rejected_original_models_and_duplicate_ids_are_preserved(self):
        def original(name):
            return json.loads(bytes.fromhex(self.history['files'][name]['hex']))
        matrix = original('matrix/baseline.json')['records']
        self.assertEqual((len(matrix), len({r['id'] for r in matrix})), (1368, 1254))
        self.assertEqual(sum(any(r['model_errors'][:2]) for r in matrix), 36)
        for name, count in (('late/prototype.json', 43), ('scopes/prototype.json', 2), ('branches/prototype.json', 2)):
            records = original(name)['records']
            self.assertEqual(sum(any(r['model_errors'][:2]) for r in records), count)
            self.assertTrue(all(not any(r['bounds']) and not r['oracle_repeat_differences'] for r in records))
        for build in ('debug', 'release'):
            records = original(f'selected-{build}.json')['records']
            self.assertEqual(sum(bool(r['candidate_differences']) for r in records), 2)
        self.assertIn('rmake-shuffle-matrix.py', self.history['model_scripts'])
        self.assertIn('refine-shuffle-phases.py', self.history['model_scripts'])

    def test_expected_models_reject_status_stream_and_declaration_corruption(self):
        for row in list(self.records.values())[::67]:
            case = h.load_case(ROOT / 'tests/cases' / row['id'])
            good = h.Observation(bytes.fromhex(row['expected']['stdout_hex']),
                                 bytes.fromhex(row['expected']['stderr_hex']), row['expected']['exit_code'], None, {})
            self.assertEqual(h.expectation_errors(case, good), [])
            for changed in (dataclasses.replace(good, stdout=good.stdout + b'bad'),
                            dataclasses.replace(good, stderr=good.stderr + b'bad'),
                            dataclasses.replace(good, returncode=good.returncode + 1)):
                self.assertTrue(h.expectation_errors(case, changed))


if __name__ == '__main__':
    unittest.main()
