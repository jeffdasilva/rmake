# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import gzip
import hashlib
import json
from pathlib import Path
import unittest


class LocaleArtifactTests(unittest.TestCase):
    def test_performance_keeps_every_sample_trace_repeat_and_negative_result(self):
        root = Path(__file__).resolve().parents[1] / 'tests/locales'
        manifest = json.loads((root / 'performance-ctype.json').read_text())
        packed = (root / 'performance-ctype.json.gz').read_bytes()
        self.assertEqual(hashlib.sha256(packed).hexdigest(), manifest['sha256'])
        raw = gzip.decompress(packed)
        self.assertEqual(hashlib.sha256(raw).hexdigest(), manifest['raw_sha256'])
        self.assertEqual(len(raw), manifest['raw_bytes'])
        value = json.loads(raw)
        self.assertEqual(value['context']['affinity'], [23])
        files = value['files']
        for suite, repeats in (('core', 101), ('startup-arguments', 101), ('runtime-arguments', 301)):
            report = json.loads(files[suite + '.json'])
            for workload in report['workloads']:
                self.assertEqual(len(workload['paired_baseline_over_candidate']), repeats)
        self.assertEqual(sum(name.endswith('/report.json') for name in files), 16)
        self.assertEqual(sum(name.endswith('/trace.txt') for name in files), 16)
        core = json.loads(files['core.json'])
        large = next(row for row in core['workloads'] if row['name'] == 'large-noop')
        self.assertLess(large['median_paired_baseline_ratio'], 1)

    def test_original_failures_models_full_observations_and_source_closure_are_preserved(self):
        root = Path(__file__).resolve().parents[1] / 'tests/locales'
        manifest = json.loads((root / 'history-ctype.json').read_text())
        packed = (root / 'history-ctype.json.gz').read_bytes()
        self.assertEqual(hashlib.sha256(packed).hexdigest(), manifest['sha256'])
        raw = gzip.decompress(packed)
        self.assertEqual(hashlib.sha256(raw).hexdigest(), manifest['raw_sha256'])
        self.assertEqual(len(raw), manifest['raw_bytes'])
        archive = json.loads(raw)
        self.assertEqual(archive['qualified_profiles'], 183)
        baseline = archive['reports']['expanded-preceding.json']
        self.assertEqual(baseline['totals'], {'cases': 183, 'pass': 111, 'fail': 72})
        self.assertTrue(all(not any(row['errors'][:2]) and not row['differences'][0]
                            for row in baseline['cases']))
        for name in ('expanded-gnu.json', 'expanded-development.json', 'expanded-optimized.json'):
            report = archive['reports'][name]
            self.assertEqual(report['totals'], {'cases': 183, 'pass': 183, 'fail': 0})
            for case in report['cases']:
                self.assertTrue(case['passed'])
                self.assertTrue(all('filesystem' in observation and 'stdout_hex' in observation
                                    and 'stderr_hex' in observation for observation in case['observations']))
        sources = archive['source']
        for name, record in sources.items():
            self.assertEqual(hashlib.sha256(record['text'].encode()).hexdigest(), record['sha256'], name)
        self.assertIn('crates/rmake-linux/src/locale.rs', sources)
        self.assertIn('crates/rmake/src/main.rs', sources)
        self.assertIn('crates/rmake-core/data/default-variables.mk', sources)
        self.assertEqual(len(archive['inputs']['models-before.json']['models']), 90)
        self.assertEqual(len(archive['inputs']['models-expanded-reviewed-before.json']['models']), 183)


if __name__ == '__main__':
    unittest.main()
