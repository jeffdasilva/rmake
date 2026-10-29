# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect independently modeled startup recursion and diagnostic boundaries."""
import dataclasses
import json
from pathlib import Path
import tomllib
import unittest
import conformance as h
ROOT=Path(__file__).resolve().parents[1]

class StartupMetadataLevelTests(unittest.TestCase):
    def case(self,name):return h.load_case(ROOT/'tests/cases'/name)
    def out(self,name):return bytes.fromhex(self.case(name).expected['stdout_hex'])

    def test_inventory_keeps_numeric_input_roles_and_incomplete_models_separate(self):
        ledger=tomllib.loads((ROOT/'tests/requirements.toml').read_text())
        family=next(r for r in ledger['requirements'] if r['id']=='startup.metadata-recursion')
        self.assertEqual((family['fixture_count'],family['candidate_required']),(55,55))
        self.assertEqual(self.out('startup-metadata-level-n07-environment-none'),b'SEEN=[D0]\n')
        self.assertEqual(self.out('startup-metadata-level-n07-command-none'),b'SEEN=[D1]\n')
        p=json.loads((ROOT/'tests/research/startup-metadata-level.json').read_text())
        self.assertIn('no separate conformance or safety-exception credit',p['classification'])
        self.assertEqual(len(p['records']),55)
        self.assertEqual(sum(bool(r['baseline']['candidate_differences']) for r in p['records']),35)
        self.assertEqual(len(p['initial_incomplete_models']),2)
        for r in p['records']:
            self.assertEqual(self.case(r['id']).expected,r['expected'])
            for build in ('baseline','debug','release'):
                o=r[build]
                self.assertEqual(o['oracle_repeat_differences'],[])
                self.assertEqual(o['model_errors'][:2],[[],[]])
                self.assertEqual(o['bounds'],[False,False,False])
                if build!='baseline':self.assertEqual(o['candidate_differences'],[])

    def test_cli_effects_precede_level_selection_and_restarts_repeat_metadata(self):
        for placement in ('environment','before','after'):
            name='startup-metadata-level-cli-boundary-'+placement
            out=b'EARLY=[E1] LATE=[D4]\n';self.assertEqual(self.out(name),out)
            good=h.Observation(out,b'',0,None,{})
            for wrong in (b'EARLY=[E4] LATE=[D4]\n',b'EARLY=[E1] LATE=[D1]\n'):
                self.assertTrue(h.expectation_errors(self.case(name),dataclasses.replace(good,stdout=wrong)))
        for source in ('environment','command'):
            case=self.case('startup-metadata-level-warning-boundary-'+source)
            self.assertEqual(bytes.fromhex(case.expected['stderr_hex']),b'make: cli\nmake[2]: metadata\n')
            for mode in ('none','target'):
                name='startup-metadata-level-restart-'+source+'-'+mode
                self.assertEqual(self.out(name),b'N=[1] D=[D4]\nN=[2] D=[D4]\n')

    def test_metadata_errors_never_discard_a_real_definition_source(self):
        for source in ('environment','command'):
            case=self.case('startup-metadata-level-error-2-'+source)
            self.assertEqual(bytes.fromhex(case.expected['stderr_hex']),b'make[2]: *** stop.  Stop.\n')
            for level in (0,2):
                case=self.case(f'startup-metadata-level-source-{level}-'+source)
                error=bytes.fromhex(case.expected['stderr_hex'])
                self.assertEqual(error,b'vars.mk:1: *** stop.  Stop.\n')
                self.assertEqual((case.path/'vars.mk').read_bytes(),b'A = $(error stop)\n')
                good=h.Observation(b'',error,2,None,{})
                self.assertTrue(h.expectation_errors(case,dataclasses.replace(good,stderr=b'make: *** stop.  Stop.\n')))

if __name__=='__main__':unittest.main()
