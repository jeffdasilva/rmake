# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect original byte-exact trace lifetime, workspace inputs and file witnesses."""
import copy
import dataclasses
import json
import os
from pathlib import Path
import unittest
import conformance as h
import imported_trace as gate

ROOT=Path(__file__).resolve().parents[1]

def observation(raw):
    return h.Observation(bytes.fromhex(raw['stdout_hex']),bytes.fromhex(raw['stderr_hex']),raw['returncode'],raw['limit'],raw['filesystem'],raw['leftover_processes'])

class ImportedTraceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cells={c['id']:c for c in gate.inventory()}
        cls.reports={b:json.loads((ROOT/f'tests/research/imported-trace-{b}.json').read_text())for b in ('before','debug','release')}
        cls.debug={r['id']:r for r in cls.reports['debug']['records']}

    def test_inventory_and_frozen_raw_observations_are_all_qualified(self):
        self.assertEqual(len(self.cells),482)
        for build in ('debug','release'):
            prototype=json.loads((ROOT/f'tests/research/imported-trace-unconditional-{build}.json').read_text())
            self.assertEqual((len(prototype['records']),prototype['failures']),(482,0))
        before=self.reports['before']['records']
        self.assertEqual(sum(bool(r['candidate_differences'])for r in before),242)
        for r in before:
            self.assertEqual((r['model_errors'][:2],r['oracle_repeat_differences'],r['bounds']),([[],[]],[],[False]*3))
            self.assertEqual(r['inputs'],self.cells[r['id']]['inputs'])
        for build in ('debug','release'):
            proof=self.reports[build];self.assertEqual(proof['failures'],0)
            self.assertEqual({r['id']for r in proof['records']},set(self.cells))
            for r in proof['records']:
                cell=self.cells[r['id']]
                self.assertEqual((r['args'],r['environment'],r['inputs']),(cell['args'],cell['environment'],cell['inputs']))
                self.assertEqual(r['expected'],gate.expected(cell,Path(r['workspace_input'])))
                self.assertEqual((r['errors'],r['repeat'],r['differences'],r['bounds']),([[],[],[]],[],[],[False]*3))
                for raw in r['observations']:self.assertEqual(gate.anchors(cell,observation(raw),r['expected']),[])

    def test_exact_historical_reproducers_keep_original_sources_arguments_and_environments(self):
        old=json.loads((ROOT/'tests/research/imported-restart-directory-markers.json').read_text())['records']
        self.assertEqual(len(old),2)
        for r in old:
            cell=self.cells['traced-exact-'+r['id']]
            self.assertEqual((cell['args'],cell['environment']),(r['args'],r['environment']))
            self.assertEqual({n:v['hex']for n,v in cell['inputs'].items()},r['inputs_hex'])
            self.assertTrue(bytes.fromhex(cell['expected_template']['stdout_hex']).endswith(b"make: Leaving directory '{WORK}'\n"))
            self.assertNotIn(b'Leaving',bytes.fromhex(r['initial_model']['stdout_hex']))

    def test_workspace_paths_are_raw_model_inputs_without_output_normalization(self):
        cell=self.cells['traced-restart-traced-chdir-l2-target-i1']
        saved=copy.deepcopy(cell)
        path=Path(os.fsdecode(b"/controlled/quote'\xff/work"))
        model=gate.expected(cell,path)
        self.assertEqual(bytes.fromhex(model['stdout_hex']),b"BODY\nmake[2]: Leaving directory '/controlled/quote'\xff/work/child'\n")
        self.assertEqual(cell,saved)
        record=self.debug[cell['id']];good=observation(record['observations'][2])
        self.assertTrue(gate.anchors(cell,good,model))

    def test_missing_duplicate_wrong_identity_and_wrong_directory_messages_fail(self):
        cell=self.cells['traced-restart-traced-quiet-l2-none-i0']
        r=self.debug[cell['id']];good=observation(r['observations'][2])
        self.assertEqual(gate.anchors(cell,good,r['expected']),[])
        for output in (b'',good.stdout*2,good.stdout.replace(b'make[2]',b'make'),good.stdout.replace(b'work',b'elsewhere'),good.stdout.replace(b'Leaving',b'Entering')):
            self.assertNotEqual(output,good.stdout)
            self.assertTrue(gate.anchors(cell,dataclasses.replace(good,stdout=output),r['expected']))

    def test_source_modes_generated_bytes_and_no_extra_effects_are_required(self):
        cell=self.cells['traced-restart-traced-remake-l2-target-i1'];r=self.debug[cell['id']];good=observation(r['observations'][2])
        mutations=[]
        fs=copy.deepcopy(good.filesystem);del fs['work/generated.mk'];mutations.append(fs)
        fs=copy.deepcopy(good.filesystem);fs['work/after-fatal']=dict(fs['work/Makefile']);mutations.append(fs)
        fs=copy.deepcopy(good.filesystem);fs['work/Makefile']['mode']=0o600;mutations.append(fs)
        fs=copy.deepcopy(good.filesystem);fs['work/generated.mk']['content']='V1JPTkc=';mutations.append(fs)
        for fs in mutations:self.assertTrue(gate.anchors(cell,dataclasses.replace(good,filesystem=fs),r['expected']))

if __name__=='__main__':unittest.main()
