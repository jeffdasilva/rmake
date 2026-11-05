# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Typed ownership controls for the legacy early-stdin signal gate."""
from dataclasses import replace
import os
from pathlib import Path
import tempfile
import unittest

import cancellation_effects as c
import conformance as h


class CancellationStdinTests(unittest.TestCase):
    def setUp(self):
        self.temporary=tempfile.TemporaryDirectory();self.addCleanup(self.temporary.cleanup)
        self.directory=Path(self.temporary.name)/'owned';self.directory.mkdir(mode=0o755)
        self.source=self.directory/'GmABC123';self.source.write_bytes(b'');self.source.chmod(0o600)
        self.static={'work/Makefile':{'type':'file','mode':0o644,'content':'original'}}
        self.observed=h.Observation(b'',b'',-15,None,self.static)
        self.captured=c.capture_stdin_source(self.directory,os.getpid())

    def check(self,captured=None,observed=None):
        return c.inspect_stdin_source(self.directory,self.captured if captured is None else captured,self.static,self.observed if observed is None else observed)

    def test_actual_host_capture_has_live_before_signal_full_byte_identity(self):
        self.assertTrue(self.captured['captured_before_signal'])
        self.assertEqual(self.captured['size'],0);self.assertEqual(self.captured['content_hex'],'')
        self.assertEqual(self.captured['mode'],0o600)
        self.assertEqual(self.check()['errors'],[])

    def test_missing_identity_or_shared_wrong_mode_content_and_phase_cannot_pass(self):
        self.assertTrue(c.inspect_stdin_source(self.directory,None,self.static,self.observed)['errors'])
        for changed in({'captured_before_signal':False},{'mode':0o644},{'size':1,'content_hex':'78'}):
            self.assertTrue(self.check({**self.captured,**changed})['errors'])

    def test_only_same_physical_owned_source_is_accepted(self):
        self.assertTrue(self.check({**self.captured,'inode':self.captured['inode']+1})['errors'])
        self.assertTrue(self.check({**self.captured,'device':self.captured['device']+1})['errors'])
        self.source.unlink();self.assertTrue(self.check()['errors'])

    def test_unknown_files_or_changed_full_static_filesystem_are_never_dropped(self):
        (self.directory/'GmOTHER1').write_bytes(b'')
        self.assertTrue(self.check()['errors'])
        with self.assertRaises(ValueError):c.capture_stdin_source(self.directory,os.getpid())
        self.assertTrue(self.check(observed=replace(self.observed,filesystem={**self.static,'work/extra':{'type':'file'}}))['errors'])

    def test_wrong_namespace_and_symbolic_source_paths_are_rejected(self):
        outside=Path(self.temporary.name)/'other';outside.write_bytes(b'')
        self.assertTrue(self.check({**self.captured,'path_hex':os.fsencode(outside).hex()})['errors'])
        self.source.unlink();self.source.symlink_to(outside)
        with self.assertRaises(OSError):c.capture_stdin_source(self.directory,os.getpid())

    def test_legacy_inventory_and_stream_status_anchors_are_preserved(self):
        cells=[cell for cell in c.inventory()if cell.phase=='stdin-read']
        self.assertEqual(len(cells),21);self.assertEqual(len(c.inventory()),420)
        for cell in cells:
            good=replace(self.observed,returncode=1 if cell.number==3 else-cell.number)
            self.assertEqual(c.anchors(cell,good),[])
            for bad in(replace(good,stdout=b'late'),replace(good,stderr=b'late'),replace(good,returncode=0),replace(good,limit='timeout')):
                self.assertTrue(c.anchors(cell,bad))


if __name__=='__main__':unittest.main()
