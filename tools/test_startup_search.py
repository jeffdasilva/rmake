# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect startup phase ordering, raw argv and retained rejected evidence."""
import base64
import dataclasses
import hashlib
import json
import os
from pathlib import Path
import tomllib
import unittest
import conformance as h
from test_runtime_home import archive
ROOT = Path(__file__).resolve().parents[1]


class StartupSearchTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.proof = archive('startup-include-search')
        cls.history = archive('startup-include-discovery')
        cls.rows = cls.proof['records']

    def test_all_inputs_hex_argv_models_and_bounded_observations(self):
        family = next(r for r in tomllib.loads((ROOT / 'tests/requirements.toml').read_text())['requirements'] if r['id'] == 'startup.second-include-search')
        self.assertEqual((len(self.rows), family['fixture_count'], family['candidate_required'], self.proof['baseline_mismatches']), (500, 500, 500, 438))
        self.assertEqual(set(family['cases']), {r['id'] for r in self.rows})
        self.assertEqual(sum(r['hex_manifest'] for r in self.rows), 192)
        self.assertEqual(sum(bool(r['baseline']['candidate_differences']) for r in self.rows), 438)
        for row in self.rows:
            case = h.load_case(ROOT / 'tests/cases' / row['id'])
            self.assertEqual((case.args, case.environment, case.expected), (tuple(row['args']), row['environment'], row['expected']))
            self.assertEqual([os.fsencode(arg).hex() for arg in case.args], row['args_hex'])
            manifest = tomllib.loads((case.path / 'case.toml').read_text())
            self.assertEqual('args_hex' in manifest, row['hex_manifest'])
            self.assertEqual({str(p.relative_to(case.path)): {'mode': p.stat().st_mode & 0o777, 'hex': p.read_bytes().hex()} for p in case.path.rglob('*') if p.is_file() and p.name != 'case.toml'}, row['inputs'])
            for build in ('baseline', 'debug', 'release'):
                r = row[build]
                self.assertEqual((r['model_errors'][:2], r['oracle_repeat_differences'], r['bounds']), ([[], []], [], [False] * 3))
                if build != 'baseline': self.assertEqual((r['model_errors'], r['candidate_differences']), ([[], [], []], []))
                for i, o in enumerate(r['observations']):
                    self.assertFalse(o['limit'])
                    self.assertFalse(o['leftover_processes'])
                    if i < 2 or build != 'baseline' or not r['candidate_differences']:
                        self.assertEqual(base64.b64decode(o['filesystem']['work/seen']['content']), bytes.fromhex(row['expected_seen']))

    def test_independent_phase_callback_and_error_witnesses(self):
        for prefix, count in (('startup-search-recursive-', 192), ('startup-search-simple-', 192), ('startup-search-nested-', 16), ('startup-search-fatal-', 8), ('startup-search-deferred-sync-unexport-', 4), ('startup-search-path-', 8), ('startup-search-fallback-', 16), ('startup-search-metadata-', 16), ('startup-search-callback-', 32), ('startup-search-home-nul-', 16)):
            self.assertEqual(sum(r['id'].startswith(prefix) for r in self.rows), count)
        for r in self.rows:
            seen = bytes.fromhex(r['expected_seen'])
            if '-fatal-invalid-sync-' in r['id']:
                self.assertEqual((r['expected']['exit_code'], seen), (2, b'anchor\nHOME\nLATE\nFORBIDDEN\nFORBIDDEN\nLATE\n'))
            if r['id'].startswith('startup-search-deferred-sync-unexport-'):
                self.assertEqual((r['expected']['exit_code'], seen), (0, b'anchor\nHOME\nLATE\nFORBIDDEN\nFORBIDDEN\n'))
            if r['id'].startswith('startup-search-callback-'):
                self.assertEqual(seen, b'anchor\nHOME\nLATE\nINCLUDE-new\n')

    def test_over_eager_prototype_and_original_models_remain_unchanged(self):
        def original(name): return json.loads(bytes.fromhex(self.history['files'][name]['hex']))
        rows = original('controls-first-prototype.json')['records']
        self.assertEqual(sum(any(r['model_errors'][:2]) for r in rows), 20)
        self.assertEqual(sum(bool(r['candidate_differences']) for r in rows), 24)
        self.assertEqual(sum(base64.b64decode(r['observations'][0]['filesystem']['work/seen']['content']) != bytes.fromhex(r['expected_seen']) for r in rows), 8)
        self.assertTrue(all(not r['oracle_repeat_differences'] and not any(r['bounds']) for r in rows))
        saved = original('over-eager-source/identity.json')
        for filename, identity in saved['files'].items():
            self.assertEqual(hashlib.sha256(bytes.fromhex(self.history['files']['over-eager-source/' + filename]['hex'])).hexdigest(), identity['sha256'])
        self.assertEqual(len(original('first-baseline.json')['records']), 24)
        self.assertFalse(any(p.endswith('.so') for p in self.history['files']))
        for value in self.history['source_files'].values():
            self.assertEqual(hashlib.sha256(bytes.fromhex(value['hex'])).hexdigest(), value['sha256'])
        for build in ('debug', 'release'):
            self.assertIn(self.history['binaries']['/tmp/rmake-startup-search-final-' + build], self.proof['identities'][build].values())

    def test_corrupted_stream_status_and_ordered_effects_cannot_pass(self):
        for row in self.rows[::19]:
            case = h.load_case(ROOT / 'tests/cases' / row['id'])
            good = h.Observation(bytes.fromhex(row['expected']['stdout_hex']), bytes.fromhex(row['expected']['stderr_hex']), row['expected']['exit_code'], None, {})
            self.assertEqual(h.expectation_errors(case, good), [])
            for changed in (dataclasses.replace(good, stdout=good.stdout+b'bad'), dataclasses.replace(good, stderr=good.stderr+b'bad'), dataclasses.replace(good, returncode=good.returncode+1)):
                self.assertTrue(h.expectation_errors(case, changed))
        good = h.Observation(b'', b'', 0, None, {'seen': b'HOME\nLATE\nINCLUDE\n'.hex()})
        for value in (b'HOME\nINCLUDE\nLATE\n', b'HOME\nINCLUDE\n', b'HOME\nLATE\nLATE\nINCLUDE\n'):
            self.assertTrue(h.differences(good, dataclasses.replace(good, filesystem={'seen': value.hex()})))


if __name__ == '__main__': unittest.main()
