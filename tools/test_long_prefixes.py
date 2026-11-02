# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect independently qualified prefix metadata, operands and phase witnesses."""
import base64
import json
from pathlib import Path
import tomllib
import unittest
import conformance as h

ROOT=Path(__file__).resolve().parents[1]

class LongPrefixTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.proof=json.loads((ROOT/'tests/research/long-option-prefixes.json').read_text())
        cls.records={r['id']:r for r in cls.proof['records']}

    def test_inventory_inputs_and_each_frozen_comparison_are_qualified(self):
        ledger=tomllib.loads((ROOT/'tests/requirements.toml').read_text())
        family=next(r for r in ledger['requirements']if r['id']=='options.long-prefixes')
        self.assertEqual((family['candidate_required'],family['fixture_count']),(1068,1068))
        self.assertEqual(set(family['cases']),set(self.records))
        self.assertEqual(sum(bool(r['baseline']['candidate_differences'])for r in self.records.values()),921)
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

    def test_original_ambiguity_warning_chatter_and_early_auth_models_keep_disagreements(self):
        rejected=self.proof['discarded_models']['records']
        self.assertEqual(len(rejected),162)
        for r in rejected:
            self.assertTrue(all(r['model_errors'][:2]));self.assertNotIn(r['id'],self.records)
            self.assertEqual((r['oracle_repeat_differences'],r['bounds']),([],[False]*3))
        ambiguous=[r for r in rejected if r['id'].startswith('long-prefix-warn-undefined-variables-1-')]
        self.assertEqual(len(ambiguous),3)
        self.assertEqual({r['observations'][0]['returncode']for r in ambiguous},{0,2})
        warnings=[r for r in rejected if bytes.fromhex(r['observations'][0]['stderr_hex'])==b"make: warning: undefined variable 'GNUMAKEFLAGS'\n"]
        self.assertEqual(len(warnings),69)
        self.assertEqual(sum(r['id'].startswith(('long-value-old-file-','long-value-assume-old-'))for r in rejected),60)

    def test_operand_equal_bytes_select_actual_files_directories_includes_and_eval(self):
        for family,expected_path,expected_bytes in (
            ('file','work/selected',b'left=right\n'),('makefile','work/selected',b'left=right\n'),
            ('directory','work/child=one/selected',b'child\n'),('include-dir','work/selected',b'left=right\n'),
            ('eval','work/selected',b'left=right\n')):
            records=[r for n,r in self.records.items()if n.startswith('long-value-'+family+'-')]
            self.assertTrue(records)
            self.assertTrue(any('equals' in r['id']for r in records))
            self.assertTrue(any('separate' in r['id']for r in records))
            for r in records:
                for b in ('debug','release'):
                    for o in r[b]['observations']:self.assertEqual(base64.b64decode(o['filesystem'][expected_path]['content']),expected_bytes)

    def test_jobserver_auth_is_observed_after_real_publication(self):
        records=[r for n,r in self.records.items()if n.startswith('long-value-jobserver-style-')]
        self.assertEqual(len(records),30)
        for r in records:
            self.assertTrue(r['id'].endswith('-runtime-auth'))
            self.assertEqual(bytes.fromhex(r['expected']['stdout_hex']),b'AUTH=[present] TYPE=[pipe]\n')
            for b in ('debug','release'):
                for o in r[b]['observations']:self.assertEqual(base64.b64decode(o['filesystem']['work/selected']['content']),b'pipe\n')
        before=[r for r in self.proof['discarded_models']['records']if r['id'].startswith('long-value-jobserver-style-')]
        self.assertEqual(len(before),30)
        for r in before:self.assertEqual(bytes.fromhex(r['observations'][0]['stdout_hex']),b'AUTH=[absent] TYPE=[pipe]\n')

    def test_boolean_models_cover_three_origins_and_exact_metadata(self):
        records=[r for n,r in self.records.items()if n.startswith('long-prefix-')]
        self.assertEqual(len(records),552)
        for origin in ('cli','environment','reader'):
            self.assertEqual(sum(n.endswith('-'+origin)or n.endswith('-'+origin+'-bound-flags')for n in self.records if n.startswith('long-prefix-')),184)
        for r in records:
            case=h.load_case(ROOT/'tests/cases'/r['id']);output=bytes.fromhex(r['expected']['stdout_hex'])
            self.assertTrue(output.startswith(b'FLAGS=['));self.assertIn(b'] MFLAGS=[-',output)
            good=h.Observation(output,b'',r['expected']['exit_code'],None,{})
            self.assertEqual(h.expectation_errors(case,good),[])
            bad=h.Observation(output.replace(b'MFLAGS=[-',b'MFLAGS=[',1),b'',r['expected']['exit_code'],None,{})
            self.assertTrue(h.expectation_errors(case,bad))

if __name__=='__main__':unittest.main()
