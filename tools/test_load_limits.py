# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect independent load campaign anchors against matching wrong engines."""
import dataclasses
from pathlib import Path
import tempfile
import unittest
import conformance
import load_limits as campaign

class LoadLimitsTests(unittest.TestCase):
    def test_inventory_covers_thresholds_recovery_errors_and_failure_queue_policy(self):
        cells = campaign.inventory()
        self.assertEqual(len(cells), 21)
        self.assertEqual(len({c.id for c in cells}), len(cells))
        self.assertTrue({'above', 'equal', 'below', 'zero', 'disabled', 'unsigned-wrap',
                         'unsigned-overflow', 'load-drops-after-peer-completion',
                         'interrupted-read', 'read-fallback', 'malformed-fallback',
                         'average-unsupported', 'average-eio', 'queued-fail',
                         'queued-fail-keep', 'queued-fail-ignore', 'load-drops-during-token-wait'} <= {c.id for c in cells})

    def test_anchors_reject_duplicate_missing_and_wrongly_ordered_serial_jobs(self):
        cell = campaign.inventory()[0]
        correct = conformance.Observation(b'active=0 maximum=1\n', b'', 0, None, {})
        self.assertEqual(campaign.anchors(cell, correct, b'a\nc\nb\n', cell.trace), [])
        for order in [b'a\nb\nc\n', b'a\nc\nc\n', b'a\nb\n', b'a\nc\nb\na\n']:
            self.assertTrue(campaign.anchors(cell, correct, order, cell.trace))
        for change in [dict(stdout=b'active=1 maximum=1\n'), dict(stdout=b'active=0 maximum=2\n'),
                       dict(stderr=b'ignored\n'), dict(returncode=2), dict(limit='timeout'),
                       dict(leftover_processes=True)]:
            self.assertTrue(campaign.anchors(cell, dataclasses.replace(correct, **change), b'a\nc\nb\n', cell.trace))
        self.assertTrue(campaign.anchors(cell, correct, b'a\nc\nb\n', b''))

    def test_parallel_completion_uses_exact_membership_and_retains_failure_policy(self):
        cell = next(c for c in campaign.inventory() if c.id == 'equal')
        correct = conformance.Observation(b'active=0 maximum=3\n', b'', 0, None, {})
        for order in [b'a\nb\nc\n', b'c\nb\na\n', b'b\na\nc\n']:
            self.assertEqual(campaign.anchors(cell, correct, order, cell.trace), [])
        self.assertTrue(campaign.anchors(cell, correct, b'a\na\nc\n', cell.trace))
        cell = next(c for c in campaign.inventory() if c.id == 'queued-fail')
        failed = conformance.Observation(b'', cell.stderr, 2, None, {})
        self.assertEqual(campaign.anchors(cell, failed, b'a\n', cell.trace), [])
        self.assertTrue(campaign.anchors(cell, failed, b'a\nc\nb\n', cell.trace))

    def test_delayed_load_transition_holds_all_three_slots_until_the_token_wait(self):
        cell = next(c for c in campaign.inventory() if c.id == 'load-drops-during-token-wait')
        self.assertEqual(cell.jobs, 3)
        self.assertEqual(cell.trace, b'proc\ntoken-wait\n')
        self.assertIn(b'$(file >expanded-d)true', cell.makefile)
        self.assertNotIn(b'$(file >load.input', cell.makefile)

    def test_materialization_preserves_controlled_load_and_expansion_barriers(self):
        cell = next(c for c in campaign.inventory() if c.behavior == 'dynamic')
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / 'input'
            campaign.materialize(cell, root)
            self.assertEqual((root / 'load.input').read_bytes(), cell.proc)
            self.assertEqual((root / 'helper.py').read_bytes(), campaign.HELPER)
            self.assertIn(b'all: a b c d\n', (root / 'Makefile').read_bytes())
            self.assertIn(b'$(file >load.input,0 0 0 0/100 1)', (root / 'Makefile').read_bytes())
            self.assertEqual((root / 'active').read_bytes(), b'0')

if __name__ == '__main__':
    unittest.main()
