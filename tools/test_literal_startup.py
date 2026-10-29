# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Keep original operand, phase and reconstruction models independently anchored."""
import base64
import dataclasses
import json
from pathlib import Path
import tomllib
import unittest
import conformance as h

ROOT=Path(__file__).resolve().parents[1]

class LiteralStartupTests(unittest.TestCase):
    def case(self,name):return h.load_case(ROOT/'tests/cases'/name)
    def out(self,name):return bytes.fromhex(self.case(name).expected['stdout_hex'])
    def evidence(self):return json.loads((ROOT/'tests/research/literal-startup.json').read_text())

    def test_inventory_retains_both_input_classes_and_rejected_candidate_evidence(self):
        ledger=tomllib.loads((ROOT/'tests/requirements.toml').read_text())
        family=next(r for r in ledger['requirements'] if r['id']=='startup.literal-arguments')
        self.assertEqual((family['fixture_count'],family['candidate_required']),(146,146))
        evidence=self.evidence();records=evidence['records']
        self.assertIn('no separate conformance or safety-exception credit',evidence['classification'])
        self.assertEqual(len(records),146)
        self.assertEqual(sum(bool(r['baseline']['candidate_differences']) for r in records),58)
        self.assertEqual(sum(bool(r['rejected']['candidate_differences']) for r in records),22)
        self.assertEqual(len(evidence['rejected_candidate']['failures']),72)
        self.assertEqual(evidence['rejected_candidate']['ordinary_results'],dict(pass_count=10848,fail_count=72,pending_count=0,xpass_count=0))
        for r in records:
            case=self.case(r['id'])
            self.assertEqual(case.expected,r['expected'])
            self.assertEqual(case.args,tuple(r['args']))
            self.assertEqual(case.environment,r['environment'])
            for name,content in r['inputs_hex'].items():self.assertEqual((case.path/name).read_bytes(),bytes.fromhex(content))
            for build in ('baseline','rejected','debug','release'):
                o=r[build]
                self.assertEqual(o['oracle_repeat_differences'],[])
                self.assertEqual(o['model_errors'][:2],[[],[]])
                self.assertEqual(o['bounds'],[False,False,False])
                if build in ('debug','release'):self.assertEqual(o['candidate_differences'],[])

    def test_operand_comparison_precedes_normalization_and_makefiles_remain_repeatable(self):
        for route in ('literal','prepared'):
            for form in ('short','long'):
                first=f'literal-startup-directory-{form}-3-{route}'
                second=f'literal-startup-directory-{form}-4-{route}'
                self.assertEqual(self.out(first),b'LOC=first\n')
                self.assertEqual(self.out(second),b'LOC=second\n')
                good=h.Observation(b'LOC=first\n',b'',0,None,{})
                self.assertTrue(h.expectation_errors(self.case(first),dataclasses.replace(good,stdout=b'LOC=second\n')))
                name=f'literal-startup-file-repeat-{form}-4-{route}'
                expected=b'LIST=[Makefile]\nLIST=[Makefile Makefile]\n'
                self.assertEqual(self.out(name),expected)
                self.assertTrue(h.expectation_errors(self.case(name),dataclasses.replace(good,stdout=b'LIST=[./Makefile]\n')))

    def test_initial_warnings_and_directory_errors_use_distinct_recursion_phases(self):
        for route in ('literal','prepared'):
            case=self.case('literal-startup-warning-3-'+route)
            warning=b"make: warning: undefined variable 'MAKEFLAGS'\n"
            self.assertEqual(bytes.fromhex(case.expected['stderr_hex']),warning)
            good=h.Observation(b'',warning,0,None,{})
            self.assertTrue(h.expectation_errors(case,dataclasses.replace(good,stderr=warning.replace(b'make:',b'make[3]:'))))
            for form in ('short','long'):
                for placement in ('before-three','after-three'):
                    case=self.case(f'literal-startup-directory-error-{form}-{placement}-{route}')
                    error=b'make[3]: *** missing: No such file or directory.  Stop.\n'
                    self.assertEqual(bytes.fromhex(case.expected['stderr_hex']),error)
                    self.assertTrue(h.expectation_errors(case,dataclasses.replace(good,returncode=2,stderr=error.replace(b'make[3]',b'make[2]'))))

    def test_reconstruction_keeps_file_effects_and_retired_pool_diagnostics(self):
        for record in self.evidence()['records']:
            name=record['id']
            if not ('directory-restart-' in name or 'return-failure-' in name):continue
            for build in ('debug','release'):
                for observation in record[build]['observations']:
                    fs=observation['filesystem']
                    if 'return-failure-' in name:
                        self.assertEqual(fs['work'],{'type':'missing'})
                        path='home/retired/d/generated.mk'
                        error=bytes.fromhex(observation['stderr_hex'])
                        self.assertIn(b'make: chdir: No such file or directory\n',error)
                        if '-1-fifo-' not in name:self.assertIn(b'INTERNAL: Exiting with 1 jobserver tokens available;',error)
                    else:
                        selected='d' if b'LOC=first' in bytes.fromhex(record['expected']['stdout_hex']) else 'd/d'
                        path='work/'+selected+'/generated.mk'
                    self.assertEqual(base64.b64decode(fs[path]['content']),b'# generated\n')

if __name__=='__main__':unittest.main()
