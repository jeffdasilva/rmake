# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import unittest

from guile_detached_parallel import wait_kind


class ParallelWaitWitnessTests(unittest.TestCase):
    def test_wait4_rejects_nonblocking_option(self):
        prefix = '61 0xffffffff 0x0 '
        suffix = ' 0x0 0x0 0x0 0x0 0x0'
        self.assertEqual(wait_kind(prefix + '0x0' + suffix, 'fifo', [3], True), 'wait4')
        self.assertIsNone(wait_kind(prefix + '0x1' + suffix, 'fifo', [3], True))

    def test_poll_rejects_zero_timeout_and_unrelated_syscalls(self):
        prefix = '7 0x1 0x4 '
        suffix = ' 0x0 0x0 0x0 0x0 0x0'
        self.assertEqual(wait_kind(prefix + '0xffffffff' + suffix, 'pipe', [3], False), 'poll')
        self.assertIsNone(wait_kind(prefix + '0x0' + suffix, 'pipe', [3], False))
        self.assertIsNone(wait_kind('7 0x1 0x0 0xffffffff' + suffix, 'pipe', [3], False))
        self.assertIsNone(wait_kind('running', 'pipe', [3], False))

    def test_token_read_requires_actual_live_reader(self):
        arguments = ' 0x1 0x1 0x0 0x0 0x0 0x0 0x0'
        self.assertEqual(wait_kind('0 0x3' + arguments, 'pipe', [3], True), 'pipe-token-read')
        self.assertIsNone(wait_kind('0 0x4' + arguments, 'pipe', [3], True))


if __name__ == '__main__':
    unittest.main()
