# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Independent witnesses and negative controls for contextual include operands."""
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


class RuntimeHomeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.proof = archive('runtime-home-includes')
        cls.history = archive('runtime-home-discovery')
        cls.records = {r['id']: r for r in cls.proof['records']}

    def test_inventory_input_bytes_and_every_frozen_observation(self):
        family = next(r for r in tomllib.loads((ROOT / 'tests/requirements.toml').read_text())['requirements']
                      if r['id'] == 'options.runtime-home-includes')
        self.assertEqual((family['fixture_count'], family['candidate_required']), (460, 460))
        self.assertEqual(set(family['cases']), set(self.records))
        self.assertEqual(self.proof['baseline_mismatches'], 398)
        self.assertEqual(sum(bool(r['baseline']['candidate_differences']) for r in self.records.values()), 398)
        for row in self.records.values():
            case = h.load_case(ROOT / 'tests/cases' / row['id'])
            self.assertEqual((case.args, case.environment, case.output, case.expected),
                             (tuple(row['args']), row['environment'], row['output'], row['expected']))
            self.assertEqual({str(p.relative_to(case.path)) for p in case.path.rglob('*')
                              if p.is_file() and p.name != 'case.toml'}, set(row['inputs']))
            for filename, value in row['inputs'].items():
                path = case.path / filename
                self.assertEqual((path.stat().st_mode & 0o777, path.read_bytes()), (value['mode'], bytes.fromhex(value['hex'])))
            for build in ('baseline', 'debug', 'release'):
                result = row[build]
                self.assertEqual((result['model_errors'][:2], result['oracle_repeat_differences'], result['bounds']), ([[], []], [], [False] * 3))
                if build != 'baseline':
                    self.assertEqual((result['model_errors'], result['candidate_differences']), ([[], [], []], []))
                for index, observation in enumerate(result['observations']):
                    self.assertFalse(observation['limit'])
                    self.assertFalse(observation['leftover_processes'])
                    if index < 2 or build != 'baseline' or not result['candidate_differences']:
                        self.assertEqual(base64.b64decode(observation['filesystem']['work/seen']['content']), bytes.fromhex(row['expected_seen']))

    def test_phase_policy_byte_callbacks_and_ignored_operands_have_independent_anchors(self):
        for phase in ('reader', 'eval', 'include', 'secondary', 'recipe', 'gnu'):
            self.assertEqual(sum(n.startswith('owned-home-' + phase + '-') and '-parallel-' not in n for n in self.records), 64)
        self.assertEqual(sum('-parallel-' in n for n in self.records), 24)
        self.assertEqual(sum(n.startswith('owned-home-callback-') for n in self.records), 16)
        ignored = [r for n, r in self.records.items() if n.startswith('owned-home-ignored-')]
        self.assertEqual(len(ignored), 16)
        for row in ignored:
            self.assertEqual(bytes.fromhex(row['expected_seen']), b'anchor\nHOME\nINCLUDE\n')
        fatal = [r for n, r in self.records.items() if n.startswith('owned-home-fatal-')]
        self.assertEqual(len(fatal), 4)
        for row in fatal:
            self.assertEqual((row['expected']['exit_code'], bytes.fromhex(row['expected_seen'])), (2, b'anchor\nHOME\n'))
            self.assertNotIn(b'safety error', bytes.fromhex(row['expected']['stderr_hex']))

    def test_rejected_models_and_first_prototype_failures_are_not_rewritten(self):
        def original(name): return json.loads(bytes.fromhex(self.history['files'][name]['hex']))
        for name in ('owned-home/first-debug.json', 'owned-home/refined-debug.json'):
            records = original(name)['records']
            self.assertEqual(sum(any(r['model_errors'][:2]) for r in records), 160)
        controls = original('owned-home/ignored-first-debug.json')['records']
        self.assertEqual(len(controls), 16)
        self.assertTrue(all(r['candidate_differences'] == ['filesystem'] for r in controls))
        for row in controls:
            self.assertEqual(base64.b64decode(row['observations'][0]['filesystem']['work/seen']['content']), b'anchor\nHOME\nINCLUDE\n')
            self.assertEqual(base64.b64decode(row['observations'][2]['filesystem']['work/seen']['content']).count(b'HOME\n'), 2)
        self.assertIn('owned-home/first-source/crates/rmake/src/cli.rs', self.history['files'])
        self.assertEqual(len(self.history['final_source_files']), 3)
        for source in self.history['final_source_files'].values():
            self.assertEqual(hashlib.sha256(bytes.fromhex(source['hex'])).hexdigest(), source['sha256'])

    def test_changed_streams_status_and_effect_order_cannot_pass(self):
        for row in list(self.records.values())[::19]:
            case = h.load_case(ROOT / 'tests/cases' / row['id'])
            good = h.Observation(bytes.fromhex(row['expected']['stdout_hex']), bytes.fromhex(row['expected']['stderr_hex']), row['expected']['exit_code'], None, {})
            self.assertEqual(h.expectation_errors(case, good), [])
            for changed in (dataclasses.replace(good, stdout=good.stdout + b'bad'), dataclasses.replace(good, stderr=good.stderr + b'bad'), dataclasses.replace(good, returncode=good.returncode + 1)):
                self.assertTrue(h.expectation_errors(case, changed))
        a = h.Observation(b'', b'', 0, None, {'seen': b'HOME\nINCLUDE\n'.hex()})
        for value in (b'INCLUDE\nHOME\n', b'HOME\n', b'HOME\nHOME\nINCLUDE\n'):
            self.assertTrue(h.differences(a, dataclasses.replace(a, filesystem={'seen': value.hex()})))


if __name__ == '__main__':
    unittest.main()
