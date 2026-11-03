# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Qualified original reader-side module cases and independently ordered witnesses."""
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


class LoadReaderTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.proof = archive('load-reader')
        cls.history = archive('load-reader-discovery')
        cls.rows = cls.proof['records']

    def test_required_inputs_and_all_three_independent_models(self):
        family = next(r for r in tomllib.loads((ROOT / 'tests/requirements.toml').read_text())['requirements'] if r['id'] == 'native.load-reader')
        self.assertEqual((len(self.rows), family['fixture_count'], family['candidate_required'], self.proof['baseline_mismatches']), (324, 324, 324, 312))
        self.assertEqual(set(family['cases']), {r['id'] for r in self.rows})
        self.assertEqual(sum(bool(r['baseline']['candidate_differences']) for r in self.rows), 312)
        for row in self.rows:
            case = h.load_case(ROOT / 'tests/cases' / row['id'])
            self.assertEqual((case.args, case.environment, case.expected), (tuple(row['args']), row['environment'], row['expected']))
            self.assertEqual({str(p.relative_to(case.path)): {'mode': p.stat().st_mode & 0o777, 'hex': p.read_bytes().hex()} for p in case.path.rglob('*') if p.is_file() and p.name != 'case.toml'}, row['inputs'])
            for build in ('baseline', 'debug', 'release'):
                r = row[build]
                self.assertEqual((r['model_errors'][:2], r['oracle_repeat_differences'], r['bounds']), ([[], []], [], [False] * 3))
                if build != 'baseline': self.assertEqual((r['model_errors'], r['candidate_differences']), ([[], [], []], []))
                for i, observation in enumerate(r['observations']):
                    self.assertFalse(observation['limit'] or observation['leftover_processes'])
                    if i < 2 or build != 'baseline' or not r['candidate_differences']:
                        self.assertEqual(base64.b64decode(observation['filesystem']['work/seen']['content']), bytes.fromhex(row['expected_seen']))

    def test_coverage_and_independent_phase_boundaries(self):
        for prefix, count in (('load-reader-load-', 96), ('load-reader-empty-symbol-', 48), ('load-reader-expansion-error-', 16), ('load-reader-prepared-', 112), ('load-reader-warning-', 16), ('load-reader-comment-', 4), ('load-reader-boundary-', 4), ('load-reader-nul-empty-', 16), ('load-reader-symbol-spelling-', 12)):
            self.assertEqual(sum(r['id'].startswith(prefix) for r in self.rows), count)
        rows = {r['id']: r for r in self.rows}
        for optional in ('false', 'true'):
            for kind in ('later', 'group', 'fatal-home'):
                row = rows[f'load-reader-prepared-{optional}-read-none-{kind}']
                self.assertEqual((row['expected']['exit_code'], bytes.fromhex(row['expected_seen'])), (2, b'anchor\nbefore\noperand\nHOME\n'))
            empty = rows['load-reader-load-' + ('optional' if optional == 'true' else 'required') + '-read-none-blank']
            self.assertEqual(bytes.fromhex(empty['expected_seen']), b'anchor\nbefore\nafter\nrecipe\nHOME\n')
        self.assertIn('No shared-library execution or native ABI credit', self.proof['classification'])
        self.assertFalse(any(name.endswith('.so') for r in self.rows for name in r['inputs']))

    def test_original_wrong_models_are_preserved_with_exact_observations(self):
        files = self.history['files']
        for value in files.values(): self.assertEqual(hashlib.sha256(bytes.fromhex(value['hex'])).hexdigest(), value['sha256'])
        def original(name): return json.loads(bytes.fromhex(files[name]['hex']))
        first = original('first-baseline.json')['records']
        self.assertEqual((len(first), sum(any(r['model_errors'][:2]) for r in first)), (160, 16))
        second = original('second-baseline.json')['records']
        self.assertEqual((len(second), sum(any(r['model_errors'][:2]) for r in second)), (296, 0))
        self.assertEqual(sum(base64.b64decode(r['observations'][0]['filesystem']['work/seen']['content']) != bytes.fromhex(r['expected_seen']) for r in second), 116)
        for r in first + second:
            self.assertFalse(r['oracle_repeat_differences'] or any(r['bounds']))
        self.assertEqual(hashlib.sha256(bytes.fromhex(self.history['source_patch_hex'])).hexdigest(), self.history['source_patch_sha256'])
        for build in ('debug', 'release'):
            self.assertIn(self.history['binaries']['/tmp/rmake-load-reader-selected-' + build], self.proof['identities'][build].values())

    def test_wrong_stream_status_and_effect_order_fail_comparison(self):
        for row in self.rows[::19]:
            case = h.load_case(ROOT / 'tests/cases' / row['id'])
            good = h.Observation(bytes.fromhex(row['expected']['stdout_hex']), bytes.fromhex(row['expected']['stderr_hex']), row['expected']['exit_code'], None, {})
            self.assertEqual(h.expectation_errors(case, good), [])
            for bad in (dataclasses.replace(good, stdout=good.stdout+b'bad'), dataclasses.replace(good, stderr=good.stderr+b'bad'), dataclasses.replace(good, returncode=good.returncode+1)):
                self.assertTrue(h.expectation_errors(case, bad))
        good = h.Observation(b'', b'', 2, None, {'seen': b'before\noperand\nHOME\n'.hex()})
        for value in (b'before\nHOME\noperand\n', b'before\noperand\n', b'before\noperand\nHOME\nHOME\n'):
            self.assertTrue(h.differences(good, dataclasses.replace(good, filesystem={'seen': value.hex()})))


if __name__ == '__main__': unittest.main()
