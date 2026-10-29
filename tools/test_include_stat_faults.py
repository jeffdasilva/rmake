# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect fault causality, observer ABI qualification and cancellation anchors."""
import dataclasses
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock
import conformance as h
import include_stat_faults as f

class IncludeStatFaultTests(unittest.TestCase):
    def test_inventory_covers_each_effect_site_recovery_and_fatal_stop(self):
        cells=f.inventory()
        self.assertEqual(len(cells),12)
        self.assertEqual(len({c.id for c in cells}),12)
        self.assertEqual({c.phase for c in cells},set(f.PHASES))
        self.assertEqual({c.mode for c in cells},set(f.MODES))
        history=json.loads((f.ROOT/'tests/research/include-stat-interruptions.json').read_text())
        self.assertIn('zero ordinary-conformance or safety-exception credit',history['classification'])
        self.assertEqual(len(history['records']),12)
        proof=history['discarded_observer']['abi_proof']
        self.assertEqual([r['returncode'] for r in proof],[0,-11,0])
        self.assertEqual(proof[0]['stdout_hex'],proof[2]['stdout_hex'])
        self.assertEqual(sum(bool(r['baseline']['candidate_differences']) for r in history['records']),6)
        for r in history['records']:
            for build in ('baseline','debug','release'):
                self.assertEqual(r[build]['model_errors'][:2],[[],[]])
                self.assertEqual(r[build]['bounds'],[False,False,False])
                if build!='baseline':self.assertEqual(r[build]['candidate_differences'],[])

        for c in cells:
            self.assertEqual(c.arguments[:3],('-rR','--no-print-directory','-I-'))
            self.assertEqual('-Iinc' in c.arguments,c.phase=='startup')
            self.assertEqual(b'forbidden' in c.source,c.mode=='signal')

    def test_trace_gate_rejects_unreached_interruption_and_missing_recovery(self):
        cell=f.Cell('reader','eintr-3')
        for operation in ('stat','stat64','statx'):
            good=''.join(f'eintr {operation} {n}\n' for n in (1,2,3))+f'real {operation} 4\n'
            self.assertEqual(f.trace_errors(cell,good),[])
            for bad in ('',good.splitlines()[0]+'\n',good.replace('eintr','real'),good.replace(' 4',' 9')):
                self.assertTrue(f.trace_errors(cell,bad))

    def test_fatal_stop_rejects_later_syscalls_effects_diagnostics_and_wrong_status(self):
        cell=f.Cell('recipe','signal');good=h.Observation(b'',b'',-2,None,{})
        self.assertEqual(f.anchors(cell,good,'eintr statx 1\n'),[])
        self.assertTrue(f.anchors(cell,good,'eintr statx 1\nreal statx 2\n'))
        for changes in (dict(stdout=b'VALUE=[ok]\n'),dict(stderr=b'interrupted\n'),dict(returncode=2),
                        dict(limit='timeout'),dict(leftover_processes=True),dict(filesystem={'work/forbidden':{}})):
            self.assertTrue(f.anchors(cell,dataclasses.replace(good,**changes),'eintr statx 1\n'))

    def test_abi_qualification_failure_is_preserved_and_never_accepted(self):
        results=[mock.Mock(returncode=0,stdout=b'statx-null=0:0\n',stderr=b''),
                 mock.Mock(returncode=-11,stdout=b'',stderr=b'')]
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(f.subprocess,'run',side_effect=results):
            with self.assertRaises(h.HarnessError):f.qualify_abi(Path('/client'),Path('/library'),Path(directory))
            self.assertTrue((Path(directory)/'abi-qualification-failure.json').is_file())

    def test_oracle_phase_disagreement_preserves_evidence_and_stops_before_candidate(self):
        good=h.Observation(b'VALUE=[ok]\n',b'',0,None,{})
        with tempfile.TemporaryDirectory() as directory, \
             mock.patch.object(f,'compile_helpers',return_value=(Path('/library'),Path('/client'),{})), \
             mock.patch.object(f,'qualify_abi',return_value=[]), \
             mock.patch.object(f,'observe',side_effect=[(good,'real stat 1\n'),(good,'')]) as observe:
            with self.assertRaises(h.HarnessError):
                f.campaign((f.Cell('startup','normal'),),Path('/oracle'),Path('/candidate'),Path(directory),1)
            self.assertEqual(observe.call_count,2)
            self.assertTrue((Path(directory)/'oracle-failure-include-stat-startup-normal.json').is_file())

if __name__=='__main__':unittest.main()
