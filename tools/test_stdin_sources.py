# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Negative controls for actual-source and pre-exit identity qualification."""
from copy import deepcopy
from dataclasses import replace
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import conformance as h
import stdin_sources as s


class StdinSourcesTests(unittest.TestCase):
    def setUp(self):
        self.runtime=Path('/declared/reset')
        self.row=s.models()[0]
        self.filesystem={'work/seen':s.file_record(bytes.fromhex(self.row['expected_seen_hex']))}
        self.good=s.Execution(h.Observation(bytes.fromhex(self.row['expected_stdout_hex']),b'',self.row['expected_status'],None,self.filesystem),None,None,None)

    def test_inventory_keeps_original_cases_and_bounded_publication_controls(self):
        rows=s.models();self.assertEqual(len(rows),44)
        self.assertEqual(len({row['id']for row in rows}),44)
        self.assertEqual(sum('owned_temp_identity'in row for row in rows),2)
        self.assertEqual(sum(row['id'].startswith('stdin-owned-original-')for row in rows),4)
        self.assertIn('stdin-owned-eval-flags-publication',{row['id']for row in rows})
        self.assertIn('stdin-owned-native-live-environment-restart',{row['id']for row in rows})
        with patch.object(s.h,'sha256_file',return_value='changed'):
            with self.assertRaisesRegex(h.HarnessError,'fixture identity changed'):s.models()

    def test_shared_wrong_stdout_status_or_effects_do_not_qualify(self):
        self.assertTrue(s.assess(self.row,self.runtime,self.filesystem,[self.good]*3)['passed'])
        for observed in(replace(self.good.observation,stdout=b'wrong'),replace(self.good.observation,returncode=2),
                        replace(self.good.observation,filesystem={'work/seen':s.file_record(b'wrong')})):
            wrong=replace(self.good,observation=observed)
            self.assertFalse(s.assess(self.row,self.runtime,self.filesystem,[wrong]*3)['passed'])

    def test_every_stream_bound_repeat_and_extra_file_is_mandatory(self):
        for observed in(replace(self.good.observation,stderr=b'extra'),replace(self.good.observation,limit='timeout'),
                        replace(self.good.observation,limit='output'),replace(self.good.observation,leftover_processes=True),
                        replace(self.good.observation,filesystem={**self.filesystem,'work/unexpected':s.file_record(b'bad')})):
            for index in range(3):
                runs=[self.good]*3;runs[index]=replace(self.good,observation=observed)
                self.assertFalse(s.assess(self.row,self.runtime,self.filesystem,runs)['passed'])
        with self.assertRaises(h.HarnessError):s.assess(self.row,self.runtime,self.filesystem,[self.good])

    def test_only_preexit_same_physical_identity_with_exact_full_source_can_pass(self):
        row=next(row for row in s.models()if row['id']=='stdin-owned-signal-blocked-copy')
        identity={'path_hex':bytes(self.runtime/'tmp/GmABC123').hex(),'device':7,'inode':9,'mode':0o600,'size':0,'content_hex':'','captured_before_exit':True}
        final={'device':7,'inode':9,'mode':0o600,'regular':True}
        base={'work/seen':s.file_record(bytes.fromhex(row['expected_seen_hex']))}
        filesystem={**base,'tmp/GmABC123':s.file_record(b'',0o600)}
        good=s.Execution(h.Observation(b'',b'',-15,None,filesystem),identity,final,json.dumps(identity).encode().hex())
        self.assertTrue(s.assess(row,self.runtime,base,[good]*3)['passed'])
        for changes in({'captured_before_exit':False},{'content_hex':b'partial'.hex(),'size':7},{'mode':0o644},
                        {'path_hex':bytes(self.runtime/'work/GmABC123').hex()},{'path_hex':bytes(self.runtime/'tmp/not-owned').hex()}):
            wrong=replace(good,identity={**identity,**changes})
            self.assertFalse(s.assess(row,self.runtime,base,[wrong]*3)['passed'])
        for wrong in(replace(good,identity=None),replace(good,final_identity={**final,'inode':10}),
                     replace(good,observation=replace(good.observation,filesystem={**filesystem,'tmp/GmOTHER1':s.file_record(b'',0o600)}))):
            self.assertFalse(s.assess(row,self.runtime,base,[wrong]*3)['passed'])

    def test_module_compiles_once_before_execution_and_uses_original_fixture(self):
        row=next(row for row in s.models()if row['id']=='stdin-owned-goal-unlinked')
        commands=[]
        def compile(command,*_):
            commands.append(command);Path(command[-1]).write_bytes(b'compiled-original');return b'',b'',0,None,False
        with tempfile.TemporaryDirectory()as temporary,patch.object(s.h,'capture',side_effect=compile):
            case,compilation=s.prepare(row,Path(temporary)/'input','/compiler with spaces')
            self.assertEqual((case.path/'module.c').read_bytes(),(s.FIXTURES/row['module_source']).read_bytes())
            self.assertEqual(compilation['module_sha256'],s.hashlib.sha256(b'compiled-original').hexdigest())
        self.assertEqual(len(commands),1);self.assertEqual(commands[0][0],'/compiler with spaces')

    def test_campaign_compiles_once_then_resets_gnu2_and_candidate(self):
        case=h.Case(Path('/unused'),self.row['id'],'stdin.owned','required',(),8,{},{});events=[]
        def prepare(*_):events.append('compile');return case,None
        def execute(*args):events.append(str(args[3]));return self.good
        with tempfile.TemporaryDirectory()as temporary,patch.object(s,'prepare',side_effect=prepare),\
             patch.object(s,'expected_filesystem',return_value=self.filesystem),patch.object(s,'execute',side_effect=execute):
            report=s.run_campaign([self.row],Path('/oracle'),Path('/candidate'),'/cc',Path(temporary))
        self.assertTrue(report['passed']);self.assertEqual(events,['compile','/oracle','/oracle','/candidate'])

    def test_failed_candidate_is_separate_nonzero_report_and_bad_selector_is_rejected(self):
        with tempfile.TemporaryDirectory()as temporary:
            report=Path(temporary)/'report.json';args=['--oracle','/oracle','--candidate','/candidate','--report',str(report)]
            with patch.object(s.shutil,'which',return_value='/cc'),patch.object(s.reference_oracle,'verify'),\
                 patch.object(s.h,'executable_identity',side_effect=lambda path,**_:{'path':str(path)}),\
                 patch.object(s,'run_campaign',return_value={'passed':False,'totals':{'fail':1}}):
                self.assertEqual(s.main(args),1);self.assertFalse(json.loads(report.read_bytes())['passed'])
            self.assertEqual(s.main([*args,'--case','unknown']),2)

    def test_only_declared_file_updates_and_removals_enter_expected_filesystem(self):
        row=deepcopy(self.row);row['expected_updates_hex']={'generated.mk':b'ready\n'.hex()};row['expected_removals']=['owned.mk']
        initial={'work/seen':s.file_record(b'anchor\n'),'work/owned.mk':s.file_record(b'owned'),'work/input.mk':s.file_record(b'input')}
        with patch.object(s.h,'reset_workspace'),patch.object(s.h,'snapshot',return_value=initial):
            expected=s.expected_filesystem(row,h.Case(Path('/unused'),'one','stdin.owned','required',(),8,{},{}),self.runtime,Path('/oracle'))
        self.assertNotIn('work/owned.mk',expected)
        self.assertEqual(expected['work/generated.mk'],s.file_record(b'ready\n'))
        self.assertEqual(expected['work/input.mk'],s.file_record(b'input'))


if __name__=='__main__':unittest.main()
