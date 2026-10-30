# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect original text-frame models, diagnostics, effects and benchmark outputs."""
import base64
import dataclasses
import json
from pathlib import Path
import tomllib
import unittest
import benchmark
import conformance as h
ROOT = Path(__file__).resolve().parents[1]

class TextFrameTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.evidence = json.loads((ROOT/'tests/research/text-frames.json').read_text())
        cls.records = {r['id']: r for r in cls.evidence['records']}

    def case(self, name):
        return h.load_case(ROOT/'tests/cases'/name)

    def out(self, name):
        return bytes.fromhex(self.case(name).expected['stdout_hex'])

    def test_inventory_and_original_observations_preserve_exact_inputs(self):
        ledger = tomllib.loads((ROOT/'tests/requirements.toml').read_text())
        family = next(r for r in ledger['requirements'] if r['id']=='functions.text-frames')
        self.assertEqual((family['fixture_count'], family['candidate_required']), (479,479))
        self.assertEqual(sorted(family['cases']), sorted(self.records))
        self.assertEqual(sum(bool(r['baseline']['candidate_differences']) for r in self.records.values()),182)
        for r in self.records.values():
            case = self.case(r['id'])
            self.assertEqual((case.args,case.environment,case.expected), (tuple(r['args']),r['environment'],r['expected']))
            for name, content in r['inputs'].items():
                path = case.path/name
                self.assertEqual(path.stat().st_mode & 0o777,content['mode'])
                self.assertEqual(path.read_bytes(),bytes.fromhex(content['hex']))
            for build in ('baseline','debug','release'):
                check = r[build]
                self.assertEqual(check['model_errors'][:2],[[],[]])
                self.assertEqual(check['oracle_repeat_differences'],[])
                self.assertEqual(check['bounds'],[False,False,False])
                if build!='baseline':
                    self.assertEqual(check['candidate_differences'],[])
                    self.assertEqual(check['model_errors'],[[],[],[]])

    def test_finite_depth_and_real_cycles_have_separate_models(self):
        for phase in ('reader','recipe'):
            for mode in ('none','target'):
                name=f'text-frame-bracket-256-simple-{phase}-{mode}'
                self.assertEqual(self.out(name), b'VALUE=['+b'['*256+b'END'+b']'*256+b']\n')
                cycle=self.case(f'text-frame-bracket-256-cycle-{phase}-{mode}')
                self.assertEqual((cycle.expected['exit_code'],bytes.fromhex(cycle.expected['stderr_hex'])),(2,b"Makefile:1: *** Recursive variable 'R0' references itself (eventually).  Stop.\n"))
                self.assertTrue(h.expectation_errors(self.case(name),h.Observation(b'',b'',2,None,{})))

    def test_mixed_and_short_reference_spellings_keep_literal_text(self):
        for phase in ('reader','recipe'):
            for mode in ('none','target'):
                self.assertEqual(self.out(f'text-reference-mixed-192-simple-{phase}-{mode}'),b'VALUE=['+b'<'*192+b'END'+b'>'*192+b']\n')
                self.assertEqual(self.out(f'text-reference-short-simple-{phase}-{mode}'),b'VALUE=['+b'['*25+b'END'+b']'*25+b']\n')

    def test_shared_graphs_repeat_warnings_with_exact_callers(self):
        for phase,line in [('reader',10),('recipe',11)]:
            for mode in ('none','target'):
                case=self.case(f'text-cross-dag-warning-8-{phase}-{mode}')
                self.assertEqual(case.environment,{'MAKEFLAGS':'','GNUMAKEFLAGS':''})
                self.assertEqual(bytes.fromhex(case.expected['stderr_hex']),f"Makefile:{line}: warning: undefined variable 'MISSING'\n".encode()*256)
                bad=h.Observation(bytes.fromhex(case.expected['stdout_hex']),b'',0,None,{})
                self.assertTrue(h.expectation_errors(case,bad))

    def test_scope_numeric_and_cli_cycle_sources_are_preserved(self):
        for mode in ('none','target'):
            wrap=lambda v:b'['*192+v+b']'*192
            self.assertEqual(self.out(f'text-cross-numeric-192-{mode}'), b'CALL=['+wrap(b'outer')+b']\nMASK=['+wrap(b'lex')+b']\nLET=['+wrap(b'lex')+b']\nLOOP=['+wrap(b'a')+b' '+wrap(b'b')+b']\n')
            self.assertEqual(self.out(f'text-cross-private-1-{mode}'),b'C=[[<global>]]\nP=[[<parent>]]\n')
            self.assertEqual(self.out(f'text-cross-append-{mode}'),b'VALUE=[base [<local>]]\n')
            for phase in ('reader','recipe'):
                case=self.case(f'text-cross-cli-cycle-192-{phase}-{mode}')
                self.assertEqual(bytes.fromhex(case.expected['stderr_hex']),b"Makefile:192: *** Recursive variable 'R0' references itself (eventually).  Stop.\n")

    def test_rejected_effectful_plans_never_duplicate_file_effects(self):
        for phase in ('reader','recipe'):
            for mode in ('none','target'):
                r=self.records[f'text-cross-effect-{phase}-{mode}']
                self.assertEqual(bytes.fromhex(r['expected']['stdout_hex']),b'VALUE=[[<changed>]]\nEFFECT=[1]\n')
                self.assertEqual(bytes.fromhex(r['expected']['stderr_hex']).count(b'warning:'),2)
                for build in ('baseline','debug','release'):
                    for o in r[build]['observations']:
                        self.assertEqual(base64.b64decode(o['filesystem']['work/seen']['content']),b'E\n')

    def test_scaling_sources_and_every_eager_read_are_observed(self):
        self.assertEqual(list(benchmark.text_frame_workloads()),list(benchmark.selected_workloads('text-frames')))
        for (name,source),depth in zip(benchmark.text_frame_workloads(),(0,32,96)):
            case=self.case(name)
            self.assertEqual((case.path/'Makefile').read_bytes(),source)
            self.assertEqual(case.args,tuple(benchmark.workload_arguments(name)))
            self.assertEqual(source.count(b':= $(T0)'),1000)
            value=b'['*depth+b'end'+b']'*depth
            out=b'READS=['+b'|'.join([value]*1000)+b"]\nmake: 'all' is up to date.\n"
            good=h.Observation(out,b'',0,None,{})
            self.assertEqual(h.expectation_errors(case,good),[])
            self.assertTrue(h.expectation_errors(case,dataclasses.replace(good,stdout=out.replace(b'end',b'WRONG',1))))

if __name__=='__main__':
    unittest.main()
