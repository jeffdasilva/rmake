# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Keep original finite history separate from newly qualified required fixtures."""
import json
from pathlib import Path
import unittest
import conformance as h
ROOT=Path(__file__).resolve().parents[1]

class IncludeMetadataResearchTests(unittest.TestCase):
    def test_original_finite_history_survives_qualified_fixture_promotion(self):
        p=json.loads((ROOT/'tests/research/include-metadata-phases.json').read_text())
        self.assertIn('zero conformance or safety-exception credit',p['classification'])
        self.assertEqual(len(p['records']),19)
        self.assertEqual(sum(bool(r['candidate_differences']) for r in p['records']),13)
        for r in p['records']:
            case=h.load_case(ROOT/'tests/cases'/r['id'])
            self.assertEqual((case.candidate,case.requirement),('required','startup.constructor-metadata'))
            self.assertEqual(case.expected,r['expected'])
            self.assertIn('Makefile',r['inputs'])
            self.assertEqual(r['inputs']['Makefile']['type'],'file')
            self.assertEqual(r['oracle_repeat_differences'],[])
            self.assertEqual(r['model_errors'][:2],[[],[]])
            self.assertEqual(r['bounds'],[False,False,False])
            for o in r['observations']:
                self.assertIsNone(o['limit'])
                self.assertFalse(o['leftover_processes'])
        r=next(r for r in p['records'] if r['id']=='include-meta-eval-old-search')
        self.assertEqual([o['returncode'] for o in r['observations']],[0,0,2])
        r=next(r for r in p['records'] if r['id']=='include-meta-make_command')
        self.assertNotEqual(r['original_incomplete_model']['stdout_hex'],r['expected']['stdout_hex'])
        self.assertEqual(bytes.fromhex(r['expected']['stdout_hex']),b'SEEN=[undefined]\n')

if __name__=='__main__':unittest.main()
