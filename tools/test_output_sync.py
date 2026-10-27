# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Independent anchors must reject matching wrong output-sync implementations."""
import dataclasses
from pathlib import Path
import tempfile
import unittest
import conformance
import output_sync as campaign

class OutputSyncTests(unittest.TestCase):
    def test_inventory_covers_streams_modes_owned_mutex_and_both_mixed_directions(self):
        cells = campaign.inventory()
        self.assertEqual(len(cells), 76)
        self.assertEqual(len({c.id for c in cells}), len(cells))
        self.assertEqual({c.mode for c in cells}, set(campaign.MODES))
        self.assertEqual({c.child for c in cells}, {'', 'same', 'gnu', 'candidate'})
        self.assertEqual({c.owned for c in cells}, {True, False})

    def test_target_grouping_and_line_boundaries_have_distinct_exact_anchors(self):
        line = campaign.Cell('line', True)
        target = campaign.Cell('target', True)
        self.assertEqual(line.expected(), (b'parse-before\nparse-err\nparse-after\nexpand-a1\nexpand-a2\na1-out\na1-err\nexpand-b1\nexpand-b2\nb1-out\nb1-err\nb2-out\nb2-err\na2-out\na2-err\n', b''))
        out, err = target.expected()
        self.assertEqual(err, b'')
        self.assertLess(out.index(b'b2-err'), out.index(b'expand-a1'))
        self.assertNotEqual(line.expected(), target.expected())

    def test_initial_and_continuation_shell_configuration_have_separate_phases(self):
        out, err = campaign.Cell('target', True, True).expected()
        self.assertEqual(err, b'')
        self.assertLess(out.index(b'Makefile:9: config-a'), out.index(b'expand-b1'))
        self.assertLess(out.index(b'expand-a2'), out.index(b'Makefile:8: config-a'))
        self.assertLess(out.index(b'Makefile:12: config-b'), out.index(b'Makefile:11: config-b'))

    def test_anchors_reject_wrong_order_missing_duplicate_and_wrong_status(self):
        cell = campaign.Cell('target', False)
        out, err = cell.expected()
        correct = conformance.Observation(out, err, 0, None, {})
        self.assertEqual(campaign.anchors(cell, correct), [])
        for change in [dict(stdout=out[::-1]), dict(stdout=out+out), dict(stderr=b''),
                       dict(returncode=2), dict(limit='timeout'), dict(leftover_processes=True)]:
            self.assertTrue(campaign.anchors(cell, dataclasses.replace(correct, **change)))

    def test_recursion_preserves_grouping_instead_of_merely_all_lines(self):
        cell = campaign.Cell('recurse', True, child='gnu')
        self.assertEqual(cell.expected(), (b'b1-out\nb1-err\nb2-out\nb2-err\na1-out\na1-err\na2-out\na2-err\n', b''))
        wrong = conformance.Observation(campaign.Cell('none', True, child='gnu').expected()[0], b'', 0, None, {})
        self.assertTrue(campaign.anchors(cell, wrong))

    def test_large_output_validator_accepts_only_two_complete_target_orders(self):
        cell = campaign.Cell('target', True, child='gnu', stress=True)
        out, err = cell.expected()
        half = len(out) // 2
        for data in [out, out[half:] + out[:half]]:
            self.assertEqual(campaign.anchors(cell, conformance.Observation(data, err, 0, None, {})), [])
        for data in [out[:-1], out[:32768] + out[half:half+32768] + out[32768:half] + out[half+32768:], out+out]:
            self.assertTrue(campaign.anchors(cell, conformance.Observation(data, err, 0, None, {})))

    def test_materialization_preserves_original_protected_mutex_and_bounded_barriers(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / 'input'
            cell = campaign.Cell('line', False, child='candidate')
            campaign.materialize(cell, root)
            self.assertEqual((root / 'mutex').read_bytes(), campaign.PROTECTED)
            self.assertIn(b'BRANCH=a', (root / 'Makefile').read_bytes())
            self.assertIn(b'bounded output barrier', (root / 'helper.py').read_bytes())
            self.assertIn(b'$(BRANCH)2', (root / 'child.mk').read_bytes())

if __name__ == '__main__': unittest.main()
