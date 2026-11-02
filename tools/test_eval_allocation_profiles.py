# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect owned eval observations, profile inputs and measured request counts."""
import dataclasses
import hashlib
import json
from pathlib import Path
import tomllib
import unittest
import allocations
import benchmark
import conformance as h
ROOT=Path(__file__).resolve().parents[1]

class EvalAllocationProfileTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.proof=json.loads((ROOT/'tests/research/eval-allocation-profiles.json').read_text())

    def test_frozen_controls_preserve_inputs_models_bounds_and_source_profiles(self):
        proof=self.proof
        self.assertEqual(len(proof['records']),46)
        self.assertTrue(proof['product']['source_unchanged'])
        self.assertEqual(len(proof['product']['allocation_profiles']),3)
        for report in proof['product']['allocation_profiles']:
            case=h.load_case(ROOT/'tests/cases'/report['workload'])
            self.assertEqual(report['source_sha256'],hashlib.sha256((case.path/'Makefile').read_bytes()).hexdigest())
            self.assertIn(report['binary']['sha256'],proof['product']['builds']['release']['identities'].values())
        for build in proof['product']['builds'].values():
            self.assertEqual(len(build['records']),3)
            for record in build['records']:
                self.assertEqual((record['model_errors'],record['oracle_repeat_differences'],record['candidate_differences'],record['bounds']),([[],[],[]],[],[],[False]*3))
        self.assertEqual(proof['source_evidence_sha256'],hashlib.sha256((ROOT/'tests/research/live-sources.json').read_bytes()).hexdigest())
        ledger=tomllib.loads((ROOT/'tests/requirements.toml').read_text())
        family=next(r for r in ledger['requirements']if r['id']=='performance.eval-allocation-profiles')
        self.assertEqual((family['fixture_count'],family['candidate_required']),(3,3))
        self.assertEqual(set(family['cases']),{n for n,_ in allocations.nested_eval_workloads()})
        for r in proof['records']:
            case=h.load_case(ROOT/'tests/cases'/r['id'])
            self.assertEqual((case.args,case.environment,case.expected),(tuple(r['args']),r['environment'],r['expected']))
            for n,v in r['inputs'].items():self.assertEqual((case.path/n).read_bytes(),bytes.fromhex(v['hex']))
            for b in ('baseline','debug','release'):
                c=r[b];self.assertEqual((c['model_errors'],c['oracle_repeat_differences'],c['candidate_differences'],c['bounds']),([[],[],[]],[],[],[False]*3))
        for b in ('debug','release'):
            report=proof['source_io'][b]
            self.assertEqual((len(report['records']),report['failures']),(36,0))
            self.assertEqual(report['identities'][f'/tmp/rmake-active-final-{b}'],proof['identities'][b][f'/tmp/rmake-active-final-{b}'])
            for r in report['records']:self.assertEqual((r['errors'],r['differences'],r['repeat'],r['bounds']),([[],[],[]],[],[],[False]*3))
        self.assertEqual(set(proof['prototypes']),{'name-filter','owned-keys-only','root-inline-all','indirect-named'})
        for p in proof['prototypes'].values():
            for d in p['builds'].values():self.assertEqual(len(d['records']),43)

    def test_recipe_free_profiles_observe_every_eval_read_and_empty_rule_message(self):
        for name,source in allocations.nested_eval_workloads():
            case=h.load_case(ROOT/'tests/cases'/name)
            self.assertEqual(source,(case.path/'Makefile').read_bytes())
            self.assertTrue(source.endswith(b'.PHONY: all\nall:;\n'))
            self.assertNotIn(b'@:',source)
            self.assertEqual(source.count(b'$(R0)\n'),100)
            self.assertEqual(case.args,tuple(benchmark.workload_arguments(name)))
            good=h.Observation(b'LEAF\n'*100+b"make: Nothing to be done for 'all'.\n",b'',0,None,{})
            self.assertEqual(h.expectation_errors(case,good),[])
            for bad in (good.stdout.replace(b'LEAF',b'WRONG',1),good.stdout[len(b'LEAF\n'):]):
                self.assertTrue(h.expectation_errors(case,dataclasses.replace(good,stdout=bad)))

    def test_repeated_allocation_reports_keep_qualified_sources_and_actual_limits(self):
        for depth in (0,32,96):
            summaries={}
            name=f'profile-nested-evals-{depth}';case=h.load_case(ROOT/'tests/cases'/name)
            for label in ('baseline','candidate'):
                data=[]
                for repeat in (1,2):
                    r=json.loads((ROOT/f'docs/benchmarks/eval-rejected-ownership-alloc-{label}-{depth}-{repeat}.json').read_text())
                    self.assertEqual((r['workload'],r['arguments'],r['environment']),(name,list(case.args),case.environment))
                    self.assertEqual(r['source_sha256'],hashlib.sha256((case.path/'Makefile').read_bytes()).hexdigest())
                    self.assertIn('not live/peak',r['limits'])
                    identity=self.proof['identities']['release' if label=='candidate' else 'baseline_release']
                    self.assertIn(r['binary']['sha256'],identity.values())
                    self.assertGreater(r['summary']['calls'],0)
                    data.append(r['summary'])
                self.assertEqual(data[0],data[1]);summaries[label]=data[0]
            if depth:
                self.assertLess(summaries['candidate']['calls'],summaries['baseline']['calls'])
                self.assertLess(summaries['candidate']['requested_bytes'],summaries['baseline']['requested_bytes'])
            else:self.assertEqual(summaries['candidate']['calls'],summaries['baseline']['calls'])

if __name__=='__main__':unittest.main()
