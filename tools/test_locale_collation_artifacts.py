# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import gzip
import hashlib
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1] / 'tests/locale-collation'


def archive(name):
    manifest = json.loads((ROOT / (name + '.json')).read_bytes())
    packed = (ROOT / (name + '.json.gz')).read_bytes()
    if hashlib.sha256(packed).hexdigest() != manifest['sha256']:
        raise AssertionError('compressed archive identity: ' + name)
    raw = gzip.decompress(packed)
    if hashlib.sha256(raw).hexdigest() != manifest['raw_sha256'] or len(raw) != manifest['raw_bytes']:
        raise AssertionError('complete raw archive identity: ' + name)
    value = json.loads(raw)
    for filename, record in value['files'].items():
        data = bytes.fromhex(record['content_hex'])
        if hashlib.sha256(data).hexdigest() != record['sha256']:
            raise AssertionError('archived input identity: ' + filename)
    return value


class LocaleCollationArtifactTests(unittest.TestCase):
    def test_ranked_selection_retains_both_builds_clocks_and_previous_regression(self):
        value = archive('ranked-qualification')
        for profile in ('debug', 'release'):
            for count in (74, 183):
                name = f'rmake-collation-checked-{profile}{count}.json'
                report = json.loads(bytes.fromhex(value['files'][name]['content_hex']))
                self.assertEqual(report['totals'], {'cases': count, 'pass': count, 'fail': 0})
        self.assertEqual(sum(name.endswith('/report.json') for name in value['files']), 20)
        failed = bytes.fromhex(value['files']['rmake-collation-ranked-build.log']['content_hex'])
        self.assertIn(b'FAILED (failures=1)', failed)
        corrected = bytes.fromhex(value['files']['rmake-collation-ranked-benchmark-units.log']['content_hex'])
        self.assertIn(b'Ran 32 tests', corrected)
        self.assertTrue(corrected.rstrip().endswith(b'OK'))
        for record in value['source']['source'].values():
            self.assertEqual(hashlib.sha256(record['text'].encode()).hexdigest(), record['sha256'])

    def test_performance_screen_retains_all_variants_samples_and_allocator_traces(self):
        value = archive('performance-storage-collation-screen')
        files = value['files']
        self.assertEqual(sum(name.endswith('/report.json') for name in files), 80)
        self.assertEqual(sum(name.endswith('/trace.txt') for name in files), 80)
        reports = {name: json.loads(bytes.fromhex(record['content_hex']))
                   for name, record in files.items()
                   if '/' not in name and name.endswith('.json') and not name.startswith('context')}
        self.assertEqual(len(reports), 13)
        for name, report in reports.items():
            count = 301 if 'runtime-arguments' in name else 31 if 'wildcard-' in name else 101
            self.assertTrue(all(len(row['paired_baseline_over_candidate']) == count
                                for row in report['workloads']))
        # Keep the measured C.UTF8 regression and both rejected layout variants.
        self.assertTrue(any(row['median_paired_baseline_ratio'] < 0.8
                            for row in reports['collation-wildcard-utf8.json']['workloads']))
        self.assertEqual(set(value['source_closures']), {'binding', 'inline', 'combined', 'collation'})
        for record in value['source_closures'].values():
            self.assertEqual(hashlib.sha256(bytes.fromhex(record['content_hex'])).hexdigest(), record['sha256'])

    def test_qualified_full_observations_preceding_failures_and_coherent_sources(self):
        value = archive('qualification')
        files = value['files']
        for profile in ('development', 'optimized'):
            report = json.loads(bytes.fromhex(files['permanent-' + profile + '.json']['content_hex']))
            self.assertEqual(report['totals'], {'cases': 74, 'pass': 74, 'fail': 0})
            self.assertTrue(all(row['passed'] and not any(row['differences']) for row in report['cases']))
            self.assertTrue(all('filesystem' in observation and 'stdout_hex' in observation and 'stderr_hex' in observation
                                for row in report['cases'] for observation in row['observations']))
            old = json.loads(bytes.fromhex(files['original-locales-' + profile + '.json']['content_hex']))
            self.assertEqual(old['totals'], {'cases': 183, 'pass': 183, 'fail': 0})
        preceding = json.loads(bytes.fromhex(files['permanent-preceding.json']['content_hex']))
        self.assertEqual(preceding['totals'], {'cases': 74, 'pass': 41, 'fail': 33})
        self.assertTrue(all(not any(row['errors'][:2]) and not row['differences'][0] for row in preceding['cases']))
        for filename, record in value['source']['files'].items():
            data = bytes.fromhex(record['data_hex'])
            self.assertEqual(hashlib.sha256(data).hexdigest(), record['sha256'], filename)
            self.assertEqual(len(data), record['bytes'], filename)
        self.assertIn('crates/rmake-core/data/default-variables.mk', value['source']['files'])
        self.assertIn('tools/locale_data.py', value['source']['files'])

    def test_wrong_hypotheses_raw_libc_ties_and_observer_errors_remain_distinct(self):
        value = archive('history')
        files = value['files']
        first = json.loads(bytes.fromhex(files['first-observations.json']['content_hex']))
        self.assertEqual(first['totals'], {'cases': 65, 'pass': 59, 'fail': 6})
        raw = json.loads(bytes.fromhex(files['public-strcoll-raw.json']['content_hex']))
        self.assertTrue(any(row['a'] != row['b'] and row['order'] == 0 for row in raw['pairs']))
        v1 = json.loads(bytes.fromhex(files['candidate-v1.json']['content_hex']))
        self.assertEqual(v1['totals'], {'cases': 74, 'pass': 71, 'fail': 3})
        self.assertTrue(all(not any(row['differences']) for row in v1['cases']))
        self.assertIn('refine_models-refined.log', files)
        self.assertIn('parent/collation-probe.log', files)

    def test_authenticated_cold_and_warm_locale_provenance_is_complete(self):
        value = archive('locale-data-history')
        self.assertEqual(value['generated_tree_sha256'], '6bbef6b878271d58f9e4451f9cfb89d22358f006eb829ef2721e83c8f07a68c5')
        self.assertTrue(any(name.endswith('libc-bin.package') for name in value['files']))
        self.assertTrue(any(name.endswith('locales.package') for name in value['files']))


if __name__ == '__main__':
    unittest.main()
