# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect live invalid-option continuation, byte syntax and ordered effects."""
import base64
import json
from pathlib import Path
import tomllib
import unittest
import conformance as h

ROOT=Path(__file__).resolve().parents[1]

class LiveInvalidOptionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.proof=json.loads((ROOT/'tests/research/live-invalid-options.json').read_text())
        cls.records={r['id']:r for r in cls.proof['records']}

    def test_inventory_and_frozen_raw_observations_are_qualified(self):
        ledger=tomllib.loads((ROOT/'tests/requirements.toml').read_text())
        family=next(r for r in ledger['requirements']if r['id']=='options.live-invalid')
        self.assertEqual((family['candidate_required'],family['fixture_count']),(641,641))
        self.assertEqual(set(family['cases']),set(self.records))
        self.assertEqual(sum(bool(r['baseline']['candidate_differences'])for r in self.records.values()),481)
        for r in self.records.values():
            case=h.load_case(ROOT/'tests/cases'/r['id'])
            self.assertEqual((case.args,case.environment,case.expected),(tuple(r['args']),r['environment'],r['expected']))
            for n,v in r['inputs'].items():
                p=case.path/n
                self.assertEqual((p.stat().st_mode&0o777,p.read_bytes()),(v['mode'],bytes.fromhex(v['hex'])))
            for b in ('baseline','debug','release'):
                c=r[b]
                self.assertEqual((c['model_errors'][:2],c['oracle_repeat_differences'],c['bounds']),([[],[]],[],[False]*3))
                if b!='baseline':self.assertEqual((c['candidate_differences'],c['model_errors']),([],[[],[],[]]))

    def test_the_exact_recorded_reader_w_source_is_closed_without_rewriting_history(self):
        history=json.loads((ROOT/'tests/research/long-option-prefixes.json').read_text())['discarded_models']['records']
        original=next(r for r in history if r['id']=='long-prefix-warn-undefined-variables-1-reader')
        current=self.records['live-invalid-recorded-reader-ambiguous-w']
        self.assertEqual((current['args'],current['environment'],current['inputs']),(original['args'],original['env'],original['inputs']))
        self.assertTrue(all(original['model_errors'][:2]))
        self.assertEqual(bytes.fromhex(current['expected']['stdout_hex']),b'FLAGS=[qrR --no-print-directory] MFLAGS=[-qrR --no-print-directory]\n')
        self.assertTrue(current['baseline']['candidate_differences'])

    def test_all_effectful_and_pure_forms_keep_ordered_file_witnesses(self):
        rows=[r for r in self.records.values()if 'expected_seen'in r]
        self.assertEqual(len(rows),640)
        for r in rows:
            pure=r['id'].endswith('-pure');inactive='-conditional'in r['id']
            expected=b'anchor\nEND:'+ (b'before'if pure or inactive else b'after')+b'\n'
            self.assertEqual(bytes.fromhex(r['expected_seen']),expected)
            self.assertEqual(bool(r['baseline']['candidate_differences']),not inactive)
            for b in ('debug','release'):
                for o in r[b]['observations']:self.assertEqual(base64.b64decode(o['filesystem']['work/seen']['content']),expected)
        for phase in ('reader','eval','include','recipe','secondary'):
            self.assertEqual(sum('-'+phase+'-'in r['id']for r in rows),128)

    def test_byte_options_and_assignment_origins_are_not_normalized(self):
        rows=[r for n,r in self.records.items()if n.startswith('live-invalid-byte-')]
        self.assertEqual(len(rows),80)
        for r in rows:
            self.assertTrue(any(b'\xff'in bytes.fromhex(v['hex'])for v in r['inputs'].values()))
            out=bytes.fromhex(r['expected']['stdout_hex'])
            pure=r['id'].endswith('-pure');inactive='-conditional'in r['id']
            self.assertIn(b'O=[override]'if '-override'in r['id']and not pure else b'O=[file]',out)
            self.assertIn(b'F=[recursive]'if pure or inactive else b'F=[simple]',out)
        for r in rows[:4]:
            case=h.load_case(ROOT/'tests/cases'/r['id']);out=bytes.fromhex(r['expected']['stdout_hex'])
            good=h.Observation(out,b'',0,None,{})
            self.assertEqual(h.expectation_errors(case,good),[])
            bad=h.Observation(out.replace(b'END:',b'LOST:',1),b'',0,None,{})
            self.assertTrue(h.expectation_errors(case,bad))

    def test_rejected_secondary_sources_keep_original_parse_errors_without_credit(self):
        rejected=self.proof['discarded_models']['records']
        self.assertEqual(len(rejected),64)
        for r in rejected:
            self.assertNotIn(r['id'],self.records)
            self.assertTrue(all(r['model_errors'][:2]))
            self.assertEqual(r['bounds'],[False]*3)
            for o in r['observations'][:2]:
                self.assertEqual(o['returncode'],2)
                self.assertIn(b'multiple target patterns',bytes.fromhex(o['stderr_hex']))
        secondary=[r for n,r in self.records.items()if '-secondary-'in n]
        self.assertEqual(len(secondary),128)
        for r in secondary:self.assertIn(b'all: $$(DO)\n',bytes.fromhex(r['inputs']['Makefile']['hex']))

if __name__=='__main__':unittest.main()
