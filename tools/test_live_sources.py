# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect original live-source models, refinements and frozen observations."""
import base64
import dataclasses
import json
from pathlib import Path
import tomllib
import unittest
import benchmark
import conformance as h
ROOT = Path(__file__).resolve().parents[1]

class LiveSourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.evidence = json.loads((ROOT/'tests/research/live-sources.json').read_text())
        cls.records = {r['id']: r for r in cls.evidence['records']}

    def test_inventory_inputs_and_frozen_qualification(self):
        ledger=tomllib.loads((ROOT/'tests/requirements.toml').read_text())
        family=next(r for r in ledger['requirements'] if r['id']=='includes.live-sources')
        self.assertEqual((family['fixture_count'],family['candidate_required']),(269,269))
        self.assertEqual(sorted(family['cases']),sorted(self.records))
        self.assertEqual(sum(bool(r['baseline']['candidate_differences'])for r in self.records.values()),158)
        for record in self.records.values():
            case=h.load_case(ROOT/'tests/cases'/record['id'])
            self.assertEqual((case.args,case.environment,case.expected),(tuple(record['args']),record['environment'],record['expected']))
            for name,data in record['inputs'].items():
                path=case.path/name
                self.assertEqual((path.stat().st_mode&0o777,path.read_bytes()),(data['mode'],bytes.fromhex(data['hex'])))
            for build in ('baseline','debug','release'):
                check=record[build]
                self.assertEqual(check['model_errors'][:2],[[],[]])
                self.assertEqual(check['oracle_repeat_differences'],[])
                self.assertEqual(check['bounds'],[False,False,False])
                if build!='baseline':self.assertEqual((check['candidate_differences'],check['model_errors']),([],[[],[],[]]))

    def test_original_streaming_gap_closes_without_losing_no_append_controls(self):
        records=[r for n,r in self.records.items() if n.startswith('source-stream-')]
        self.assertEqual(len(records),32)
        self.assertEqual(sum(bool(r['baseline']['candidate_differences'])for r in records),24)
        for r in records:
            output=bytes.fromhex(r['expected']['stdout_hex'])
            self.assertIn(b'BODY\n',output)
            self.assertIn(b'LATE\n',output)

    def test_include_frames_preserve_effect_order_at_each_depth(self):
        records=[r for n,r in self.records.items() if n.startswith('live-include-')]
        self.assertEqual(len(records),120)
        for r in records:
            depth=int(r['id'].split('-')[2]);effects=r['id'].endswith('-e1')
            expected=b''.join(f'b{i}\n'.encode()for i in range(depth))
            expected+=b''.join(f'a{i}\n'.encode()for i in reversed(range(depth)))
            for build in ('debug','release'):
                for observation in r[build]['observations']:
                    fs=observation['filesystem']
                    if effects:self.assertEqual(base64.b64decode(fs['work/seen']['content']),expected)
                    else:self.assertNotIn('work/seen',fs)
            self.assertEqual(r['expected']['exit_code'],0)

    def test_eval_cycles_rebinding_and_return_effect_order(self):
        records=[r for n,r in self.records.items() if n.startswith('live-eval-')]
        self.assertEqual(len(records),40)
        for r in records:
            _,_,depth,kind,mode=r['id'].split('-');depth=int(depth)
            expected=r['expected'];stderr=bytes.fromhex(expected['stderr_hex'])
            if kind in ('cycle','recursive'):
                self.assertEqual(expected['exit_code'],2)
                line=1 if kind=='cycle' else depth+2
                self.assertIn(f'Makefile:{line}:'.encode(),stderr)
                self.assertIn(b"Recursive variable 'R0'",stderr)
            else:self.assertEqual(expected['exit_code'],0)
            if kind=='effects':
                order=b''.join(f'b{i}\n'.encode()for i in range(depth))
                order+=b''.join(f'a{i}\n'.encode()for i in reversed(range(depth)))
                for build in ('debug','release'):
                    for o in r[build]['observations']:self.assertEqual(base64.b64decode(o['filesystem']['work/seen']['content']),order)

    def test_filename_preparation_refinement_retains_wrong_models(self):
        rejected=self.evidence['discarded_models']['records']
        self.assertEqual(len(rejected),27)
        for r in rejected:self.assertTrue(all(r['model_errors'][:2]))
        selected=[r for n,r in self.records.items() if n.startswith('live-resume-')]
        self.assertEqual(len(selected),24)
        for r in selected:
            output=bytes.fromhex(r['expected']['stdout_hex'])
            self.assertEqual(output,b'BEFORE\nDONE\n' if '-home-' in r['id'] else b'OLD\nDONE\n')
            self.assertTrue(r['id'].endswith('-prepared'))

    def test_source_identity_eof_and_unlocated_directory_errors(self):
        edges=[r for n,r in self.records.items() if n.startswith('live-edge-')]
        self.assertEqual(len(edges),24)
        for r in edges:
            output=bytes.fromhex(r['expected']['stdout_hex']);name=r['id']
            if '-inode-' in name:self.assertEqual(output,b'BEFORE\nTAIL\n')
            if '-no-newline-' in name:self.assertEqual(output,b'BEFORE\n')
            if '-fd-' in name:self.assertEqual(output,b'BEFORE\nFD=[closed]\n')
            if '-truncate-' in name:self.assertEqual(output,b'BEFORE\n'+(b'' if '-16384-' in name else b'TAIL\n'))
        dirs=[r for n,r in self.records.items() if n.startswith('live-dir-')]
        self.assertEqual(len(dirs),14)
        for r in dirs:
            self.assertEqual(bytes.fromhex(r['expected']['stderr_hex']),b'make: *** dir: Is a directory.  Stop.\n')
            self.assertIn('dir/.keep',r['inputs'])

    def test_scaling_observes_every_valid_preceding_eval_read(self):
        self.assertEqual(list(benchmark.nested_eval_workloads()),list(benchmark.selected_workloads('nested-evals')))
        for name,source in benchmark.nested_eval_workloads():
            case=h.load_case(ROOT/'tests/cases'/name)
            self.assertEqual((case.path/'Makefile').read_bytes(),source)
            self.assertEqual(case.args,tuple(benchmark.workload_arguments(name)))
            self.assertEqual(source.count(b'$(R0)\n'),100)
            good=h.Observation(b'LEAF\n'*100,b'',0,None,{})
            self.assertEqual(h.expectation_errors(case,good),[])
            for output in (b'LEAF\n'*99,b'WRONG\n'+b'LEAF\n'*99):
                self.assertTrue(h.expectation_errors(case,dataclasses.replace(good,stdout=output)))

if __name__=='__main__':unittest.main()
