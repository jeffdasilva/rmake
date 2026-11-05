# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import gzip
import hashlib
import json
from pathlib import Path
import unittest


class GuileComponentArtifactTests(unittest.TestCase):
    def test_complete_component_proof_and_source_hashes_are_retained(self):
        root = Path(__file__).resolve().parents[1] / 'tests/guile-component'
        manifest = json.loads((root / 'qualification.json').read_text())
        packed = (root / 'qualification.json.gz').read_bytes()
        self.assertEqual(hashlib.sha256(packed).hexdigest(), manifest['sha256'])
        raw = gzip.decompress(packed)
        self.assertEqual(hashlib.sha256(raw).hexdigest(), manifest['raw_sha256'])
        self.assertEqual(len(raw), manifest['raw_bytes'])
        value = json.loads(raw)
        self.assertEqual(value['component_cases'], 20)
        self.assertEqual(value['native_fenv_cases'], 18)
        for name, record in value['source'].items():
            self.assertEqual(hashlib.sha256(record['text'].encode()).hexdigest(), record['sha256'], name)
        log = value['files']['/tmp/rmake-guile-portable-component20-fenv-corrected.log']
        self.assertEqual(sum(line.startswith('PASS ') for line in log.splitlines()), 20)
        self.assertIn('/tmp/rmake-guile-portable-component18.log', value['files'])
        self.assertIn('/tmp/rmake-guile-portable-component20-fenv.log', value['files'])
        for profile in ('development', 'optimized'):
            report = json.loads(value['files'][f'artifacts/native-fenv/{profile}-v1.json'])
            self.assertTrue(report['passed'])
            self.assertEqual(report['totals'], {'cases': 18, 'pass': 18, 'fail': 0})

    def test_exploratory_cost_keeps_all_samples_and_regressions(self):
        root = Path(__file__).resolve().parents[1] / 'tests/guile-component'
        manifest = json.loads((root / 'performance-exploratory.json').read_text())
        packed = (root / 'performance-exploratory.json.gz').read_bytes()
        self.assertEqual(hashlib.sha256(packed).hexdigest(), manifest['sha256'])
        raw = gzip.decompress(packed)
        self.assertEqual(hashlib.sha256(raw).hexdigest(), manifest['raw_sha256'])
        value = json.loads(raw)
        self.assertEqual(value['context']['affinity'], [23])
        files = value['files']
        self.assertEqual(sum(name.endswith('/report.json') for name in files), 16)
        self.assertEqual(sum(name.endswith('/trace.txt') for name in files), 16)
        for suite, count in (('core', 101), ('startup-arguments', 101), ('runtime-arguments', 301)):
            report = json.loads(files['component-' + suite + '.json'])
            self.assertTrue(all(len(row['paired_baseline_over_candidate']) == count for row in report['workloads']))
        core = json.loads(files['component-core.json'])
        self.assertTrue(all(row['median_paired_baseline_ratio'] < 1 for row in core['workloads']))
        for name, record in value['tools'].items():
            self.assertEqual(hashlib.sha256(record['text'].encode()).hexdigest(), record['sha256'], name)

    def test_selected_cost_and_source_after_locale_remain_independent(self):
        root = Path(__file__).resolve().parents[1] / 'tests/guile-component'
        manifest = json.loads((root / 'performance-selected.json').read_text())
        packed = (root / 'performance-selected.json.gz').read_bytes()
        self.assertEqual(hashlib.sha256(packed).hexdigest(), manifest['sha256'])
        raw = gzip.decompress(packed)
        self.assertEqual(hashlib.sha256(raw).hexdigest(), manifest['raw_sha256'])
        value = json.loads(raw)
        for name, record in value['candidate_source']['source'].items():
            self.assertEqual(hashlib.sha256(record['text'].encode()).hexdigest(), record['sha256'], name)
        files = value['files']
        for suite, count in (('core', 101), ('startup-arguments', 101), ('runtime-arguments', 301)):
            report = json.loads(files['component-' + suite + '.json'])
            self.assertEqual(report['identities']['candidate']['sha256'], value['candidate_source']['binary']['sha256'])
            self.assertTrue(all(len(row['paired_baseline_over_candidate']) == count for row in report['workloads']))
        for work in ('tiny-noop', 'large-noop', 'startup-arguments-1000', 'runtime-arguments-512-updates-100'):
            reports = [json.loads(files[f'component-{side}-{work}-{repeat}/report.json'])['summary']
                       for side in ('preceding', 'candidate') for repeat in (1, 2)]
            self.assertTrue(all(row == reports[0] for row in reports))
        self.assertIn('20 isolated cases passed', value['functional_after_locale']['/tmp/rmake-component-locale-component20.log'])

    def test_all_native_fenv_discovery_and_leading_archives_are_intact(self):
        root = Path(__file__).resolve().parents[1] / 'tests/native'
        for name in ('fenv-history', 'fenv-leading-history'):
            manifest = json.loads((root / (name + '.json')).read_text())
            packed = (root / (name + '.json.gz')).read_bytes()
            self.assertEqual(hashlib.sha256(packed).hexdigest(), manifest['sha256'])
            raw = gzip.decompress(packed)
            self.assertEqual(hashlib.sha256(raw).hexdigest(), manifest['raw_sha256'])
            self.assertEqual(len(raw), manifest['raw_bytes'])


if __name__ == '__main__': unittest.main()
