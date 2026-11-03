# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Guard raw prerequisite models and rejected optimization evidence."""
import base64
import dataclasses
import gzip
import hashlib
import json
from pathlib import Path
import tomllib
import unittest

import benchmark
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


class ImplicitSpellingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.proof = archive('implicit-prerequisite-spelling')
        cls.history = archive('prerequisite-sharing-experiments')
        cls.records = {r['id']: r for r in cls.proof['records']}

    def test_inventory_sources_models_and_every_frozen_observation(self):
        family = next(r for r in tomllib.loads((ROOT / 'tests/requirements.toml').read_text())['requirements']
                      if r['id'] == 'resolution.implicit-prerequisite-spelling')
        self.assertEqual((family['fixture_count'], family['candidate_required']), (144, 144))
        self.assertEqual(set(family['cases']), set(self.records))
        self.assertEqual(self.proof['baseline_mismatches'], 40)
        self.assertEqual(sum(bool(r['baseline']['candidate_differences']) for r in self.records.values()), 40)
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
                for index, observation in enumerate(result['observations']):
                    self.assertFalse(observation['leftover_processes'])
                    if row['expected_seen'] is not None and (index < 2 or build != 'baseline' or not result['candidate_differences']):
                        self.assertEqual(base64.b64decode(observation['filesystem']['work/seen']['content']),
                                         bytes.fromhex(row['expected_seen']))

    def test_literal_percent_directory_raw_byte_and_shared_controls_are_complete(self):
        names = list(self.records)
        matrix = [n for n in names if n.startswith('implicit-spelling-')]
        self.assertEqual(len(matrix), 96)
        for kind in ('stem', 'dot', 'multi', 'literal'):
            self.assertEqual(sum(n.startswith('implicit-spelling-' + kind + '-') for n in matrix), 24)
        self.assertEqual(sum('-parent-ready' in n for n in matrix), 18)
        self.assertEqual(sum('-read-prefix' in n for n in matrix), 2)
        self.assertEqual(sum(n.startswith('shared-prerequisites-') for n in names), 14)
        rejected = [r for n, r in self.records.items() if n.endswith('-literal-percent-rejection')]
        self.assertEqual(len(rejected), 4)
        for row in rejected:
            self.assertEqual(row['expected']['exit_code'], 2)
            self.assertEqual(bytes.fromhex(row['expected_seen']), b'anchor\n')

    def test_original_wrong_models_and_all_rejected_sharing_measurements_remain(self):
        def original(name):
            return json.loads(bytes.fromhex(self.history['files'][name]['hex']))
        for name, count in (('implicit-spelling/baseline.json', 20), ('shared-prerequisites/implicit/baseline.json', 4)):
            records = original(name)['records']
            self.assertEqual(sum(any(r['model_errors'][:2]) for r in records), count)
            self.assertTrue(all(not any(r['bounds']) and not r['oracle_repeat_differences'] for r in records))
        self.assertIn('shared-prerequisites/first-dependencies.rs', self.history['files'])
        self.assertIn('shared-prerequisites/lazy-source.json', self.history['files'])
        for tag in ('a', 'b'):
            report = original(f'shared-prerequisites/first-bench/implicit-rejections-{tag}.json')
            self.assertEqual(len(report['workloads']), 2)
            for workload in report['workloads']:
                values = workload['measurements']
                self.assertTrue(all(len(v['samples_ns']) == 101 for v in values.values()))
                self.assertGreater(values['candidate']['median_ns'], values['baseline']['median_ns'])

    def test_scaling_contracts_and_changed_raw_streams_cannot_pass(self):
        for name, source in benchmark.implicit_rejection_workloads():
            row = self.records[name]
            self.assertEqual(bytes.fromhex(row['inputs']['Makefile']['hex']), source)
            self.assertEqual((row['args'], row['environment']),
                             (benchmark.workload_arguments(name), benchmark.workload_environment(name)))
        for row in list(self.records.values())[::11]:
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
