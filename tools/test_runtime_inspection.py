# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect real recipe behavior and measured parsing provenance."""
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


class RuntimeInspectionTests(unittest.TestCase):
    def test_input_inventory_raw_observations_and_negative_recipe_controls(self):
        proof = archive('runtime-flag-inspection')
        family = next(r for r in tomllib.loads((ROOT / 'tests/requirements.toml').read_text())['requirements'] if r['id'] == 'performance.runtime-flag-inspection')
        rows = proof['records']
        self.assertEqual((len(rows), family['fixture_count'], family['candidate_required'], proof['baseline_mismatches']), (96, 96, 96, 0))
        self.assertEqual(set(family['cases']), {r['id'] for r in rows})
        for row in rows:
            case = h.load_case(ROOT / 'tests/cases' / row['id'])
            self.assertEqual((case.args, case.environment, case.expected), (tuple(row['args']), row['environment'], row['expected']))
            self.assertEqual({str(p.relative_to(case.path)): {'mode': p.stat().st_mode & 0o777, 'hex': p.read_bytes().hex()} for p in case.path.rglob('*') if p.is_file() and p.name != 'case.toml'}, row['inputs'])
            for build in ('baseline', 'debug', 'release'):
                r = row[build]
                self.assertEqual((r['model_errors'], r['oracle_repeat_differences'], r['candidate_differences'], r['bounds']), ([[], [], []], [], [], [False] * 3))
                for o in r['observations']:
                    self.assertFalse(o['limit'])
                    self.assertFalse(o['leftover_processes'])
                    self.assertEqual(base64.b64decode(o['filesystem']['work/seen']['content']), bytes.fromhex(row['expected_seen']))
            good = h.Observation(bytes.fromhex(row['expected']['stdout_hex']), bytes.fromhex(row['expected']['stderr_hex']), row['expected']['exit_code'], None, {})
            self.assertEqual(h.expectation_errors(case, good), [])
            for changed in (dataclasses.replace(good, stdout=good.stdout+b'bad'), dataclasses.replace(good, stderr=good.stderr+b'bad'), dataclasses.replace(good, returncode=0)):
                self.assertTrue(h.expectation_errors(case, changed))
        a = h.Observation(b'', b'', 2, None, {'seen': b'anchor\nfirst\nsecond\n'.hex()})
        self.assertTrue(h.differences(a, dataclasses.replace(a, filesystem={'seen': b'anchor\nfirst\n'.hex()})))

    def test_rejected_models_profiles_and_source_have_provenance(self):
        history = archive('runtime-inspection-discovery')
        def original(name): return json.loads(bytes.fromhex(history['files'][name]['hex']))
        first = original('first-debug.json')['records']
        self.assertEqual(sum(any(r['model_errors'][:2]) for r in first), 24)
        self.assertTrue(all(not r['candidate_differences'] and not any(r['bounds']) for r in first))
        self.assertFalse(any(p.endswith('.so') for p in history['files']))
        for v in history['source_files'].values():
            self.assertEqual(hashlib.sha256(bytes.fromhex(v['hex'])).hexdigest(), v['sha256'])
        proof = archive('runtime-flag-inspection')
        for build in ('debug', 'release'):
            self.assertIn(history['binaries']['/tmp/rmake-fused-inspection-selected-' + build], proof['identities'][build].values())
        summaries = {}
        for engine in ('preceding', 'selected'):
            samples = [original(f'profiles/{engine}-runtime-arguments-512-updates-100-{i}/report.json')['summary'] for i in (1, 2)]
            self.assertEqual(samples[0], samples[1])
            summaries[engine] = samples[0]
        self.assertEqual((summaries['preceding']['calls'], summaries['selected']['calls']), (21071, 20062))
        self.assertEqual(summaries['preceding']['frees'] - summaries['selected']['frees'], 1009)
        for batch in ('a', 'b'):
            for suffix, size in (('', 101), ('-pinned', 301)):
                data = original(f'bench/runtime-arguments{suffix}-{batch}.json')
                self.assertTrue(all(len(v['samples_ns']) == size for w in data['workloads'] for v in w['measurements'].values()))


if __name__ == '__main__':
    unittest.main()
