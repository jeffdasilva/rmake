# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Independent signal anchors must reject equal-but-wrong implementations."""
import dataclasses
from pathlib import Path
import signal
import tempfile
import unittest
import conformance
import runtime_signals as campaign

class RuntimeSignalTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for name in ('work', 'home', 'tmp'): (self.root / name).mkdir()
        (self.root / 'work/protected').write_bytes(campaign.PROTECTED)

    def test_inventory_exercises_each_delivery_transport_and_protection_policy(self):
        cells = campaign.inventory()
        self.assertEqual(len(cells), 337)
        self.assertEqual(len({cell.id for cell in cells}), len(cells))
        for number in campaign.SIGNALS:
            for delivery in ('parent', 'group', 'child'):
                self.assertTrue(any(c.number == number and c.delivery == delivery and c.kind == 'pair' and c.style == 'fifo' for c in cells))
        self.assertTrue({'precious', 'phony', 'unchanged', 'same-mtime', 'directory',
                         'symlink', 'grouped', 'intermediate', 'ignore-recipe', 'last', 'pair'} <= {c.kind for c in cells})

    def test_child_failure_anchors_reject_changed_status_output_targets_and_leaks(self):
        cell = campaign.Cell(signal.SIGTERM, 'child')
        out, err, status = cell.expected
        observation = conformance.Observation(out, err, status, None, {})
        self.assertEqual(campaign.anchors(cell, observation, ['ready', 'sent'], self.root), [])
        for changes in [dict(returncode=0), dict(stderr=b''), dict(stdout=b'extra\n'),
                        dict(limit='timeout'), dict(leftover_processes=True)]:
            self.assertTrue(campaign.anchors(cell, dataclasses.replace(observation, **changes), ['ready', 'sent'], self.root))
        (self.root / 'work/out').write_bytes(b'changed\n')
        self.assertIn('target deletion policy', campaign.anchors(cell, observation, ['ready', 'sent'], self.root))
        (self.root / 'work/out').unlink()
        (self.root / 'work/tail').touch()
        self.assertIn('continuation cancellation', campaign.anchors(cell, observation, ['ready', 'sent'], self.root))

    def test_parent_acknowledgment_and_parallel_reap_order_are_required(self):
        cell = campaign.Cell(signal.SIGINT, 'parent', 2, 'pipe', 'pair')
        observation = conformance.Observation(*cell.expected[:2], cell.expected[2], None, {})
        events = ['ready', 'sent', 'acknowledged', 'release-a', 'release-b']
        self.assertEqual(campaign.anchors(cell, observation, events, self.root), [])
        self.assertTrue(campaign.anchors(cell, observation, ['ready', 'sent'], self.root))
        self.assertTrue(campaign.anchors(cell, observation, ['sent', 'ready', 'acknowledged', 'release-b', 'release-a'], self.root))
        (self.root / 'work/b.tail').touch()
        self.assertIn('ordinary vs fatal peer continuation', campaign.anchors(cell, observation, events, self.root))

    def test_external_pool_retains_distinctive_bytes_and_borrowed_identity(self):
        for style in ('fifo', 'pipe'):
            pool = campaign.ExternalPool(self.root, style)
            try:
                self.assertTrue(pool.intact())
                self.assertEqual(campaign.available(pool.reader), 2)
                self.assertEqual(pool.returned(), b'+*')
                self.assertEqual(pool.returned(), b'')
                pool.refill()
                self.assertEqual(pool.returned(), b'+*')
                if style == 'fifo':
                    pool.path.unlink()
                    pool.path.write_bytes(b'protected replacement')
                    self.assertFalse(pool.intact())
                    pool.path.unlink()
            finally:
                pool.close()

    def test_unstaged_research_retains_default_child_delivery_without_changing_acceptance(self):
        staged = campaign.Cell(signal.SIGINT, 'group')
        raw = dataclasses.replace(staged, unstaged=True)
        campaign.materialize(staged, self.root / 'staged')
        campaign.materialize(raw, self.root / 'raw')
        self.assertIn(b'signal.signal(number, record)', (self.root / 'staged/helper.py').read_bytes())
        self.assertIn(b'signal.signal(number, signal.SIG_DFL)', (self.root / 'raw/helper.py').read_bytes())
        self.assertEqual((self.root / 'staged/Makefile').read_bytes(), (self.root / 'raw/Makefile').read_bytes())

    def test_materialization_controls_coredump_policy_and_preserves_fixed_input_time(self):
        cell = campaign.Cell(signal.SIGQUIT, 'group', 2, kind='same-mtime')
        fixture = self.root / 'input'
        campaign.materialize(cell, fixture)
        self.assertEqual((fixture / 'out').stat().st_mtime_ns, 1700000000000000000)
        self.assertIn(b'prctl(4, 0, 0, 0, 0)', (fixture / 'helper.py').read_bytes())
        self.assertEqual((fixture / 'Makefile').read_bytes(), cell.source)

if __name__ == '__main__':
    unittest.main()
