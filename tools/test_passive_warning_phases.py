# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect original post-read warning models and exact frozen observations."""
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


class PassiveWarningPhaseTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.proof = json.loads((ROOT / 'tests/research/passive-warning-phases.json').read_text())
        cls.records = {r['id']: r for r in cls.proof['records']}

    def test_inventory_inputs_and_complete_frozen_observations(self):
        ledger = tomllib.loads((ROOT / 'tests/requirements.toml').read_text())
        family = next(r for r in ledger['requirements'] if r['id'] == 'options.passive-warning-phases')
        self.assertEqual((family['fixture_count'], family['candidate_required']), (1872, 1872))
        self.assertEqual(set(family['cases']), set(self.records))
        self.assertEqual(sum(bool(r['baseline']['candidate_differences']) for r in self.records.values()), 288)
        for row in self.records.values():
            case = h.load_case(ROOT / 'tests/cases' / row['id'])
            self.assertEqual((case.args, case.environment, case.expected),
                             (tuple(row['args']), row['environment'], row['expected']))
            self.assertEqual({str(p.relative_to(case.path)) for p in case.path.rglob('*')
                              if p.is_file() and p.name != 'case.toml'}, set(row['inputs']))
            for name, value in row['inputs'].items():
                path = case.path / name
                self.assertEqual((path.stat().st_mode & 0o777, path.read_bytes()),
                                 (value['mode'], bytes.fromhex(value['hex'])))
            for build in ('baseline', 'debug', 'release'):
                observed = row[build]
                self.assertEqual((observed['model_errors'][:2], observed['oracle_repeat_differences'], observed['bounds']),
                                 ([[], []], [], [False] * 3))
                if build != 'baseline':
                    self.assertEqual((observed['model_errors'], observed['candidate_differences']), ([[], [], []], []))
                for observation in observed['observations']:
                    self.assertFalse(observation['leftover_processes'])
                    self.assertEqual(base64.b64decode(observation['filesystem']['work/seen']['content']),
                                     bytes.fromhex(row['expected_seen']))

    def test_phase_kind_policy_priority_environment_and_bytes_matrix(self):
        names = [name for name in self.records if name.startswith('passive-warning-')]
        for phase in ('reader', 'eval', 'include'):
            self.assertEqual(sum(name.startswith('passive-warning-' + phase + '-') for name in names), 576)
        for kind in ('simple', 'immediate', 'recursive', 'conditional'):
            self.assertEqual(sum('-' + kind + '-' in name for name in names), 432)
        for order in ('early', 'mid', 'late'):
            self.assertEqual(sum('-' + order + '-' in name for name in names), 576)
        for env_at in ('none', 'before', 'between', 'after'):
            self.assertEqual(sum('-e' + env_at + '-' in name for name in names), 432)
        for priority in ('none', 'file', 'cli'):
            self.assertEqual(sum('-p' + priority + '-' in name for name in names), 576)
        self.assertEqual(sum('-i1-' in name for name in names), 864)
        for name, row in self.records.items():
            raw = b''.join(bytes.fromhex(v['hex']) for v in row['inputs'].values())
            self.assertEqual(b'\xff' in raw, name.endswith('-b1'))
            if row['baseline']['candidate_differences']:
                self.assertIn('-i0-', name)
                self.assertTrue('-simple-' in name or '-immediate-' in name)
                self.assertTrue('-mid-' in name or '-late-' in name)
                self.assertEqual(row['baseline']['candidate_differences'], ['stderr'])

    def test_eager_priority_and_conditional_skip_keep_exact_effects(self):
        for name, row in self.records.items():
            if name.startswith('passive-source-warning-'):
                continue
            seen = bytes.fromhex(row['expected_seen'])
            stderr = bytes.fromhex(row['expected']['stderr_hex'])
            if '-simple-' in name or '-immediate-' in name:
                self.assertEqual(seen, b'anchor\nA\nB\nDONE\n')
                self.assertNotIn(b'Makefile:', stderr)
            elif '-conditional-' in name and '-pnone-' not in name:
                self.assertEqual(seen, b'anchor\nDONE\n')
                self.assertEqual(stderr, b'')
            elif '-pnone-' in name:
                self.assertEqual(seen, b'anchor\nA\nB\nDONE\n')
                self.assertEqual(stderr.count(b'Makefile:'), 2)
            if '-pcli-' in name:
                self.assertIn(b'A=[cliA]/command line/recursive B=[cliB]/command line/recursive',
                              bytes.fromhex(row['expected']['stdout_hex']))

    def test_model_anchors_reject_changed_streams_status_and_warning_location(self):
        for row in list(self.records.values())[::71]:
            case = h.load_case(ROOT / 'tests/cases' / row['id'])
            good = h.Observation(bytes.fromhex(row['expected']['stdout_hex']),
                                 bytes.fromhex(row['expected']['stderr_hex']), 0, None, {})
            self.assertEqual(h.expectation_errors(case, good), [])
            self.assertTrue(h.expectation_errors(case, dataclasses.replace(good, stdout=b'LOST\n')))
            self.assertTrue(h.expectation_errors(case, dataclasses.replace(good, returncode=2)))
            changed = good.stderr.replace(b'make:', b'Makefile:99:').replace(b'Makefile:', b'other.mk:')
            if not changed: changed = b'unexpected warning\n'
            self.assertTrue(h.expectation_errors(case, dataclasses.replace(good, stderr=changed)))

    def test_discovery_inputs_remain_separate_without_extra_credit(self):
        rows = self.proof['discovery']['records']
        self.assertEqual(len(rows), 9)
        self.assertEqual(sum(bool(r['candidate_differences']) for r in rows), 6)
        for row in rows:
            self.assertNotIn(row['id'], self.records)
            self.assertEqual((row['model_errors'][:2], row['oracle_repeat_differences'], row['bounds']),
                             ([[], []], [], [False] * 3))
            self.assertTrue(row['inputs'])

    def test_source_expansion_policy_is_captured_after_its_effects(self):
        rows = [r for name, r in self.records.items() if name.startswith('passive-source-warning-')]
        self.assertEqual(len(rows), 144)
        self.assertEqual(sum(bool(r['prototype']['candidate_differences']) for r in rows), 48)
        for row in rows:
            self.assertEqual(row['baseline']['candidate_differences'], [])
            self.assertEqual(bytes.fromhex(row['expected_seen']), b'anchor\nSOURCE\nA\nB\nDONE\n')
            stderr = bytes.fromhex(row['expected']['stderr_hex'])
            self.assertEqual(stderr.count(b'make: warning: undefined variable'), 2)
            self.assertNotIn(b'Makefile:', stderr)
            prototype = row['prototype']
            self.assertEqual((prototype['model_errors'][:2], prototype['oracle_repeat_differences'], prototype['bounds']),
                             ([[], []], [], [False] * 3))
            if prototype['candidate_differences']:
                self.assertIn('-i0-', row['id'])
                self.assertTrue('-mid-' in row['id'] or '-late-' in row['id'])
                self.assertEqual(prototype['candidate_differences'], ['stderr'])

    def test_unselected_prototype_keeps_complete_observations_and_identity(self):
        manifest = json.loads((ROOT / self.proof['unselected_prototype']).read_text())
        packed = (ROOT / manifest['raw_observations']).read_bytes()
        raw = gzip.decompress(packed)
        self.assertEqual(hashlib.sha256(packed).hexdigest(), manifest['compressed_sha256'])
        self.assertEqual(hashlib.sha256(raw).hexdigest(), manifest['uncompressed_sha256'])
        original = json.loads(raw)
        self.assertEqual(len(original['records']), 1728)
        self.assertEqual(manifest['source_commit'], 'e2bb5db2ec76a874356c713500b5f2ff574d252f')
        for row in original['records']:
            selected = self.records[row['id']]
            for field in ('inputs', 'args', 'environment', 'expected', 'expected_seen'):
                self.assertEqual(row[field], selected[field])
            for build in ('debug', 'release'):
                self.assertEqual(row[build]['candidate_differences'], [])


if __name__ == '__main__':
    unittest.main()
