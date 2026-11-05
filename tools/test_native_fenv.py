# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Negative controls for exact native floating-point state qualification."""
import base64
from dataclasses import replace
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import conformance as h
import native_fenv as f


class NativeFenvTests(unittest.TestCase):
    def setUp(self):
        self.model = f.models()[0]
        self.case = h.Case(Path('/unused'), self.model.id, 'native.fenv', 'required',
                           self.model.args, 5, self.model.environment, self.model.expected)
        self.filesystem = {'work/seen': {'type': 'file', 'mode': 0o644,
                                         'content': base64.b64encode(self.model.seen).decode()},
                           'work/module-0.c': {'type': 'file', 'mode': 0o644, 'content': 'original-source'},
                           'work/fenv.so': {'type': 'file', 'mode': 0o755, 'content': 'original-module'}}
        self.good = h.Observation(bytes.fromhex(self.model.expected['stdout_hex']), b'', 0, None,
                                  self.filesystem)

    def assess(self, observations):
        return f.assess(self.model, self.case, self.filesystem, observations)

    def test_independent_modes_and_witnesses_reject_shared_wrong_results(self):
        self.assertTrue(self.assess([self.good] * 3)['passed'])
        wrong_fs = {**self.filesystem, 'work/seen': {**self.filesystem['work/seen'],
                    'content': base64.b64encode(self.model.seen.replace(b'query:nearest', b'query:upward')).decode()}}
        wrong = replace(self.good, stdout=b'OUT=upward\n', filesystem=wrong_fs)
        report = self.assess([wrong] * 3)
        self.assertFalse(report['passed'])
        self.assertEqual(report['candidate_differences'], [])
        self.assertTrue(all(report['expected_errors']))
        self.assertEqual(report['seen_matches'], [False] * 3)

    def test_independent_complete_filesystem_rejects_shared_undeclared_changes(self):
        for filesystem in ({**self.filesystem, 'work/extra': {'type': 'file'}},
                           {**self.filesystem, 'work/module-0.c': {'type': 'file', 'content': 'changed'}},
                           {**self.filesystem, 'work/fenv.so': {'type': 'file', 'mode': 0o644}},
                           {'work/seen': self.filesystem['work/seen']}):
            report = self.assess([replace(self.good, filesystem=filesystem)] * 3)
            self.assertFalse(report['passed'])
            self.assertEqual(report['filesystem_matches'], [False] * 3)

    def test_every_stream_status_repeat_and_resource_bound_is_mandatory(self):
        for wrong in (replace(self.good, stdout=b'wrong'), replace(self.good, stderr=b'wrong'),
                      replace(self.good, returncode=2), replace(self.good, limit='timeout'),
                      replace(self.good, limit='output'), replace(self.good, leftover_processes=True)):
            for index in range(3):
                observations = [self.good] * 3; observations[index] = wrong
                self.assertFalse(self.assess(observations)['passed'])
        self.assertTrue(self.assess([self.good] * 2)['passed'])
        with self.assertRaises(h.HarnessError): self.assess([self.good])

    def test_original_inventory_keeps_rounding_nested_restore_flags_and_finalization(self):
        models = f.models()
        self.assertEqual(len(models), 18)
        self.assertEqual(len({model.id for model in models}), 18)
        self.assertTrue(all(model.id.startswith('native-fenv-') for model in models))
        by_id = {model.id: model for model in models}
        for mode in ('upward', 'downward', 'towardzero'):
            self.assertEqual(bytes.fromhex(by_id['native-fenv-successive-' + mode].expected['stdout_hex']),
                             f'OUT={mode}|{mode}\n'.encode())
        self.assertIn(b'final-return:upward', by_id['native-fenv-finalizer'].seen)
        self.assertIn(b'saved-return:nearest', by_id['native-fenv-saved-expand'].seen)
        self.assertIn(b'flag-divbyzero=1', by_id['native-fenv-successive-exception'].seen)
        self.assertTrue(all(b'$(fp_get)' not in model.files['Makefile'] for model in models))

    def test_compiler_runs_once_with_explicit_libm_and_preserves_inputs(self):
        commands = []
        def compiler(command, directory, environment, timeout):
            commands.append((command, environment, timeout))
            Path(command[command.index('-o') + 1]).write_bytes(b'original-compiled-module')
            return b'', b'', 0, None, False
        with tempfile.TemporaryDirectory() as temporary, patch.object(f.h, 'capture', side_effect=compiler):
            case, compilation = f.prepare(self.model, Path(temporary) / 'input', '/compiler with spaces')
            self.assertEqual((case.path / 'Makefile').read_bytes(), self.model.files['Makefile'])
            self.assertEqual((case.path / 'module-0.c').read_bytes(), f.SOURCE.read_bytes())
            self.assertEqual(compilation['module_sha256'], hashlib.sha256(b'original-compiled-module').hexdigest())
        self.assertEqual(len(commands), 1)
        self.assertEqual(commands[0][0][0], '/compiler with spaces')
        self.assertEqual(commands[0][0][-1], '-lm')
        self.assertEqual(commands[0][2], 15)
        with tempfile.TemporaryDirectory() as temporary, patch.object(f.h, 'capture', return_value=(b'', b'failure', 1, None, False)):
            with self.assertRaisesRegex(h.HarnessError, 'compilation failed'):
                f.prepare(self.model, Path(temporary) / 'input', '/compiler')

    def test_campaign_prepares_once_then_executes_gnu_gnu_candidate_in_order(self):
        for candidate, engines in ((Path('/candidate'), ['/oracle', '/oracle', '/candidate']),
                                   (None, ['/oracle', '/oracle'])):
            events = []
            def prepare(*_args):
                events.append('compile'); return self.case, {'original': True}
            def execute(_case, _runtime, binary):
                events.append(str(binary)); return self.good
            with tempfile.TemporaryDirectory() as temporary, \
                 patch.object(f, 'prepare', side_effect=prepare), \
                 patch.object(f, 'expected_filesystem', return_value=self.filesystem), \
                 patch.object(f.h, 'execute', side_effect=execute):
                report = f.run_campaign([self.model], Path('/oracle'), candidate, '/compiler', Path(temporary))
            self.assertEqual(events, ['compile', *engines])
            self.assertTrue(report['passed'])
            self.assertEqual(report['oracle_only'], candidate is None)

    def test_failed_candidate_or_oracle_qualification_always_exits_nonzero(self):
        with tempfile.TemporaryDirectory() as temporary:
            report = Path(temporary) / 'report.json'
            for oracle_only in (False, True):
                args = ['--oracle', '/oracle', '--report', str(report)] + (
                    ['--oracle-only'] if oracle_only else ['--candidate', '/candidate'])
                with patch.object(f.shutil, 'which', return_value='/compiler'), \
                     patch.object(f.reference_oracle, 'verify'), \
                     patch.object(f.h, 'executable_identity', side_effect=lambda path, **_: {'path': str(path)}), \
                     patch.object(f, 'run_campaign', return_value={'passed': False, 'totals': {'cases': 1, 'pass': 0, 'fail': 1}}):
                    self.assertEqual(f.main(args), 1)
                self.assertFalse(json.loads(report.read_bytes())['passed'])

    def test_unknown_exact_case_and_missing_compiler_fail_before_campaign(self):
        with tempfile.TemporaryDirectory() as temporary:
            report = Path(temporary) / 'report.json'
            with patch.object(f, 'run_campaign') as campaign:
                self.assertEqual(f.main(['--oracle-only', '--case', 'native-fenv-unknown', '--report', str(report)]), 2)
                campaign.assert_not_called()
            self.assertIn('unknown native fenv cases', json.loads(report.read_bytes())['error'])
            with patch.object(f.shutil, 'which', return_value=None):
                self.assertEqual(f.main(['--oracle-only', '--report', str(report)]), 2)
            self.assertIn('compiler executable not found', json.loads(report.read_bytes())['error'])


if __name__ == '__main__': unittest.main()
