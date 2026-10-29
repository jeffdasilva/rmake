# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect independent Linux byte-bound, phase, effect and scaling anchors."""
import base64
import dataclasses
import json
from pathlib import Path
import tomllib
import unittest
import benchmark
import conformance as h

ROOT=Path(__file__).resolve().parents[1]

class AbsolutePathBoundTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.evidence=json.loads((ROOT/'tests/research/abspath-bounds.json').read_text())
        cls.records={r['id']:r for r in cls.evidence['records']}
    def case(self,name):return h.load_case(ROOT/'tests/cases'/name)
    def out(self,name):return bytes.fromhex(self.case(name).expected['stdout_hex'])

    def test_inventory_retains_exact_inputs_and_every_frozen_observation(self):
        ledger=tomllib.loads((ROOT/'tests/requirements.toml').read_text())
        family=next(r for r in ledger['requirements'] if r['id']=='functions.abspath-bounds')
        self.assertEqual((family['fixture_count'],family['candidate_required']),(243,243))
        self.assertEqual(len(self.records),243)
        self.assertEqual(sum(bool(r['baseline']['candidate_differences']) for r in self.records.values()),138)
        for r in self.records.values():
            case=self.case(r['id'])
            self.assertEqual((case.args,case.environment,case.expected),(tuple(r['args']),r['environment'],r['expected']))
            for name,content in r['inputs'].items():
                path=case.path/name
                self.assertEqual(path.stat().st_mode&0o777,content['mode'])
                self.assertEqual(path.read_bytes(),bytes.fromhex(content['hex']))
            for kind in ('baseline','debug','release'):
                check=r[kind]
                self.assertEqual(check['model_errors'][:2],[[],[]])
                self.assertEqual(check['oracle_repeat_differences'],[])
                self.assertEqual(check['bounds'],[False,False,False])
                if kind!='baseline':self.assertEqual(check['candidate_differences'],[])

    def test_limits_count_raw_bytes_and_apply_before_shortening(self):
        for role in ('gnu','make','simple','recursive','reader','recipe'):
            for mode in ('none','target'):
                prefix=f'abspath-bound-{role}-{mode}-'
                self.assertEqual(self.out(prefix+'1'),b'VALUE=[/'+b'a'*4094+b']\n')
                for index in (2,3,5,7,9,12,13,14):self.assertEqual(self.out(prefix+str(index)),b'VALUE=[]\n')
                self.assertEqual(self.out(prefix+'4'),b'VALUE=[/]\n')
                self.assertEqual(self.out(prefix+'6'),b'VALUE=[/b]\n')
                self.assertEqual(self.out(prefix+'8'),b'VALUE=[/'+bytes([0xc3,0xa9])*2047+b']\n')
                self.assertEqual(self.out(prefix+'10'),b'VALUE=[/ok /done]\n')
                self.assertEqual(self.out(prefix+'11'),b'VALUE=[/]\n')
                self.assertEqual(self.out(prefix+'15'),b'VALUE=[]\n' if role in ('gnu','make','simple') else b'VALUE=[child]\n')
                case=self.case(prefix+'5')
                self.assertTrue(h.expectation_errors(case,h.Observation(b'VALUE=[/]\n',b'',0,None,{})))

    def test_discarded_words_still_expand_their_arguments_exactly_once(self):
        selected=[r for r in self.records.values() if r['id'].endswith('-effect')]
        self.assertEqual(len(selected),48)
        for r in selected:
            expected=bytes.fromhex(r['expected']['stdout_hex'])
            self.assertTrue(expected.endswith(b'EFFECT=[1]\n'))
            for kind in ('debug','release'):
                for o in r[kind]['observations']:
                    self.assertEqual(base64.b64decode(o['filesystem']['work/seen']['content']),b'ARG\n')
            case=self.case(r['id']);good=h.Observation(expected,b'',0,None,{})
            self.assertTrue(h.expectation_errors(case,dataclasses.replace(good,stdout=expected.replace(b'EFFECT=[1]',b'EFFECT=[0]'))))

    def test_context_scaling_requires_all_automatic_scope_and_path_checks(self):
        for name,source in benchmark.target_context_workloads():
            case=self.case(name)
            self.assertEqual((case.path/'Makefile').read_bytes(),source)
            self.assertEqual(case.args,tuple(benchmark.workload_arguments(name)))
            self.assertEqual(source.count(b'$(abspath input)'),3000)
            self.assertEqual(source.count(b'missing automatic target'),3000)
            if not name.endswith('-none'):self.assertEqual(source.count(b'missing scoped value'),3000)
            expected=b"make: Nothing to be done for 'all'.\n"
            good=h.Observation(expected,b'',0,None,{})
            self.assertEqual(h.expectation_errors(case,good),[])
            self.assertTrue(h.expectation_errors(case,dataclasses.replace(good,stdout=b'')))

if __name__=='__main__':unittest.main()
