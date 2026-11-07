# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import remake_parallel


class PollWaitTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.proc = Path(directory.name)
        architecture = patch.object(remake_parallel, "X86_64", True)
        architecture.start()
        self.addCleanup(architecture.stop)

    def sample(self, number, timeout):
        (self.proc / "syscall").write_text(
            f"{number} 0x10 0x2 {timeout} 0x0 0x0 0x0 0x100 0x200\n"
        )
        return remake_parallel.poll_wait(self.proc)

    def test_blocking_poll_timeout_uses_the_signed_int_width(self):
        self.assertEqual(self.sample(7, "0xffffffff"), (7, True))
        self.assertEqual(self.sample(7, "0x5"), (7, True))
        self.assertEqual(self.sample(7, "0x100000000"), (7, False))

    def test_indefinite_ppoll_and_other_syscalls(self):
        self.assertEqual(self.sample(271, "0x0"), (271, True))
        self.assertEqual(self.sample(271, "0x20"), (271, False))
        self.assertEqual(self.sample(0, "0xffffffff"), (0, False))

    def pselect(self, channel, *, state="S", nfds="0x8", descriptors="0x20", timeout="0x70"):
        (self.proc / "syscall").write_text(
            f"270 {nfds} {descriptors} 0x0 0x0 {timeout} 0x90 0x100 0x200\n"
        )
        (self.proc / "stat").write_bytes(f"42 (make child)) {state} 12 12\n".encode())
        return remake_parallel.poll_wait(self.proc, channel)

    def test_finite_pselect_requires_a_verified_sleep(self):
        channel = "poll_schedule_timeout.constprop.0"
        self.assertEqual(self.pselect(channel), (270, True))
        self.assertEqual(self.pselect("poll_schedule_timeout"), (270, True))
        self.assertEqual(self.pselect("0"), (270, False))
        self.assertEqual(self.pselect(channel, state="R"), (270, False))
        self.assertEqual(self.pselect(channel, state="D"), (270, False))

    def test_finite_pselect_requires_descriptors_and_a_timespec(self):
        channel = "poll_schedule_timeout.constprop.0"
        self.assertEqual(self.pselect(channel, nfds="0x0"), (270, False))
        self.assertEqual(self.pselect(channel, descriptors="0x0"), (270, False))
        self.assertEqual(self.pselect(channel, timeout="0x0"), (270, False))
        (self.proc / "stat").write_bytes(b"malformed\n")
        self.assertEqual(remake_parallel.poll_wait(self.proc, channel), (270, False))

    def test_exited_running_or_unavailable_process(self):
        self.assertEqual(remake_parallel.poll_wait(self.proc), (None, False))
        for value in ("running\n", "-1 0x100 0x200\n", "7 0 2 invalid 0 0 0 0 0\n"):
            (self.proc / "syscall").write_text(value)
            self.assertEqual(remake_parallel.poll_wait(self.proc), (None, False))

    def test_unqualified_architecture_does_not_use_x86_numbers(self):
        self.sample(7, "0xffffffff")
        with patch.object(remake_parallel, "X86_64", False):
            self.assertEqual(remake_parallel.poll_wait(self.proc), (None, False))
