# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Independent restart-number, export-lifetime and activation anchors."""
import dataclasses
import json
from pathlib import Path
import tomllib
import unittest
import conformance as h
ROOT=Path(__file__).resolve().parents[1]

class RestartStateTests(unittest.TestCase):
    def case(self,name):return h.load_case(ROOT/'tests/cases'/name)
    def out(self,name):return bytes.fromhex(self.case(name).expected['stdout_hex'])

    def test_numeric_controls_cover_each_reconstruction_path_and_boundary(self):
        ledger=tomllib.loads((ROOT/'tests/requirements.toml').read_text())
        family=next(r for r in ledger['requirements'] if r['id']=='startup.restart-state')
        self.assertEqual((family['fixture_count'],family['candidate_required']),(214,214))
        for index in range(21):
            for route in ('direct','effect','snapshot'):
                for mode in ('serial','pipe','sync'):
                    c=self.case(f'restart-state-n{index:02}-{route}-{mode}')
                    self.assertEqual(c.candidate,'required')
                    if route=='snapshot':
                        self.assertIn('A=',c.args)
                        self.assertEqual(bytes.fromhex(c.expected['stderr_hex']),b"make: warning: undefined variable 'MAKEFLAGS'\n"*2)
        self.assertEqual(self.out('restart-state-n09-effect-pipe'),b'R=[4294967295] A=[4294967295]\nR=[0] A=[0]\n')
        self.assertEqual(self.out('restart-state-n19-effect-sync'),b'R=[129] A=[129]\nR=[130] A=[130]\n')
        self.assertIn(b'R=[ \x0b+2xx]',self.out('restart-state-n16-effect-serial'))

    def test_unsigned_wrap_and_visible_reassignment_do_not_replace_process_counter(self):
        self.assertEqual(self.out('restart-state-wrap-always'),b'R=[4294967295] A=[4294967295] N=[0]\nR=[0] A=[0] N=[1]\nR=[1] A=[1] N=[2]\n')
        for name in ('command-five','eval-five','file-five'):
            self.assertEqual(self.out('restart-state-always-'+name),b'R=[0] N=[0]\n')
        self.assertEqual(self.out('restart-state-always-command-zero'),b'R=[5] N=[0]\nR=[5] N=[1]\n')
        case=self.case('restart-state-wrap-always');good=h.Observation(self.out(case.id),b'',0,None,{})
        self.assertEqual(h.expectation_errors(case,good),[])
        self.assertTrue(h.expectation_errors(case,dataclasses.replace(good,stdout=good.stdout.replace(b'R=[0] A=[0]',b'R=[1] A=[1]'))))

    def test_no_export_policy_is_a_binding_lifetime_not_a_global_name_ban(self):
        for route in ('direct','effect'):
            for label,value in [('default','unset'),('export','5'),('unexport','unset'),('all','unset'),('undef-all','7'),('undef-file','unset')]:
                self.assertEqual(self.out('restart-state-policy-'+route+'-'+label),f'E=[{value}]\n'.encode())
        self.assertEqual(self.out('restart-state-created-command'),b'E=[7]\n')
        self.assertEqual(self.out('restart-state-created-command-unexport'),b'E=[unset]\n')
        self.assertEqual(self.out('restart-state-child'),b'R=[undefined]/[]\n')
        self.assertEqual(self.out('restart-state-shell'),b'A=[unset]\n')

    def test_early_specials_and_finite_directory_research_keep_their_classification(self):
        ledger=tomllib.loads((ROOT/'tests/requirements.toml').read_text())
        family=next(r for r in ledger['requirements'] if r['id']=='cli.early-flag-activation')
        self.assertEqual(family['candidate_required'],8)
        for name in family['cases']:
            if not name.endswith('-include'):self.assertEqual(self.out(name),b'A=[]\n')
        case=self.case('early-flags-inactive-include')
        self.assertEqual(case.expected['exit_code'],2)
        record=json.loads((ROOT/'tests/research/imported-restart-directory-markers.json').read_text())
        self.assertIn('zero conformance or safety-exception credit',record['classification'])
        self.assertEqual(len(record['records']),2)
        for row in record['records']:
            self.assertFalse((ROOT/'tests/cases'/row['id']).exists())
            self.assertEqual(row['after'][0]['stdout_hex'],row['after'][1]['stdout_hex'])
            self.assertNotEqual(row['after'][0]['stdout_hex'],row['after'][2]['stdout_hex'])
            for o in row['after']:
                self.assertEqual(o['returncode'],0)
                self.assertIsNone(o['limit'])
                self.assertFalse(o['leftover_processes'])

if __name__=='__main__':unittest.main()
