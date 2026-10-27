# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect independently anchored runtime interruption and terminal profiles."""
import base64
import dataclasses
import unittest
import conformance
import cancellation_effects as effects
import output_lock as locks
import runtime_terminals as terminals

class RuntimeEffectTests(unittest.TestCase):
    def test_effect_inventory_covers_every_phase_signal_and_buffering_mode(self):
        cells=effects.inventory()
        self.assertEqual(len(cells),294)
        self.assertEqual(len({c.id for c in cells}),len(cells))
        self.assertEqual({c.phase for c in cells},set(effects.PHASES))
        self.assertEqual({c.number for c in cells},set(effects.SIGNALS))
        self.assertEqual({c.mode for c in cells},{'none','target','recurse'})

    def test_effect_anchors_reject_extra_diagnostics_effects_and_wrong_exit(self):
        cell=effects.Cell('reader-shell',15)
        correct=conformance.Observation(b'before\n',b'',-15,None,{})
        self.assertEqual(effects.anchors(cell,correct),[])
        for changes in [dict(stdout=b''),dict(stderr=b'interrupted\n'),dict(returncode=2),
                        dict(limit='timeout'),dict(leftover_processes=True),dict(filesystem={'work/forbidden':{}})]:
            self.assertTrue(effects.anchors(cell,dataclasses.replace(correct,**changes)))
        buffered=dataclasses.replace(cell,mode='target')
        self.assertTrue(effects.anchors(buffered,correct))
        self.assertEqual(effects.anchors(buffered,dataclasses.replace(correct,stdout=b'')),[])

    def test_lock_inventory_distinguishes_reader_completed_owner_and_serial_recurse(self):
        cells=locks.inventory()
        self.assertEqual(len(cells),76)
        self.assertEqual(len({c.id for c in cells}),len(cells))
        self.assertTrue(any(c.phase=='reader' and c.number for c in cells))
        self.assertTrue(any(c.jobs==1 and c.mode=='recurse' for c in cells))
        self.assertFalse(any(c.jobs==1 and c.mode in ('line','target') for c in cells))

    def test_lock_anchors_reject_wrong_order_and_unsafe_target_or_mutex_changes(self):
        cell=locks.Cell('target',15,changed=True)
        out,err,status=cell.expected()
        files={'work/mutex':{'content':base64.b64encode(locks.PROTECTED).decode()}}
        correct=conformance.Observation(out,err,status,None,files)
        self.assertEqual(locks.anchors(cell,correct),[])
        for changes in [dict(stdout=b'locked-output\n'),dict(stderr=err[::-1]),dict(returncode=-15),
                        dict(filesystem={**files,'work/all':{}}),dict(filesystem={}),dict(limit='timeout')]:
            self.assertTrue(locks.anchors(cell,dataclasses.replace(correct,**changes)))
        self.assertEqual(locks.Cell('target',13).expected()[1],locks.NOTICE+locks.WAIT+locks.WAIT)

    def test_terminal_inventory_covers_both_streams_forced_lines_serial_and_imports(self):
        cells=terminals.inventory()
        self.assertEqual(len(cells),64)
        self.assertEqual(len({c.id for c in cells}),len(cells))
        self.assertEqual({c.jobs for c in cells},{1,2})
        self.assertEqual({c.imported for c in cells},{False,True})
        self.assertEqual({c.merged for c in cells},{False,True})

    def test_terminal_expectations_preserve_empty_imports_and_real_stream_metadata(self):
        ordinary=terminals.Cell('target',2,False,True,False)
        out,err=ordinary.expected('/dev/pts/test')
        self.assertIn(b'stdin-tty=1\r\nstdout-tty=0\r\nstderr-tty=0\r\n',out)
        self.assertIn(b'MAKE_TERMERR=[/dev/pts/test]\r\n',out)
        self.assertEqual(err,b'')
        forced=dataclasses.replace(ordinary,forced=True,imported=True)
        out,err=forced.expected('/dev/pts/test')
        self.assertIn(b'stdout-tty=1\r\n',out)
        self.assertIn(b'MAKE_TERMOUT=[]\r\n',out)
        self.assertNotIn(b'/dev/pts/test',out)
        serial=dataclasses.replace(ordinary,jobs=1,merged=False)
        out,err=serial.expected('/dev/pts/test')
        self.assertIn(b'stdout-tty=1\r\nstderr-tty=0\r\n',out)
        self.assertIn(b'termerr-origin=undefined\r\n',out)
        self.assertEqual(err,b'child-stderr\n')

if __name__=='__main__':unittest.main()
