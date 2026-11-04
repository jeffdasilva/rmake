# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import hashlib
import json
from pathlib import Path
import unittest

import integration_records as records


class IntegrationArtifactTests(unittest.TestCase):
    def test_final_qualification_preserves_sources_identities_and_complete_raw_runs(self):
        root = Path(__file__).resolve().parents[1] / 'tests/integration/qualification'
        raw = (root / 'index.json').read_bytes()
        self.assertEqual(records.digest(raw), 'e24b562319adc0ec8b6645cb9a76eea9993f3bd9a7fc61ca8545a313fe33cb4c')
        index = json.loads(raw)
        self.assertEqual(len(index['records']), 57)
        self.assertEqual(index['profiles'], ['serial-j1'])
        reports = []
        sources = None
        names = {entry['path'] for entry in index['records']}
        for entry in index['records']:
            restored = records.restore(entry, root)
            if entry['path'].endswith('-report.json.gz'):
                value = json.loads(restored)
                self.assertTrue(set(value['retained_parts']) <= names)
                reports.append(value['report'])
            elif entry['path'] == 'source-and-logs.json.gz':
                sources = json.loads(restored)
        self.assertEqual(len(reports), 2)
        for report in reports:
            self.assertTrue(report['passed'])
            self.assertFalse(report['oracle_only'])
            for key, name in (('tool_sha256', 'tools/integration.py'),
                              ('record_tool_sha256', 'tools/integration_records.py'),
                              ('observer_sha256', 'tools/conformance.py')):
                self.assertEqual(report[key], records.digest(sources[name].encode()))
            self.assertEqual(set(report['build_tools']), {'cc', 'as', 'ar', 'ld'})
            self.assertEqual(report['host']['locale'], 'C')
            self.assertEqual(len(report['projects']), 2)
            for project in report['projects']:
                self.assertTrue(project['passed'])
                self.assertEqual(project['differences'], [[[], [], [], []], [[], [], [], []]])
                self.assertEqual(len(project['runs']), 3)
                for run in project['runs']:
                    self.assertEqual(run['errors'], [])
                    self.assertEqual(len(run['parts']), 4)
                    self.assertEqual(run['mtime_witness']['fresh'], run['mtime_witness']['noop'])

    def test_immutable_history_restores_all_original_bytes_and_qualified_runs(self):
        root = Path(__file__).resolve().parents[1] / 'tests/integration/history'
        raw = (root / 'index.json').read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(),
                         '3da1c4dea89e4b25dd58075f25d13bf79f60111662eacbff402b888b643520cf')
        index = json.loads(raw)
        self.assertEqual(index['cases'], ['xxhash-0.8.3', 'zlib-1.3.2'])
        self.assertEqual(index['profiles'], ['serial-j1'])
        self.assertEqual(len(index['records']), 116)
        names = {entry['path'] for entry in index['records']}
        self.assertEqual(len(names), 116)
        sources = None
        reports = []
        for entry in index['records']:
            original = records.restore(entry, root)
            if entry['path'].endswith('-report.json.gz'):
                value = json.loads(original)
                self.assertTrue(set(value['retained_parts']) <= names)
                report = value['report']
                self.assertTrue(report['passed'])
                self.assertFalse(report['oracle_only'])
                self.assertEqual(len(report['projects']), 2)
                for project in report['projects']:
                    self.assertTrue(project['passed'])
                    self.assertEqual(len(project['runs']), 3)
                    self.assertEqual(project['differences'], [[[], [], [], []], [[], [], [], []]])
                    for run in project['runs']:
                        self.assertEqual(run['errors'], [])
                        self.assertEqual(len(run['parts']), 4)
                reports.append(report)
            elif entry['path'] == 'source-and-logs.json.gz':
                sources = json.loads(original)
        self.assertEqual(len(reports), 4)
        self.assertIsNotNone(sources)
        v1_source = sources['tools/integration-v1.py'].encode()
        self.assertEqual(records.digest(v1_source), reports[0]['tool_sha256'])
        self.assertEqual(records.digest(v1_source), reports[1]['tool_sha256'])


if __name__ == '__main__':
    unittest.main()
