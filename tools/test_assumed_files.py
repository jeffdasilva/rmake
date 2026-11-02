# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect qualified assumed-file models and independent physical/phase witnesses."""
import dataclasses
import json
from pathlib import Path
import tomllib
import unittest
import conformance as h

ROOT = Path(__file__).resolve().parents[1]

class AssumedFileTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.proof=json.loads((ROOT/'tests/research/assumed-files.json').read_text())
        cls.records={r['id']:r for r in cls.proof['records']}

    def test_inventory_inputs_and_all_frozen_observations_are_qualified(self):
        ledger=tomllib.loads((ROOT/'tests/requirements.toml').read_text())
        family=next(r for r in ledger['requirements']if r['id']=='options.assumed-files')
        self.assertEqual((family['fixture_count'],family['candidate_required']),(631,631))
        self.assertEqual(set(family['cases']),set(self.records))
        self.assertEqual(sum(bool(r['baseline']['candidate_differences'])for r in self.records.values()),560)
        self.assertEqual(self.proof['baseline_mismatches'],560)
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

    def test_rejected_original_models_keep_their_disagreement_without_credit(self):
        discarded=self.proof['discarded_models']
        self.assertIn('zero conformance',discarded['classification'])
        self.assertEqual(len(discarded['records']),41)
        for r in discarded['records']:
            self.assertTrue(all(r['model_errors'][:2]))
            self.assertTrue(r['inputs'])
            self.assertEqual(r['oracle_repeat_differences'],[])
            self.assertEqual(r['bounds'],[False]*3)
            self.assertNotIn(r['id'],self.records)
        paths=[r for r in discarded['records']if '-path-'in r['id']]
        self.assertEqual(len(paths),2)
        for r in paths:self.assertEqual(r['candidate_differences'],['stdout'])

    def test_physical_time_witness_rejects_one_nanosecond_or_missing_observation(self):
        for flag,prefix in (('old',b''),('new',b'ALL=[dep]\n')):
            r=self.records['assumed-physical-time-'+flag]
            case=h.load_case(ROOT/'tests/cases'/r['id'])
            expected=prefix+b'MTIME=1500000300000000000\n'
            self.assertEqual(bytes.fromhex(r['expected']['stdout_hex']),expected)
            good=h.Observation(expected,b'',0,None,{})
            self.assertEqual(h.expectation_errors(case,good),[])
            for output in (expected.replace(b'300000000000',b'300000000001'),prefix):
                self.assertTrue(h.expectation_errors(case,dataclasses.replace(good,stdout=output)))
            for b in ('debug','release'):
                for o in r[b]['observations']:self.assertEqual(bytes.fromhex(o['stdout_hex']),expected)

    def test_activation_and_double_colon_order_have_discriminating_models(self):
        expected={
            'assumed-extra-makeflags-stale-old-dep':b'',
            'assumed-extra-reader-stale-old-dep':b'DEP\nALL=[dep]\n',
            'assumed-extra-cli-binding-stale-old-dep-phase-path':b'DEP\nALL=[dep]\n',
            'assumed-extra-eval-stale-old-dep':b'DEP\nALL=[dep]\n',
            'assumed-extra-makeflags-stale-new-dep-phase-path':b'ALL=[dep]\n',
            'assumed-extra-path-0-new-qualified-phase-path':b'ALL=[inputs/dep]\n',
            'assumed-extra-remake-old':b'G=[old] R=[]\n',
            'assumed-extra-remake-new':b'G=[old] R=[]\n',
            'assumed-extra-remake-base':b'G=[new] R=[1]\n',
        }
        for name,output in expected.items():self.assertEqual(bytes.fromhex(self.records[name]['expected']['stdout_hex']),output)
        record=self.records['assumed-extra-double-repeat-new']
        self.assertEqual(record['expected']['exit_code'],2)
        self.assertEqual(bytes.fromhex(record['expected']['stdout_hex']),b'DEP\n')
        self.assertEqual(bytes.fromhex(record['expected']['stderr_hex']),b"make: *** No rule to make target 'dep'.  Stop.\n")
        queries=[r for n,r in self.records.items()if n.startswith('assumed-dcolon-')and '-new-'in n and '-query'in n]
        self.assertTrue(queries)
        self.assertEqual({r['expected']['exit_code']for r in queries},{1,2})

if __name__=='__main__':unittest.main()
