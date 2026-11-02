# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Keep bundled dependency notices tied to the locked registry graph."""
import hashlib
import json
from pathlib import Path
import tomllib
import unittest
ROOT=Path(__file__).resolve().parents[1]

class DependencyLicenseTests(unittest.TestCase):
    def test_every_locked_registry_package_has_original_notices_and_checksum(self):
        locked=tomllib.loads((ROOT/'Cargo.lock').read_text())['package']
        packages={(p['name'],p['version']):p for p in locked if p.get('source','').startswith('registry+')}
        manifest=json.loads((ROOT/'licenses/third-party/manifest.json').read_text())
        self.assertEqual(set(packages),{(p['name'],p['version'])for p in manifest})
        self.assertEqual(len(manifest),12)
        notices=(ROOT/'THIRD_PARTY_NOTICES.md').read_text()
        for p in manifest:
            key=(p['name'],p['version'])
            self.assertEqual(packages[key]['checksum'],p['package_checksum'])
            self.assertTrue(p['license']);self.assertTrue(p['files'])
            directory=ROOT/'licenses/third-party'/('-'.join(key))
            for file,digest in p['files'].items():
                path=directory/file
                self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),digest)
                self.assertIn(str(path.relative_to(ROOT)),notices)

if __name__=='__main__':unittest.main()
