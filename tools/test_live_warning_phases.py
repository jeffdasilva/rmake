# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect ordered warning activation, active context and original evidence."""
import base64
import dataclasses
import gzip
import hashlib
import json
from pathlib import Path
import re
import tomllib
import unittest
import conformance as h

ROOT = Path(__file__).resolve().parents[1]


class LiveWarningPhaseTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.proof = json.loads((ROOT / 'tests/research/live-warning-phases.json').read_text())
        cls.records = {row['id']: row for row in cls.proof['records']}

    def test_inventory_raw_inputs_and_all_frozen_observations_are_qualified(self):
        ledger = tomllib.loads((ROOT / 'tests/requirements.toml').read_text())
        family = next(row for row in ledger['requirements'] if row['id'] == 'options.live-warning-phases')
        self.assertEqual((family['fixture_count'], family['candidate_required']), (1213, 1213))
        self.assertEqual(set(family['cases']), set(self.records))
        self.assertEqual(sum(bool(r['baseline']['candidate_differences']) for r in self.records.values()), 713)
        for row in self.records.values():
            case = h.load_case(ROOT / 'tests/cases' / row['id'])
            self.assertEqual((case.args, case.environment, case.expected),
                             (tuple(row['args']), row['environment'], row['expected']))
            self.assertEqual(case.output, row.get('output', 'separate'))
            self.assertEqual({str(p.relative_to(case.path)) for p in case.path.rglob('*')
                              if p.is_file() and p.name != 'case.toml'}, set(row['inputs']))
            for name, value in row['inputs'].items():
                path = case.path / name
                self.assertEqual((path.stat().st_mode & 0o777, path.read_bytes()),
                                 (value['mode'], bytes.fromhex(value['hex'])))
            for build in ('baseline', 'debug', 'release'):
                r = row[build]
                self.assertEqual((r['model_errors'][:2], r['oracle_repeat_differences'], r['bounds']),
                                 ([[], []], [], [False] * 3))
                if build != 'baseline':
                    self.assertEqual((r['model_errors'], r['candidate_differences']), ([[], [], []], []))
                if 'expected_seen' in row:
                    for observed in r['observations']:
                        self.assertEqual(base64.b64decode(observed['filesystem']['work/seen']['content']),
                                         bytes.fromhex(row['expected_seen']))

    def test_scope_kind_byte_and_nested_matrix_is_complete(self):
        counts = {phase: 0 for phase in ('reader', 'eval', 'include', 'recipe', 'secondary')}
        for name in self.records:
            match = re.match(r'live-warning-(?:(?:nested|merged|cursor)-)?(reader|eval|include|recipe|secondary)-', name)
            if match:
                counts[match[1]] += 1
        self.assertEqual(set(counts.values()), {242})
        self.assertEqual(sum(name.startswith('live-warning-nested-') for name in self.records), 160)
        self.assertEqual(sum(name.startswith('live-warning-cursor-') for name in self.records), 60)
        merged = [r for r in self.records.values() if r.get('output') == 'merged']
        self.assertEqual(len(merged), 30)
        for row in merged:
            output = bytes.fromhex(row['expected']['stdout_hex'])
            self.assertLess(output.index(b'AT_A\n'), output.index(b'AT_B\n'))
            self.assertEqual(row['expected']['stderr_hex'], '')
        for kind in ('simple', 'immediate', 'recursive', 'conditional'):
            self.assertEqual(sum('-' + kind + '-' in name for name in self.records), 240)
        byte_rows = [row for name, row in self.records.items() if '-b1' in name]
        self.assertEqual(len(byte_rows), 590)
        for row in byte_rows:
            self.assertTrue(any(b'\xff' in bytes.fromhex(value['hex']) for value in row['inputs'].values()))

    def test_override_replay_and_conditional_skips_keep_distinct_file_witnesses(self):
        replay = 0
        for name, row in self.records.items():
            if 'expected_seen' not in row:
                continue
            seen = bytes.fromhex(row['expected_seen'])
            stderr = bytes.fromhex(row['expected']['stderr_hex'])
            if seen.endswith(b'DONE\nA\nB\n'):
                replay += 1
                self.assertIn('-o1-', name)
                self.assertTrue('-simple-' in name or '-immediate-' in name)
                self.assertEqual(stderr.count(b'make: warning: undefined variable'), 2)
                self.assertIn(b'SEEN=[anchor\nA\nB\nDONE]', bytes.fromhex(row['expected']['stdout_hex']))
            if '-conditional-' in name:
                self.assertEqual(seen, b'anchor\nDONE\n')
                self.assertEqual(stderr, b'')
            if name.startswith('live-warning-nested-') and '-q1' in name:
                self.assertEqual(seen, b'anchor\nB\nDONE\n')
        self.assertEqual(replay, 144)

    def test_exact_historical_sources_are_closed_and_output_models_reject_changes(self):
        history = json.loads((ROOT / 'tests/research/live-warning-phase-discovery.json').read_text())['records']
        for old in history:
            suffix = old['id'].rsplit('-', 1)[1]
            row = self.records['live-warning-recorded-' + suffix]
            self.assertEqual((row['inputs'], row['args'], row['environment']),
                             (old['inputs'], old['args'], old['env']))
            self.assertTrue(row['baseline']['candidate_differences'])
            case = h.load_case(ROOT / 'tests/cases' / row['id'])
            good = h.Observation(bytes.fromhex(row['expected']['stdout_hex']),
                                 bytes.fromhex(row['expected']['stderr_hex']), 0, None, {})
            self.assertEqual(h.expectation_errors(case, good), [])
            self.assertTrue(h.expectation_errors(case, dataclasses.replace(good, stdout=b'LOST\n')))
            changed = good.stderr.replace(b'Makefile:2:', b'other.mk:2:') if good.stderr else b'unexpected warning\n'
            self.assertTrue(h.expectation_errors(case, dataclasses.replace(good, stderr=changed)))
        self.assertEqual(self.records['live-warning-recorded-late']['expected']['stderr_hex'], '')

    def test_all_rejected_hypotheses_remain_unchanged_without_credit(self):
        rejected = self.proof['discarded_models']['records']
        self.assertEqual(len(rejected), 275)
        for row in rejected:
            self.assertNotIn(row['id'], self.records)
            self.assertTrue(all(row['model_errors'][:2]))
            self.assertEqual(row['oracle_repeat_differences'], [])
            self.assertEqual(row['bounds'], [False] * 3)
            self.assertTrue(row['inputs'])
        self.assertEqual(sum('-eager-effects' in name for name in self.records), 264)

    def test_unselected_capture_prototype_keeps_its_complete_proof_and_source(self):
        manifest = json.loads((ROOT / 'tests/research/live-warning-capture-prototype.json').read_text())
        packed = (ROOT / manifest['raw_observations']).read_bytes()
        raw = gzip.decompress(packed)
        self.assertEqual(hashlib.sha256(packed).hexdigest(), manifest['compressed_sha256'])
        self.assertEqual(hashlib.sha256(raw).hexdigest(), manifest['uncompressed_sha256'])
        self.assertEqual(hashlib.sha256(manifest['source_patch'].encode()).hexdigest(), manifest['source_patch_sha256'])
        old = json.loads(raw)
        self.assertEqual((len(old['records']), old['baseline_mismatches']), (1153, 653))
        for row in old['records']:
            chosen = self.records[row['id']]
            for key in ('inputs', 'args', 'environment', 'expected'):
                self.assertEqual(row[key], chosen[key])
            self.assertEqual(row.get('output', 'separate'), chosen.get('output', 'separate'))
            for build in ('debug', 'release'):
                r = row[build]
                self.assertEqual((r['model_errors'], r['candidate_differences'], r['bounds']),
                                 ([[], [], []], [], [False] * 3))


if __name__ == '__main__':
    unittest.main()
