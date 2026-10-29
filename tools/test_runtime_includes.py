# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Keep owned include publication and live RHS phase models independently anchored."""
import dataclasses
from pathlib import Path
import tomllib
import unittest
import conformance as h
ROOT=Path(__file__).resolve().parents[1]

class RuntimeIncludeTests(unittest.TestCase):
    def case(self,name):return h.load_case(ROOT/'tests/cases'/name)
    def out(self,name):return bytes.fromhex(self.case(name).expected['stdout_hex'])

    def test_required_inventory_keeps_both_startup_paths_all_effect_sites_and_modes(self):
        ledger=tomllib.loads((ROOT/'tests/requirements.toml').read_text())
        includes=next(r for r in ledger['requirements'] if r['id']=='includes.runtime-metadata')
        rhs=next(r for r in ledger['requirements'] if r['id']=='variables.rhs-effects')
        self.assertEqual((includes['fixture_count'],rhs['fixture_count']),(143,12))
        import json
        evidence=json.loads((ROOT/'tests/research/runtime-include-phases.json').read_text())
        self.assertIn('no separate conformance or safety-exception credit',evidence['classification'])
        self.assertEqual(len(evidence['records']),155)
        self.assertEqual(sum(bool(r['baseline']['candidate_differences']) for r in evidence['records']),102)
        for r in evidence['records']:
            self.assertEqual(self.case(r['id']).expected,r['expected'])
            for build in ('baseline','debug','release'):
                observation=r[build]
                self.assertEqual(observation['oracle_repeat_differences'],[])
                self.assertEqual(observation['model_errors'][:2],[[],[]])
                self.assertEqual(observation['bounds'],[False,False,False])
                if build!='baseline':self.assertEqual(observation['candidate_differences'],[])

        for site in ('reader','eval','recipe'):
            for mode in ('serial','pipe','sync'):
                for route in ('direct','prepared'):
                    case=self.case(f'include-runtime-{site}-{mode}-{route}-old-search')
                    self.assertEqual(case.candidate,'required')
                    self.assertEqual(self.out(case.id),b'META=[old]\nAFTER=[new]\n')
                    self.assertEqual(any('DUMMY:=' in arg for arg in case.args),route=='prepared')

    def test_nested_publication_retains_outer_search_current_inner_flags_and_live_append(self):
        name='include-runtime-reader-serial-direct-nested-live-value'
        expected=b'SEEN=[old]/[inner] VALUE=[inner]\nAFTER=[]\nFLAG=[-Iinner]\nDIR=[a- b- inner a- b-]\n'
        self.assertEqual(self.out(name),expected)
        case=self.case(name);good=h.Observation(expected,b'',0,None,{})
        self.assertEqual(h.expectation_errors(case,good),[])
        for changed in (expected.replace(b'AFTER=[]',b'AFTER=[visible]'),
                        expected.replace(b'FLAG=[-Iinner]',b'FLAG=[]'),
                        expected.replace(b'DIR=[a- b- inner a- b-]',b'DIR=[a- b-]')):
            self.assertTrue(h.expectation_errors(case,dataclasses.replace(good,stdout=changed)))

    def test_finite_error_cases_reach_physical_metadata_and_reject_unsupported_eval(self):
        for site in ('reader','eval','recipe'):
            case=self.case('include-runtime-fatal-'+site)
            self.assertEqual(case.expected['exit_code'],2)
            error=bytes.fromhex(case.expected['stderr_hex'])
            self.assertEqual(error,b'Makefile:4: *** stop.  Stop.\n')
            paths=[p for p in case.path.iterdir() if p.is_dir() and p.name.startswith('new-')]
            self.assertEqual(len(paths),1)
            self.assertTrue((paths[0]/'keep').is_file())
            good=h.Observation(b'',error,2,None,{})
            self.assertEqual(h.expectation_errors(case,good),[])
            self.assertTrue(h.expectation_errors(case,dataclasses.replace(good,stderr=b"Makefile:4: *** unsupported expansion feature 'eval'.  Stop.\n")))

    def test_live_rhs_priority_and_append_value_have_independent_negative_controls(self):
        for kind,value,origin in [('append-live-simple',b'new tail',b'file'),
                                  ('append-live-override',b'new',b'override'),
                                  ('simple-live-override',b'new',b'override'),
                                  ('shell-live-override',b'new',b'override')]:
            name='include-rhs-'+kind;out=self.out(name)
            self.assertIn(b'X=['+value+b'] O=['+origin+b']',out)
            if kind.startswith('shell'):self.assertIn(b'R=[ran]',out)
            good=h.Observation(out,b'',0,None,{})
            self.assertTrue(h.expectation_errors(self.case(name),dataclasses.replace(good,stdout=out.replace(b'X=['+value+b']',b'X=[old tail]'))))

    def test_rhs_environment_priority_changes_after_effects(self):
        for kind in ('simple','shell'):
            case=self.case('include-rhs-environment-live-'+kind)
            self.assertEqual(case.environment,{'X':'env'})
            self.assertIn(b'X=[env] O=[environment override] F=[recursive]',self.out(case.id))

if __name__=='__main__':unittest.main()
