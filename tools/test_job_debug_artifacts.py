# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Keep the complete JOBS evidence archives independently verifiable."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest


class JobDebugArtifactTests(unittest.TestCase):
    def test_all_retained_archives_match_their_manifests(self):
        root = Path(__file__).resolve().parents[1] / 'tests/job-debug'
        for name in ('history', 'qualification', 'performance'):
            with self.subTest(archive=name):
                manifest = json.loads((root / f'{name}.json').read_text())
                packed = (root / f'{name}.json.gz').read_bytes()
                self.assertEqual(hashlib.sha256(packed).hexdigest(), manifest['sha256'])
                raw = gzip.decompress(packed)
                self.assertEqual(hashlib.sha256(raw).hexdigest(), manifest['raw_sha256'])
                self.assertEqual(len(raw), manifest['raw_bytes'])
                self.assertEqual(json.loads(raw)['classification'], manifest['classification'])


if __name__ == '__main__':
    unittest.main()
