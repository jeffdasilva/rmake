# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import unittest
from unittest import mock

import conformance as h
import locale_collation as c


class LocaleCollationTests(unittest.TestCase):
    def test_inventory_keeps_complete_growth_live_thread_and_archive_controls(self):
        _, rows = c.models()
        self.assertEqual(len(rows), 74)
        names = {row['id'] for row in rows}
        self.assertTrue({'raw-growth-120', 'raw-growth-240', 'raw-growth-500',
                         'live-thread-fr-fr-utf-8', 'archive-members-fr-fr-utf-8',
                         'include-fr-fr-utf-8', 'raw-live-c'} <= names)
        self.assertTrue(all('LOCPATH' not in row['environment'] for row in rows))

    def test_comparator_rejects_wrong_tie_order_each_stream_and_unexpected_files(self):
        _, rows = c.models()
        model = next(row for row in rows if row['id'] == 'raw-invalid-fr-fr-utf-8')
        initial = {'work': {'type': 'directory', 'mode': 0o755}}
        good = h.Observation(bytes.fromhex(model['stdout_hex']), b'', 0, None, initial, False)
        self.assertEqual(c.assess(model, good, initial), [])
        changes = [dict(stdout=good.stdout.replace(b'f\xff f\x80', b'f\x80 f\xff')),
                   dict(stderr=b'extra\n'), dict(returncode=2), dict(limit='timeout'),
                   dict(leftover_processes=True), dict(filesystem={**initial, 'tmp/leak': {'type': 'file'}})]
        for change in changes:
            observation = h.Observation(**{**good.__dict__, **change})
            self.assertTrue(c.assess(model, observation, initial), change)

    def test_relative_paths_reject_traversal_absolute_and_nul(self):
        for name in [b'', b'/file', b'../file', b'a/../file', b'a\0b']:
            with self.assertRaises(h.HarnessError):
                c.relative_name(name)
        self.assertEqual(c.relative_name(b'raw/\xff'), b'raw/\xff')

    def test_archive_members_remain_raw_short_names(self):
        result = c.archive([b'item', b'f\xff'])
        self.assertEqual(len(result), 8 + 2 * 60)
        self.assertEqual(result[:8], b'!<arch>\n')
        self.assertIn(b'f\xff/', result)
        for name in [b'', b'way-too-long-member', b'a/b', b'a\0b', b'a\nb']:
            with self.assertRaises(h.HarnessError):
                c.archive([name])

    def test_unknown_case_fails_before_provisioning_or_child_execution(self):
        with mock.patch.object(c.locale_data, 'provision') as provision:
            with self.assertRaises(SystemExit) as error:
                c.main(['--case', 'absent', '--report', '/unused/report.json'])
            self.assertEqual(error.exception.code, 2)
            provision.assert_not_called()


if __name__ == '__main__':
    unittest.main()
