# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Independent controls for CLI phase, operand policy and large ordered argv."""
import dataclasses
from pathlib import Path
import tomllib
import unittest
import conformance as h
ROOT=Path(__file__).resolve().parents[1]

class OrderedStartupTests(unittest.TestCase):
    def case(self,name): return h.load_case(ROOT/'tests/cases'/('ordered-cli-'+name.lower()))
    def out(self,name): return bytes.fromhex(self.case(name).expected['stdout_hex'])
    def err(self,name): return bytes.fromhex(self.case(name).expected['stderr_hex'])

    def test_phase_matrix_is_required_and_early_metadata_is_absent(self):
        ledger=tomllib.loads((ROOT/'tests/requirements.toml').read_text())
        family=next(r for r in ledger['requirements'] if r['id']=='cli.ordered-startup')
        self.assertEqual((family['fixture_count'],family['candidate_required']),(126,126))
        for profile in ('defaults','r','big-r','r-big-r'):
            for name in ('MAKE_VERSION','MAKE_HOST','MAKE_COMMAND','MAKE','MAKEFILES','CURDIR','atD','atF'):
                case=self.case('metadata-'+profile+'-'+name)
                self.assertEqual(case.candidate,'required')
                self.assertEqual(bytes.fromhex(case.expected['stdout_hex']),b'BEFORE=[undefined]\n')
        self.assertEqual(self.out('goal-visibility'),b'A=[] B=[one] C=[one two]\n')
        self.assertEqual(self.out('goal-imported-raw'),b'A=[$(X)|one|environment]\n')

    def test_operand_and_warning_policies_keep_their_real_positions(self):
        self.assertEqual(self.err('warning-after'),b'')
        self.assertEqual(self.err('warning-before'),b"make: warning: undefined variable 'MISSING'\n")
        self.assertEqual(self.out('env-policy-after'),b'X=[file]/[ file]\n')
        for position in ('before','after'):
            self.assertEqual(self.err('tilde-warning-'+position),b'')
        self.assertEqual(self.err('tilde-explicit-warning-on'),b'make: EXPLICIT\n')
        self.assertEqual(self.out('tilde-duplicate-include'),b'N=[2]\n')
        self.assertEqual(self.case('tilde-duplicate-directory').expected['exit_code'],2)
        self.assertEqual(self.err('tilde-include-error'),b'home.mk:1: *** HOME.  Stop.\n')
        self.assertEqual(self.out('pre-cached-directory'),b'A=[root.item] L=[root.item]\n')
        for path in ('cwd-short-before','cwd-short-after','cwd-long-before','cwd-long-after'):
            self.assertEqual(self.out(path),b'A=[root]\n')

    def test_restarts_keep_early_context_and_repeat_the_right_filesystem_effects(self):
        for mode in ('serial','parallel','pipe','target','line','recurse'):
            for cwd in ('','-cwd'):
                for kind in ('value','name'):
                    expected=b'R=[] A=[] C=[undefined] N=['+(b'0' if cwd else b'1')+b']\nR=[1] A=[1] C=[undefined] N=['+(b'0' if cwd else b'2')+b']\n'
                    self.assertEqual(self.out('restart-'+mode+cwd+'-'+kind),expected)
        for name in ('early-fatal-help-before','early-fatal-help-after','early-fatal-version-after'):
            self.assertEqual(self.err(name),b'make: *** STOP.  Stop.\n')
            self.assertEqual(self.case(name).expected['exit_code'],2)

    def test_raw_values_and_every_scaling_position_are_observed(self):
        self.assertEqual(self.out('raw-file'),b'A=[\xfe]\n')
        self.assertEqual(self.err('raw-error'),b'make: *** \xfe.  Stop.\n')
        for count in (0,1000):
            case=self.case('scale-'+str(count))
            expected=b'values=['+b'|'.join(str(i+1).encode() for i in range(count))+b']\n'
            self.assertEqual(bytes.fromhex(case.expected['stdout_hex']),expected)
            self.assertEqual(len(case.args),2*count+3)
            good=h.Observation(expected,b'make: *** END.  Stop.\n',2,None,{})
            self.assertEqual(h.expectation_errors(case,good),[])
            wrong=expected.replace(b'|501|',b'|WRONG|') if count else b'values=[wrong]\n'
            self.assertTrue(h.expectation_errors(case,dataclasses.replace(good,stdout=wrong)))

if __name__=='__main__':unittest.main()
