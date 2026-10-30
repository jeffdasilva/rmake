# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect independently qualified deep-chain, context and fallback models."""
import base64
import dataclasses
import json
from pathlib import Path
import tomllib
import unittest
import benchmark
import conformance as h
ROOT=Path(__file__).resolve().parents[1]

class LiteralChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.evidence=json.loads((ROOT/'tests/research/literal-chains.json').read_text())
        cls.records={r['id']:r for r in cls.evidence['records']}
    def case(self,name):return h.load_case(ROOT/'tests/cases'/name)
    def out(self,name):return bytes.fromhex(self.case(name).expected['stdout_hex'])

    def test_inventory_and_raw_observations_preserve_exact_original_inputs(self):
        ledger=tomllib.loads((ROOT/'tests/requirements.toml').read_text())
        family=next(r for r in ledger['requirements'] if r['id']=='functions.literal-chains')
        self.assertEqual((family['fixture_count'],family['candidate_required']),(353,353))
        self.assertEqual(len(self.records),353)
        self.assertEqual(sum(bool(r['baseline']['candidate_differences']) for r in self.records.values()),166)
        for r in self.records.values():
            case=self.case(r['id'])
            self.assertEqual((case.args,case.environment,case.expected),(tuple(r['args']),r['environment'],r['expected']))
            for name,content in r['inputs'].items():
                path=case.path/name
                self.assertEqual(path.stat().st_mode&0o777,content['mode'])
                self.assertEqual(path.read_bytes(),bytes.fromhex(content['hex']))
            for build in ('baseline','debug','release'):
                check=r[build]
                self.assertEqual(check['model_errors'][:2],[[],[]])
                self.assertEqual(check['oracle_repeat_differences'],[])
                self.assertEqual(check['bounds'],[False,False,False])
                if build!='baseline':self.assertEqual(check['candidate_differences'],[])
        initial=self.evidence['initial_cross_models']['report']['records']
        self.assertEqual(sum(bool(any(r['model_errors'][:2])) for r in initial),28)

    def test_deep_pure_values_missing_leaves_and_actual_cycles_remain_distinct(self):
        for phase in ('reader','recipe'):
            for mode in ('none','target'):
                for leaf,out in [('simple',b'VALUE=[END]\n'),('literal',b'VALUE=[END]\n'),('missing',b'VALUE=[]\n')]:
                    name=f'alias-depth-512-{leaf}-{phase}-{mode}'
                    self.assertEqual(self.out(name),out)
                    case=self.case(name)
                    self.assertTrue(h.expectation_errors(case,h.Observation(b'',b'',2,None,{})))
                case=self.case(f'alias-depth-512-cycle-{phase}-{mode}')
                self.assertEqual(case.expected['exit_code'],2)
                self.assertEqual(bytes.fromhex(case.expected['stderr_hex']),b"Makefile:1: *** Recursive variable 'R0' references itself (eventually).  Stop.\n")

    def test_scope_warning_and_real_definition_contexts_are_preserved(self):
        for mode in ('none','target'):
            numeric=f'alias-cross-numeric-192-{mode}'
            self.assertEqual(self.out(numeric),b'CALL=[outer]\nMASK=[lex]\nLET=[lex]\nLOOP=[a b]\n')
            for private,child in [(0,'parent'),(1,'global')]:
                self.assertEqual(self.out(f'alias-cross-private-{private}-{mode}'),f'C=[{child}]\nP=[parent]\n'.encode())
            for phase,line in [('reader',194),('recipe',195)]:
                case=self.case(f'alias-cross-paren-192-warning-{phase}-{mode}')
                self.assertEqual(case.environment,{'MAKEFLAGS':'','GNUMAKEFLAGS':''})
                self.assertEqual(bytes.fromhex(case.expected['stderr_hex']),f"Makefile:{line}: warning: undefined variable 'MISSING'\n".encode())
                cycle=self.case(f'alias-cross-cli-cycle-192-{phase}-{mode}')
                self.assertEqual(bytes.fromhex(cycle.expected['stderr_hex']),b"Makefile:192: *** Recursive variable 'R0' references itself (eventually).  Stop.\n")

    def test_complex_and_append_fallbacks_do_not_skip_or_repeat_effects(self):
        for mode in ('none','target'):
            self.assertEqual(self.out(f'alias-cross-append-{mode}'),b'VALUE=[base local]\n')
            record=self.records[f'alias-cross-effect-{mode}']
            self.assertEqual(bytes.fromhex(record['expected']['stdout_hex']),b'VALUE=[changed]\nEFFECT=[1]\n')
            for kind in ('baseline','debug','release'):
                for o in record[kind]['observations']:
                    self.assertEqual(base64.b64decode(o['filesystem']['work/seen']['content']),b'E\n')

    def test_scaling_observes_every_eager_read_and_full_output(self):
        self.assertEqual(list(benchmark.literal_chain_workloads()),list(benchmark.selected_workloads('literal-chains')))
        for (name,source),depth in zip(benchmark.literal_chain_workloads(),(0,32,96)):
            case=self.case(name)
            self.assertEqual((case.path/'Makefile').read_bytes(),source)
            self.assertEqual(case.args,tuple(benchmark.workload_arguments(name)))
            self.assertEqual(source.count(b':= $(L0)'),1000)
            self.assertEqual(source.count(b' = $(L'),depth)
            out=b'READS=['+b'|'.join([b'end']*1000)+b"]\nmake: 'all' is up to date.\n"
            good=h.Observation(out,b'',0,None,{})
            self.assertEqual(h.expectation_errors(case,good),[])
            self.assertTrue(h.expectation_errors(case,dataclasses.replace(good,stdout=out.replace(b'end',b'WRONG',1))))

if __name__=='__main__':unittest.main()
