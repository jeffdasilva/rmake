# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Independent anchors reject matching but incorrect mixed-engine results."""
from pathlib import Path
import tempfile
import unittest

import conformance as harness
import jobserver_interop as campaign


class AnchorsTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.work = Path(self.temporary.name)
        for name, value in [('active', b'0'), ('maximum', b'3'),
                            *[(node, b'complete\n') for node in campaign.NODES],
                            ('recursive.done', b'')]:
            (self.work / name).write_bytes(value)
        self.observation = harness.Observation(b'complete\n', b'', 0, None, {}, False)

    def test_exact_completion_and_file_anchors_are_required(self):
        self.assertEqual(campaign.anchors(self.work, self.observation), [])
        (self.work / 'inner1').unlink()
        self.assertIn('file anchor: inner1', campaign.anchors(self.work, self.observation))

    def test_serial_execution_and_oversubscription_both_fail(self):
        for value in (b'1', b'2', b'4'):
            (self.work / 'maximum').write_bytes(value)
            self.assertIn('file anchor: maximum', campaign.anchors(self.work, self.observation))
        (self.work / 'maximum').write_bytes(b'3')
        (self.work / 'active').write_bytes(b'1')
        self.assertIn('file anchor: active', campaign.anchors(self.work, self.observation))

    def test_resource_limits_leftovers_and_wrong_error_exit_cannot_pass(self):
        self.observation.limit = 'timeout'
        self.assertIn('process containment', campaign.anchors(self.work, self.observation))
        self.observation.limit = None
        self.observation.leftover_processes = True
        self.assertIn('process containment', campaign.anchors(self.work, self.observation))
        self.observation.leftover_processes = False
        self.assertIn('failure status or output', campaign.anchors(self.work, self.observation, True))
        self.observation.returncode = 2
        self.observation.stdout = b''
        self.observation.stderr = b'make[1]: *** [child.mk:8: bad] Error 7\n'
        self.assertEqual(campaign.anchors(self.work, self.observation, True), [])
