# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import unittest
from pathlib import Path
import subprocess
import tempfile
from unittest import mock

import allocations


class AllocationProfileTests(unittest.TestCase):
    def test_complete_summary_and_sampled_addresses_are_preserved(self):
        summary, samples = allocations.parse_trace('sample 512 32 0xabc 0x123\nsummary calls=600 requested_bytes=9999 frees=550\n')
        self.assertEqual(summary, {'calls': 600, 'requested_bytes': 9999, 'frees': 550})
        self.assertEqual(samples, [{'call': 512, 'bytes': 32, 'addresses': ['0xabc', '0x123']}])

    def test_incomplete_or_invalid_trace_is_not_reported_as_zero_allocations(self):
        for text in ('', 'sample 512 32\n', 'summary calls=-1 requested_bytes=0 frees=0\n',
                     'broken\nsummary calls=600 requested_bytes=1 frees=0\n',
                     'sample 601 1\nsummary calls=600 requested_bytes=1 frees=0\n'):
            with self.subTest(text=text), self.assertRaises(ValueError):
                allocations.parse_trace(text)

    def test_native_helper_failure_cannot_republish_a_previous_trace(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            (output / 'trace.txt').write_text('summary calls=123 requested_bytes=456 frees=100\n')
            identity = {'path': '/candidate', 'sha256': 'test'}
            compile_result = subprocess.CompletedProcess([], 0, b'', b'')
            execution_result = subprocess.CompletedProcess([], 0, b"make: Nothing to be done for 'all'.\n", b'')
            with mock.patch.object(allocations.conformance, 'executable_identity', return_value=identity), \
                 mock.patch.object(allocations.subprocess, 'run', side_effect=[compile_result, execution_result]), \
                 self.assertRaises(OSError):
                allocations.run(Path('/candidate'), 'tiny-noop', output)
            self.assertFalse((output / 'report.json').exists())

    def test_empty_process_and_missing_executable_frames_are_distinct(self):
        summary, samples = allocations.parse_trace('summary calls=0 requested_bytes=0 frees=0\n')
        self.assertEqual(summary['calls'], 0)
        self.assertEqual(samples, [])
        _, samples = allocations.parse_trace('sample 512 100\nsummary calls=512 requested_bytes=1000 frees=1\n')
        self.assertEqual(len(samples), 1)
        self.assertEqual(samples[0]['addresses'], [])
