# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import unittest
import contextlib
import io
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
            execution_result = subprocess.CompletedProcess([], 0, b"make: 'all' is up to date.\n", b'')
            with mock.patch.object(allocations.conformance, 'executable_identity', return_value=identity), \
                 mock.patch.object(allocations.subprocess, 'run', side_effect=[compile_result, execution_result]), \
                 self.assertRaises(OSError):
                allocations.run(Path('/candidate'), 'tiny-noop', output)
            self.assertFalse((output / 'report.json').exists())

    def test_each_workload_uses_its_exact_output_contract(self):
        expected = {'tiny-noop': b"make: 'all' is up to date.\n",
                    'expansion': b"make: 'all' is up to date.\n",
                    'large-noop': b"make: Nothing to be done for 'all'.\n"}
        for workload, stdout in expected.items():
            with self.subTest(workload=workload), tempfile.TemporaryDirectory() as temporary:
                output = Path(temporary)
                def command(argv, **kwargs):
                    if argv[0] == 'cc':
                        return subprocess.CompletedProcess(argv, 0, b'', b'')
                    Path(kwargs['env']['RMAKE_ALLOC_TRACE']).write_text(
                        'summary calls=0 requested_bytes=0 frees=0\n')
                    return subprocess.CompletedProcess(argv, 0, stdout, b'')
                with mock.patch.object(allocations.conformance, 'executable_identity',
                                       return_value={'path': '/candidate', 'sha256': 'test'}), \
                     mock.patch.object(allocations.subprocess, 'run', side_effect=command), \
                     contextlib.redirect_stdout(io.StringIO()):
                    report = allocations.run(Path('/candidate'), workload, output)
                self.assertEqual(report['workload'], workload)
                self.assertTrue((output / 'report.json').is_file())

    def test_startup_profiles_keep_required_arguments_environment_and_output_anchors(self):
        for workload in ('startup-arguments-1000','startup-arguments-5000','startup-effects-1000','initial-flags-1000', 'target-context-none','target-context-explicit','target-context-pattern'):
            case=allocations.conformance.load_case(allocations.ROOT/'tests/cases'/workload)
            expected=bytes.fromhex(case.expected['stdout_hex'])
            for broken in (False,True):
                with self.subTest(workload=workload,broken=broken), tempfile.TemporaryDirectory() as temporary:
                    output=Path(temporary)
                    def command(argv,**kwargs):
                        if argv[0]=='cc':return subprocess.CompletedProcess(argv,0,b'',b'')
                        self.assertEqual(tuple(argv[1:]),case.args)
                        for name,value in case.environment.items():self.assertEqual(kwargs['env'][name],value)
                        Path(kwargs['env']['RMAKE_ALLOC_TRACE']).write_text('summary calls=1 requested_bytes=2 frees=1\n')
                        return subprocess.CompletedProcess(argv,0,(b'WRONG\n' if workload.startswith('target-context-') else expected.replace(b'value500',b'WRONG')) if broken else expected,b'')
                    with mock.patch.object(allocations.conformance,'executable_identity',return_value={'path':'/candidate','sha256':'test'}), mock.patch.object(allocations.subprocess,'run',side_effect=command), contextlib.redirect_stdout(io.StringIO()):
                        if broken:
                            with self.assertRaises(ValueError):allocations.run(Path('/candidate'),workload,output)
                            self.assertFalse((output/'report.json').exists())
                        else:
                            report=allocations.run(Path('/candidate'),workload,output)
                            self.assertEqual(report['arguments'],list(case.args))
                            self.assertEqual(report['environment'],case.environment)

    def test_profiles_reject_arguments_that_diverge_from_the_required_fixture(self):
        for workload in ('startup-arguments-1000','target-context-none'):
            case=allocations.conformance.load_case(allocations.ROOT/'tests/cases'/workload)
            with self.subTest(workload=workload), tempfile.TemporaryDirectory() as temporary:
                output=Path(temporary)
                def command(argv,**kwargs):
                    if argv[0]=='cc':return subprocess.CompletedProcess(argv,0,b'',b'')
                    Path(kwargs['env']['RMAKE_ALLOC_TRACE']).write_text('summary calls=1 requested_bytes=2 frees=1\n')
                    return subprocess.CompletedProcess(argv,0,bytes.fromhex(case.expected['stdout_hex']),b'')
                with mock.patch.object(allocations.conformance,'executable_identity',return_value={'path':'/candidate','sha256':'test'}), mock.patch.object(allocations.benchmark,'workload_arguments',return_value=['-rR','WRONG=1']), mock.patch.object(allocations.subprocess,'run',side_effect=command):
                    with self.assertRaisesRegex(ValueError,'inputs differ'):
                        allocations.run(Path('/candidate'),workload,output)
                self.assertFalse((output/'report.json').exists())

    def test_empty_process_and_missing_executable_frames_are_distinct(self):
        summary, samples = allocations.parse_trace('summary calls=0 requested_bytes=0 frees=0\n')
        self.assertEqual(summary['calls'], 0)
        self.assertEqual(samples, [])
        _, samples = allocations.parse_trace('sample 512 100\nsummary calls=512 requested_bytes=1000 frees=1\n')
        self.assertEqual(len(samples), 1)
        self.assertEqual(samples[0]['addresses'], [])
