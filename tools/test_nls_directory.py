# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import base64
import unittest

import conformance as h
import nls_directory as gate


class FifoCreationPidTests(unittest.TestCase):
    def test_only_the_recorded_pid_is_replaced(self):
        row = {"launch": "fifo-create-collision", "environment": {"MAKE_TMPDIR": "relative///"}}
        witness = {"type": "file", "mode": 0o644, "content": base64.b64encode(b"123\n").decode()}
        retained = {"type": "file", "mode": 0o644, "content": base64.b64encode(b"preserve").decode()}
        actual = h.Observation(
            b"stdout 123\n",
            b"make: jobserver mkfifo: relative////GMfifo123 : error123\n"
            b"make: jobserver mkfifo: relative////GMfifo1234 : untouched\n"
            b"make: another message relative////GMfifo123 : untouched\n",
            0, None,
            {
                gate.FIFO_PID_RECORD: witness,
                "work/shifted/relative/GMfifo123": retained,
                "work/shifted/relative/GMfifo124": retained,
            },
        )
        value, sample = gate.observe_fifo_create(actual, row)
        self.assertEqual(sample, {"pid": 123})
        self.assertEqual(value.stdout, actual.stdout)
        self.assertEqual(
            value.stderr,
            b"make: jobserver mkfifo: relative////GMfifo@FIFO_PID@ : error123\n"
            b"make: jobserver mkfifo: relative////GMfifo1234 : untouched\n"
            b"make: another message relative////GMfifo123 : untouched\n",
        )
        self.assertEqual(value.filesystem, {
            "work/shifted/relative/GMfifo@FIFO_PID@": retained,
            "work/shifted/relative/GMfifo124": retained,
        })

    def test_invalid_pid_witness_cannot_supply_a_substitution(self):
        row = {"launch": "fifo-create", "environment": {"MAKE_TMPDIR": "relative///"}}
        for content in (b"0\n", b"019\n", b"19\n20\n"):
            with self.subTest(content=content):
                entry = {"type": "file", "mode": 0o644, "content": base64.b64encode(content).decode()}
                actual = h.Observation(b"", b"", 0, None, {gate.FIFO_PID_RECORD: entry})
                with self.assertRaisesRegex(h.HarnessError, "invalid FIFO creation PID"):
                    gate.observe_fifo_create(actual, row)
