# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Negative controls for the narrowly permitted startup branding differences."""
import base64
import dataclasses
import unittest
import conformance as h
import startup_exits as exits

BANNERS = {'help': b'HELP\n', 'version': b'VERSION\n'}

class StartupExitTests(unittest.TestCase):
    def test_complete_phase_matrix(self):
        cells = exits.inventory()
        self.assertEqual(len(cells), 128)
        self.assertEqual(len({cell.id for cell in cells}), 128)
        self.assertEqual({cell.action for cell in cells}, {'help','version','help-version','version-help'})
        self.assertEqual({cell.mode for cell in cells}, {'none','line','target','recurse'})
        self.assertTrue(all('$(file >eval-forbidden,E)' in cell.args for cell in cells))

    def test_banner_order_and_fatal_visibility_are_independent(self):
        self.assertEqual(exits.Cell('help-version','after',False,False,'target').expected(BANNERS),
                         b'BEFORE\nmake: WARN\nVERSION\n\nHELP\n')
        self.assertEqual(exits.Cell('help-version','after',False,True,'target').expected(BANNERS),
                         b'VERSION\n\nHELP\nBEFORE\nmake: WARN\n')
        self.assertEqual(exits.Cell('version','before',True,True,'target').expected(BANNERS),
                         b'VERSION\nBEFORE\nmake: WARN\nmake: *** STOP.  Stop.\n')
        self.assertEqual(exits.Cell('version','after',True,True,'target').expected(BANNERS),
                         b'BEFORE\nmake: WARN\nmake: *** STOP.  Stop.\n')

    def good(self, cell):
        files={'work/seen':{'content':base64.b64encode(b'E\n').decode()}}
        if not cell.fatal: files['work/after']={'content':base64.b64encode(b'E\n').decode()}
        return h.Observation(cell.expected(BANNERS),b'',2 if cell.fatal else 0,None,files)

    def test_banner_exemption_rejects_stream_status_position_and_effect_mutations(self):
        for cell in (exits.Cell('version','before',True,True,'target'), exits.Cell('help-version','after',False,False,'none')):
            good=self.good(cell)
            self.assertEqual(exits.anchors(cell,good,BANNERS),[])
            for changes in (dict(stdout=good.stdout.replace(b'make: WARN\n',b'')),
                            dict(stdout=good.stdout+BANNERS['version']),
                            dict(stdout=good.stdout.replace(b'BEFORE\n',b'AFTER\n')),
                            dict(stderr=b'extra\n'),dict(returncode=-15),dict(limit='timeout'),
                            dict(leftover_processes=True),dict(filesystem={}),
                            dict(filesystem={**good.filesystem,'work/eval-forbidden':{'type':'file'}})):
                self.assertTrue(exits.anchors(cell,dataclasses.replace(good,**changes),BANNERS),changes)
            wrong=dataclasses.replace(good,stdout=good.stdout.replace(b'VERSION\n',b'')+b'VERSION\n')
            self.assertTrue(exits.anchors(cell,wrong,BANNERS))

    def test_later_cli_arguments_are_required_on_success_and_forbidden_after_fatal(self):
        fatal=exits.Cell('help','before',True,False,'none'); good=self.good(fatal)
        self.assertTrue(exits.anchors(fatal,dataclasses.replace(good,filesystem={**good.filesystem,'work/after':{'type':'file'}}),BANNERS))
        normal=dataclasses.replace(fatal,fatal=False); good=self.good(normal)
        self.assertTrue(exits.anchors(normal,dataclasses.replace(good,filesystem={'work/seen':good.filesystem['work/seen']}),BANNERS))

if __name__=='__main__':unittest.main()
