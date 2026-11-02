# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect live-source fault causality, effect anchors and frozen profiles."""
import base64
import dataclasses
import hashlib
import json
import unittest
import conformance as h
import source_streams as f

class SourceStreamTests(unittest.TestCase):
    def observation(self,cell):
        fs={'work/anchor':{'content':base64.b64encode(cell.files['anchor']).decode()},'work/before.marker':{}}
        if not cell.failed:fs['work/after.marker']={}
        if cell.fault!='normal':fs['work/source.trace']={'content':base64.b64encode((cell.fault+'\n').encode()).decode()}
        return h.Observation(bytes.fromhex(cell.expected['stdout_hex']),bytes.fromhex(cell.expected['stderr_hex']),cell.expected['exit_code'],None,fs)

    def test_inventory_covers_sources_eval_reentry_preloads_and_all_faults(self):
        cells=f.inventory()
        self.assertEqual((len(cells),len({c.id for c in cells})),(36,36))
        for site in f.SITES:
            selected=[c for c in cells if c.site==site]
            self.assertEqual({c.fault for c in selected},set(f.FAULTS))
            for c in selected:
                self.assertEqual(f.validate(c,self.observation(c)),[])
                self.assertEqual(c.files[c.target].count(b'$(file >before.marker'),1)
        self.assertEqual(f.Cell('deep','normal').files['Makefile'].count(b'$(eval $$(R'),512)
        self.assertEqual(f.Cell('preload','normal').environment('/probe')['MAKEFILES'],'source-probe.mk')

    def test_trace_requires_exactly_one_reached_fault(self):
        for cell in f.inventory():
            good=self.observation(cell)
            for trace in (b'wrong\n',(cell.fault+'\n').encode()*2):
                if cell.fault=='normal' and not trace:continue
                fs=dict(good.filesystem);fs['work/source.trace']={'content':base64.b64encode(trace).decode()}
                self.assertTrue(f.validate(cell,dataclasses.replace(good,filesystem=fs)))
            if cell.fault!='normal':
                fs=dict(good.filesystem);fs.pop('work/source.trace')
                self.assertTrue(f.validate(cell,dataclasses.replace(good,filesystem=fs)))

    def test_prior_effect_and_unchanged_anchor_are_required(self):
        for cell in f.inventory():
            good=self.observation(cell)
            for name in ('work/before.marker','work/anchor'):
                fs=dict(good.filesystem);fs.pop(name)
                self.assertTrue(f.validate(cell,dataclasses.replace(good,filesystem=fs)))

    def test_faults_prevent_later_effects_and_close_error_retains_success(self):
        for cell in f.inventory():
            good=self.observation(cell);fs=dict(good.filesystem)
            if cell.failed:fs['work/after.marker']={}
            else:fs.pop('work/after.marker')
            self.assertTrue(f.validate(cell,dataclasses.replace(good,filesystem=fs)))
            self.assertEqual(cell.expected['exit_code'],-15 if cell.fault=='signal' else 2 if cell.fault in ('read-eio','read-eintr') else 0)
            stderr=bytes.fromhex(cell.expected['stderr_hex'])
            if cell.fault in ('read-eio','read-eintr'):
                self.assertTrue(stderr.startswith(b'make: *** '+cell.target.encode()+b': '))
            else:self.assertEqual(stderr,b'')

    def test_frozen_profiles_require_models_repeats_observer_and_both_builds(self):
        wanted={c.id for c in f.inventory()}
        observer=hashlib.sha256((f.ROOT/'tools/source_streams.c').read_bytes()).hexdigest()
        for build in ('debug','release'):
            report=json.loads((f.ROOT/f'tests/research/live-source-io-{build}.json').read_text())
            self.assertEqual((len(report['records']),report['failures']),(36,0))
            self.assertEqual({r['id']for r in report['records']},wanted)
            self.assertEqual(report['observer_sha256'],observer)
            for r in report['records']:
                self.assertEqual((r['errors'],r['repeat'],r['differences'],r['bounds']),([[],[],[]],[],[],[False]*3))
                cell=f.Cell(r['site'],r['fault'])
                self.assertEqual((r['inputs'],r['expected']),({n:d.hex()for n,d in cell.files.items()},cell.expected))

if __name__=='__main__':unittest.main()
