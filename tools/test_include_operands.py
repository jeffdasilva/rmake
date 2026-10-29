# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Guard original byte-history, publication, file-effect and scaling models."""
import base64
import dataclasses
import json
from pathlib import Path
import tomllib
import unittest
import conformance as h
import benchmark

ROOT=Path(__file__).resolve().parents[1]

class IncludeOperandTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.evidence=json.loads((ROOT/'tests/research/include-operands.json').read_text())
        cls.records={r['id']:r for r in cls.evidence['records']}
    def case(self,name):return h.load_case(ROOT/'tests/cases'/name)
    def out(self,name):return bytes.fromhex(self.case(name).expected['stdout_hex'])

    def test_inventory_preserves_inputs_and_frozen_model_qualification(self):
        ledger=tomllib.loads((ROOT/'tests/requirements.toml').read_text())
        family=next(r for r in ledger['requirements'] if r['id']=='includes.operand-history')
        self.assertEqual((family['fixture_count'],family['candidate_required']),(941,941))
        self.assertEqual(len(self.records),941)
        self.assertEqual(sum(bool(r['baseline']['candidate_differences']) for r in self.records.values()),706)
        for r in self.records.values():
            case=self.case(r['id'])
            self.assertEqual((case.args,case.environment,case.expected),(tuple(r['args']),r['environment'],r['expected']))
            for name,content in r['inputs'].items():
                path=case.path/name
                self.assertEqual(path.stat().st_mode&0o777,content['mode'])
                if content['type']=='file':self.assertEqual(path.read_bytes(),bytes.fromhex(content['hex']))
                else:self.assertTrue(any(p.is_file() for p in path.rglob('*')))
            for build in ('baseline','debug','release'):
                check=r[build]
                self.assertEqual(check['model_errors'][:2],[[],[]])
                self.assertEqual(check['oracle_repeat_differences'],[])
                self.assertEqual(check['bounds'],[False,False,False])
                if build!='baseline':self.assertEqual(check['candidate_differences'],[])

    def test_raw_comparison_precedes_normalization_and_barriers_do_not_clear_history(self):
        for mode in ('none','target'):
            self.assertEqual(self.out(f'include-operands-initial-{mode}-1'),b'BEFORE=[2] FLAGS=[2]\n')
            self.assertEqual(self.out(f'include-operands-initial-{mode}-5'),b'BEFORE=[1] FLAGS=[1]\n')
            self.assertEqual(self.out(f'include-operands-barrier-{mode}-0'),b'BEFORE=[0] FLAGS=[1]\n')
            self.assertEqual(self.out(f'include-operands-barrier-{mode}-1'),b'BEFORE=[1] FLAGS=[2]\n')
            case=self.case(f'include-operands-initial-{mode}-1')
            self.assertTrue(h.expectation_errors(case,h.Observation(b'BEFORE=[1] FLAGS=[1]\n',b'',0,None,{})))
            for phase in ('reader','recipe'):
                self.assertEqual(self.out(f'include-operands-{phase}-{mode}-1-1'),b'BEFORE=[2] FLAGS=[2]\nAFTER=[3] FLAGS=[3]\n')
            self.assertEqual(self.out(f'include-operands-passive-{mode}-1-1'),b'BEFORE=[2] FLAGS=[2]\nPASSIVE=[2]\nAFTER=[3] FLAGS=[3]\n')

    def test_retained_operands_repeat_effects_and_rereads_without_caching(self):
        for mode in ('none','target'):
            for phase in ('initial','reader','recipe','passive'):
                record=self.records[f'include-operands-effects-{mode}-1-{phase}']
                wanted=b'T\n'*(2 if phase=='initial' else 4)
                for kind in ('debug','release'):
                    for o in record[kind]['observations']:
                        self.assertEqual(base64.b64decode(o['filesystem']['work/seen']['content']),wanted)
            self.assertEqual(self.out(f'include-operands-initial-home-{mode}-0'),b'HOME\nHOME\nBEFORE=[2]\nAFTER=[2]\n')
            record=self.records[f'include-operands-restart-{mode}-1']
            self.assertEqual(bytes.fromhex(record['expected']['stdout_hex']),b'PASS=[] N=[2]\nPASS=[1] N=[2]\n')
            for kind in ('debug','release'):
                for o in record[kind]['observations']:
                    self.assertEqual(base64.b64decode(o['filesystem']['work/generated.mk']['content']),b'# generated\n')

    def test_fault_gate_requires_every_retained_metadata_observation(self):
        faults=self.evidence['metadata_faults']
        self.assertEqual(sum(bool(r['errors']) for r in faults['baseline']['records']),11)
        for kind in ('debug','release'):
            self.assertEqual(len(faults[kind]['records']),72)
            for r in faults[kind]['records']:self.assertEqual(r['errors'],[])
        for report in self.evidence['initial_scaling_model']['reports'].values():
            self.assertEqual(len(report['records']),3)
            for r in report['records']:
                self.assertTrue(all(r['model_errors'][:2]))
                self.assertEqual(r['candidate_differences'],[])
                self.assertEqual(r['bounds'],[False,False,False])

    def test_scaling_workloads_observe_exact_ordered_operands_and_no_search_entries(self):
        workloads=list(benchmark.include_operand_workloads())
        self.assertEqual(workloads,list(benchmark.selected_workloads('include-operands')))
        for (name,source),count in zip(workloads,(0,1000,5000)):
            case=self.case(name)
            self.assertEqual((case.path/'Makefile').read_bytes(),source)
            self.assertEqual(case.args,tuple(benchmark.workload_arguments(name)))
            self.assertEqual(case.args[2:],('-I-',*(f'-Imissing-{i:05d}' for i in range(count))))
            flags=b' '.join(a.encode() for a in case.args[2:])
            out=b'DIRS=[]\nFLAGS=['+flags+b"]\nmake: Nothing to be done for 'all'.\n"
            good=h.Observation(out,b'',0,None,{})
            self.assertEqual(h.expectation_errors(case,good),[])
            self.assertTrue(h.expectation_errors(case,dataclasses.replace(good,stdout=out.replace(b'DIRS=[]',b'DIRS=[bad]'))))
            if count:self.assertTrue(h.expectation_errors(case,dataclasses.replace(good,stdout=out.replace(b'-Imissing-00500',b'-Imissing-00501'))))

if __name__=='__main__':unittest.main()
