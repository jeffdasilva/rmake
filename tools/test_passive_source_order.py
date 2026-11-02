# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect sequential passive effects, original models and complete evidence."""
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


class PassiveSourceOrderTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.proof = json.loads((ROOT / 'tests/research/passive-source-order.json').read_text())
        cls.records = {row['id']: row for row in cls.proof['records']}

    def test_inventory_inputs_and_all_frozen_observations(self):
        ledger = tomllib.loads((ROOT / 'tests/requirements.toml').read_text())
        family = next(row for row in ledger['requirements'] if row['id'] == 'options.passive-source-order')
        self.assertEqual((family['fixture_count'], family['candidate_required']), (1290, 1290))
        self.assertEqual(set(family['cases']), set(self.records))
        self.assertEqual(self.proof['baseline_mismatches'], 1290)
        for row in self.records.values():
            case = h.load_case(ROOT / 'tests/cases' / row['id'])
            self.assertEqual((case.args, case.environment, case.output, case.expected),
                             (tuple(row['args']), row['environment'], row['output'], row['expected']))
            self.assertEqual({str(p.relative_to(case.path)) for p in case.path.rglob('*')
                              if p.is_file() and p.name != 'case.toml'}, set(row['inputs']))
            for name, value in row['inputs'].items():
                path = case.path / name
                self.assertEqual((path.stat().st_mode & 0o777, path.read_bytes()),
                                 (value['mode'], bytes.fromhex(value['hex'])))
            for build in ('baseline', 'debug', 'release'):
                result = row[build]
                self.assertEqual((result['model_errors'][:2], result['oracle_repeat_differences'], result['bounds']),
                                 ([[], []], [], [False] * 3))
                if build == 'baseline':
                    self.assertTrue(result['candidate_differences'])
                else:
                    self.assertEqual((result['model_errors'], result['candidate_differences']), ([[], [], []], []))
                for index, observation in enumerate(result['observations']):
                    self.assertFalse(observation['leftover_processes'])
                    if index < 2 or build != 'baseline':
                        self.assertEqual(base64.b64decode(observation['filesystem']['work/seen']['content']),
                                         bytes.fromhex(row['expected_seen']))

    def test_phase_policy_priority_and_byte_matrix_is_complete(self):
        names = [name for name in self.records if name.startswith('passive-source-order-') and '-pilot-' not in name]
        self.assertEqual(len(names), 1248)
        for phase in ('reader', 'eval', 'include'):
            self.assertEqual(sum(name.startswith('passive-source-order-' + phase + '-') for name in names), 416)
        for kind in ('simple', 'immediate', 'recursive', 'conditional'):
            self.assertEqual(sum('-' + kind + '-' in name for name in names), 312)
        self.assertEqual(sum('-i1-' in name for name in names), 96)
        self.assertEqual(sum('-b1' in name for name in names), 576)
        for priority, count in (('none', 432), ('cli', 432), ('file', 384)):
            self.assertEqual(sum('-p' + priority + '-' in name for name in names), count)
        for env_at, count in (('none', 336), ('gnu', 336), ('make-start', 288), ('make-end', 288)):
            self.assertEqual(sum('-e' + env_at + '-' in name for name in names), count)
        for name in names:
            raw = b''.join(bytes.fromhex(v['hex']) for v in self.records[name]['inputs'].values())
            self.assertEqual(b'\xff' in raw, '-b1' in name)
            self.assertEqual(self.records[name]['output'], 'merged')

    def test_exact_effect_order_snapshots_and_stronger_values(self):
        for name, row in self.records.items():
            if name.startswith('passive-source-jobs-'):
                continue
            seen = bytes.fromhex(row['expected_seen'])
            output = bytes.fromhex(row['expected']['stdout_hex'])
            self.assertEqual(row['expected']['stderr_hex'], '')
            self.assertEqual(seen.count(b'FLAGS\n'), 1)
            self.assertTrue(seen.startswith(b'anchor\n') and seen.endswith(b'DONE\n'))
            if '-simple-' in name or '-immediate-' in name:
                self.assertEqual(seen, b'anchor\nA\nFLAGS\nB\nDONE\n')
            elif b'A\n' in seen and '-pilot-' not in name:
                self.assertLess(seen.index(b'FLAGS\n'), seen.index(b'A\n'))
            self.assertIn(b'STATE=[pending]', output)
            if '-pilot-' not in name:
                self.assertIn(b'GNU=[cleared] SEEN=[anchor\n', output)
                self.assertIn(b'STATE=[pending] GNU=[]\n', output)
            if '-pcli-' in name:
                self.assertIn(b'FLAGS A=[cliA]/command line/recursive', output)
                self.assertIn(b'RESULT B=[cliB]/command line/recursive', output)

    def test_rejected_locus_models_and_separate_refinements_are_unchanged(self):
        rejected = self.proof['discarded_models']['records']
        self.assertEqual(len(rejected), 132)
        for old in rejected:
            self.assertNotIn(old['id'], self.records)
            chosen = self.records[old['id'] + '-source-locus']
            self.assertEqual((old['inputs'], old['args'], old['env']),
                             (chosen['inputs'], chosen['args'], chosen['environment']))
            file = 'step.mk' if '-include-' in old['id'] else 'Makefile'
            lines = bytes.fromhex(old['inputs'][file]['hex']).splitlines()
            locus = next(index + 1 for index, text in enumerate(lines) if b'override MAKEFLAGS = ' in text)
            changed = bytes.fromhex(old['expected']['stdout_hex']).replace(
                b"make: warning: undefined variable 'MISSING_A",
                file.encode() + b':' + str(locus).encode() + b": warning: undefined variable 'MISSING_A")
            self.assertEqual(changed, bytes.fromhex(chosen['expected']['stdout_hex']))
            self.assertTrue(all(old['model_errors'][:2]))
            self.assertEqual((old['oracle_repeat_differences'], old['bounds']), ([], [False] * 3))
            self.assertEqual(old['prototype']['candidate_differences'], [])

    def test_six_original_pilot_inputs_and_models_are_closed_exactly(self):
        original = self.proof['discovery']['records']
        self.assertEqual(len(original), 6)
        for old in original:
            chosen = self.records[old['id']]
            self.assertEqual((old['inputs'], old['args'], old['env'], old['expected'], old['output']),
                             (chosen['inputs'], chosen['args'], chosen['environment'], chosen['expected'], chosen['output']))
            self.assertEqual((old['model_errors'][:2], old['oracle_repeat_differences'], old['bounds']),
                             ([[], []], [], [False] * 3))
            self.assertTrue(old['candidate_differences'])

    def test_rejected_elision_and_initial_copy_proofs_keep_source_and_observations(self):
        for field, count in (('unselected_prototype', 1290), ('initial_copy_proof', 1254)):
            manifest = json.loads((ROOT / self.proof[field]).read_text())
            packed = (ROOT / manifest['raw_observations']).read_bytes()
            raw = gzip.decompress(packed)
            self.assertEqual(hashlib.sha256(packed).hexdigest(), manifest['compressed_sha256'])
            self.assertEqual(hashlib.sha256(raw).hexdigest(), manifest['uncompressed_sha256'])
            self.assertEqual(hashlib.sha256(manifest['source_patch'].encode()).hexdigest(), manifest['source_patch_sha256'])
            reports = json.loads(raw)
            for build in ('debug', 'release'):
                self.assertEqual(len(reports[build]['records']), count)
                for old in reports[build]['records']:
                    chosen = self.records[old['id']]
                    self.assertEqual((old['args'], old['env'], old['expected']),
                                     (chosen['args'], chosen['environment'], chosen['expected']))
                    self.assertEqual((old['model_errors'], old['candidate_differences'], old['bounds']),
                                     ([[], [], []], [], [False] * 3))

    def test_merged_and_separate_anchors_reject_lost_order_status_and_diagnostics(self):
        for row in list(self.records.values())[::43]:
            case = h.load_case(ROOT / 'tests/cases' / row['id'])
            good = h.Observation(bytes.fromhex(row['expected']['stdout_hex']), b'', 0, None, {})
            self.assertEqual(h.expectation_errors(case, good), [])
            self.assertTrue(h.expectation_errors(case, dataclasses.replace(good, returncode=2)))
            self.assertTrue(h.expectation_errors(case, dataclasses.replace(good, stdout=good.stdout.replace(b'FLAGS', b'LATE'))))
            self.assertTrue(h.expectation_errors(case, dataclasses.replace(good, stdout=good.stdout.replace(b'pending', b'cleared'))))
            self.assertTrue(h.expectation_errors(case, dataclasses.replace(good, stderr=b'extra warning\n')))

    def test_actual_jobserver_capacity_explicit_priority_files_and_goals(self):
        rows = [row for name, row in self.records.items() if name.startswith('passive-source-jobs-')]
        self.assertEqual(len(rows), 36)
        self.assertEqual(sum('-p1' in row['id'] for row in rows), 18)
        for row in rows:
            match = re.search(r'-g(\d)-m(\d)-c(\d)-p(\d)$', row['id'])
            self.assertIsNotNone(match)
            _, last, explicit, pipe = map(int, match.groups())
            jobs = explicit or last
            out = bytes.fromhex(row['expected']['stdout_hex'])
            self.assertIn(f'FLAGS=[-j{jobs}] AUTH=[{"yes" if jobs > 1 else "no"}] TOKENS=[{jobs - 1}]\n'.encode(), out)
            self.assertEqual(bytes.fromhex(row['expected_seen']), b'anchor\nGNU\nFLAGS\nDONE\n')
            self.assertIn(b'EXTRA=[kept] STATE=[pending] GNU=[] GOALS=[last]', out)
            self.assertEqual(row['args'][:6], ['-rR', '--no-print-directory', '-f', 'extra.mk', '-f', 'Makefile'])
            self.assertEqual(row['args'][-1], 'last')
            self.assertEqual('--jobserver-style=pipe' in row['args'], bool(pipe))
            script = bytes.fromhex(row['inputs']['probe.py']['hex'])
            self.assertIn(b'os.O_NONBLOCK', script)
            self.assertIn(b'os.write(write_fd, tokens)', script)
            self.assertIn(b'fcntl.F_SETFL, saved)', script)


if __name__ == '__main__':
    unittest.main()
