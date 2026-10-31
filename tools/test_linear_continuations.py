# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect independently modeled linear continuations and opaque leaf behavior."""
import base64
import dataclasses
import json
from pathlib import Path
import tomllib
import unittest
import benchmark
import conformance as h
ROOT=Path(__file__).resolve().parents[1]

class LinearContinuationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.evidence=json.loads((ROOT/'tests/research/linear-continuations.json').read_text())
        cls.records={r['id']:r for r in cls.evidence['records']}
    def case(self,name):return h.load_case(ROOT/'tests/cases'/name)
    def out(self,name):return bytes.fromhex(self.case(name).expected['stdout_hex'])
    def observations(self,r):
        for build in ('baseline','debug','release'):
            yield from r[build]['observations'][:2 if build=='baseline' else 3]

    def test_inventory_preserves_original_inputs_and_qualified_observations(self):
        ledger=tomllib.loads((ROOT/'tests/requirements.toml').read_text())
        family=next(r for r in ledger['requirements'] if r['id']=='functions.linear-continuations')
        self.assertEqual((family['fixture_count'],family['candidate_required']),(419,419))
        self.assertEqual(sorted(family['cases']),sorted(self.records))
        self.assertEqual(sum(bool(r['baseline']['candidate_differences']) for r in self.records.values()),248)
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
                if build!='baseline':self.assertEqual((check['candidate_differences'],check['model_errors']),([],[[],[],[]]))

    def test_all_original_finite_effect_boundary_failures_are_closed(self):
        original=[r for r in self.records.values() if r['id'].startswith('text-effects-')]
        self.assertEqual(len(original),80)
        self.assertEqual(sum(bool(r['baseline']['candidate_differences']) for r in original),40)
        for leaf in ('strip','computed','call','file','eval'):
            case=self.case(f'text-effects-{leaf}-192-recipe-target')
            self.assertEqual(case.expected['exit_code'],0)
            self.assertTrue(h.expectation_errors(case,h.Observation(b'',b'',2,None,{})))

    def test_leaf_writes_occur_once_even_before_cycle_errors_and_nul_returns(self):
        for r in self.records.values():
            source=bytes.fromhex(r['inputs']['Makefile']['hex'])
            if b'$(file >>seen,E)' in source:
                for o in self.observations(r):
                    self.assertEqual(base64.b64decode(o['filesystem']['work/seen']['content']),b'E\n')
            if r['id'].startswith('linear-nul-effect-'):
                self.assertEqual(bytes.fromhex(r['inputs']['payload']['hex']),b'raw\0discard')
                self.assertNotIn(b'discard',bytes.fromhex(r['expected']['stdout_hex']))

    def test_numeric_target_metadata_and_rebinding_controls_remain_distinct(self):
        for mode in ('none','target'):
            wrap=lambda v:b'['*192+v+b']'*192
            self.assertEqual(self.out(f'linear-numeric-192-{mode}'),b'CALL=['+wrap(b'outer')+b']\nMASK=['+wrap(b'lex')+b']\nLET=['+wrap(b'lex')+b']\n')
            self.assertEqual(self.out(f'linear-append-192-{mode}'),b'VALUE=['+wrap(b'base local')+b']\n')
            for phase in ('reader','recipe'):
                for kind,value,status in [('ordinary',b'end',b'0'),('failure',b'end',b'7'),('export',b'original',b'0')]:
                    name=f'linear-shell-{kind}-192-{phase}-{mode}'
                    self.assertEqual(self.out(name),b'VALUE=['+wrap(value)+b']\nSTATUS=['+status+b']\n')
                    if kind=='export':self.assertEqual(self.case(name).environment,{'R0':'original'})

            for private,child in [(0,b'parent'),(1,b'global')]:
                r=self.records[f'linear-private-{private}-192-{mode}']
                self.assertEqual(bytes.fromhex(r['expected']['stdout_hex']),b'C=['+wrap(child)+b']\nP=['+wrap(b'parent')+b']\n')
                for o in self.observations(r):self.assertEqual(base64.b64decode(o['filesystem']['work/seen']['content']),child+b'\nparent\n')
            for phase in ('reader','recipe'):
                self.assertEqual(self.out(f'linear-active-simple-192-{phase}-{mode}'),b'VALUE=['+wrap(b'new')+b']\nAGAIN=[new]\n')
                self.assertEqual(self.out(f'linear-resume-rebinding-192-{phase}-{mode}'),b'VALUE=['+wrap(b'end')+b']\nAGAIN=[[new]]\n')
                self.assertEqual(self.out(f'linear-metadata-effect-192-{phase}-{mode}'),b'VALUE=['+wrap(b'R0')+b']\nEFFECT=[1]\n')

    def test_warning_order_and_actual_cli_cycle_locations_are_exact(self):
        for mode in ('none','target'):
            for phase,line in [('reader',194),('recipe',195)]:
                case=self.case(f'linear-warning-192-{phase}-{mode}')
                self.assertEqual(case.environment,{'MAKEFLAGS':'','GNUMAKEFLAGS':''})
                expected=f"Makefile:{line}: before\nMakefile:{line}: warning: undefined variable 'MISSING'\nMakefile:{line}: after\n".encode()
                self.assertEqual(bytes.fromhex(case.expected['stderr_hex']),expected)
                cycle=self.case(f'linear-cli-cycle-192-{phase}-{mode}')
                self.assertEqual(bytes.fromhex(cycle.expected['stderr_hex']),b"Makefile:192: *** Recursive variable 'R0' references itself (eventually).  Stop.\n")
                bad=h.Observation(bytes.fromhex(case.expected['stdout_hex']),b'',0,None,{})
                self.assertTrue(h.expectation_errors(case,bad))

    def test_escaped_dollars_remain_literal_across_all_suspended_frames(self):
        value=b'changed'
        for _ in range(256):value=b'pre$<'+value+b'>$post$'
        for phase in ('reader','recipe'):
            for mode in ('none','target'):
                self.assertEqual(self.out(f'linear-dollars-eval-256-{phase}-{mode}'),b'VALUE=['+value+b']\n')

    def test_scaling_checks_every_function_leaf_read_and_value(self):
        self.assertEqual(list(benchmark.linear_leaf_workloads()),list(benchmark.selected_workloads('linear-leaves')))
        for (name,source),depth in zip(benchmark.linear_leaf_workloads(),(0,32,96)):
            case=self.case(name)
            self.assertEqual((case.path/'Makefile').read_bytes(),source)
            self.assertEqual(case.args,tuple(benchmark.workload_arguments(name)))
            self.assertEqual(source.count(b':= $(T0)'),100)
            value=b'['*depth+b'end'+b']'*depth
            out=b'READS=['+b'|'.join([value]*100)+b"]\nmake: 'all' is up to date.\n"
            good=h.Observation(out,b'',0,None,{})
            self.assertEqual(h.expectation_errors(case,good),[])
            self.assertTrue(h.expectation_errors(case,dataclasses.replace(good,stdout=out.replace(b'end',b'WRONG',1))))

if __name__=='__main__':unittest.main()
