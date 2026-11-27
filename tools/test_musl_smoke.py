# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
from contextlib import redirect_stderr
from dataclasses import replace
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import musl_smoke as smoke


class MuslNativeModelsTests(unittest.TestCase):
    def test_pruned_reload_models_have_separate_musl_lifetimes(self):
        previous = (
            'native-setup-result-negative-one',
            'native-setup-result-negative-one-optional-explicit-and-repeat',
        )
        pruned = ('phony', 'double-colon', 'admitted-control', 'mixed')
        models = smoke.native_smoke_models()
        self.assertEqual([model.id for model in models],
                         [*previous, *('native-pruned-remake-reload-' + name for name in pruned)])
        shared = {model.id: model for model in smoke.native_modules.models()}
        for model in models:
            with self.subTest(model=model.id):
                original = shared[model.id]
                if model.id in previous:
                    self.assertEqual(model, original)
                    continue
                self.assertNotEqual(model.seen, original.seen)
                self.assertEqual(model, replace(original, seen=model.seen))
                before_goal, finalizers = model.seen.split(b'goal\n')
                self.assertEqual(before_goal.count(b':open\n'), len(model.modules))
                self.assertEqual(before_goal.count(b':setup\n'),
                                 len(model.modules) + model.id.endswith(('admitted-control', 'mixed')))
                self.assertNotIn(b':close\n', before_goal)
                self.assertEqual(finalizers, b'a:close\nb:close\n' if model.id.endswith('mixed') else b'a:close\n')


class MuslProjectReportsTests(unittest.TestCase):
    def test_requested_projects_share_verified_pair_and_contribute_to_acceptance(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / 'oracle').mkdir()
            reference = root / 'oracle/bin/make'
            candidate = root / 'target/release/rmake'
            reports = root / 'reports'
            xxhash = reports / 'xxhash.json'
            lua = reports / 'lua.json'
            summary = reports / 'profile.json'
            variables = {'MAKE_HOST': smoke.MAKE_HOST, '.INCLUDE_DIRS': ''}
            profile = {'observed': {'variables': variables}}

            with (
                patch.object(smoke, 'build_sysroot', return_value=(root / 'loader', root / 'cc', {})),
                patch.object(smoke, 'build_ucontext', return_value=(root / 'context', {})),
                patch.object(smoke, 'build_reference', return_value=reference),
                patch.object(smoke, 'build_candidate', return_value=candidate),
                patch.object(smoke, 'observe_runtime', return_value={}),
                patch.object(smoke, 'cases', side_effect=lambda *_: {'passed': True}),
                patch.object(smoke.oracle, 'clean_environment', return_value={}),
                patch.object(smoke.oracle, 'load_config', return_value={}),
                patch.object(smoke.oracle, 'checked_run', return_value='tool'),
                patch.object(smoke.oracle, 'create_profile', return_value=profile),
                patch.object(smoke.oracle, 'verify') as verify,
                patch.object(smoke.oracle, 'probe_variables', return_value=variables),
                patch.object(smoke.oracle, 'sha256_file', return_value='rmake-digest'),
                patch.object(smoke.integration, 'main', side_effect=(0, 1, 0)) as integration,
            ):
                self.assertFalse(smoke.run(root, summary, 'revision', xxhash_report=xxhash,
                                           lua_report=lua))
                first_case, second_case = integration.call_args_list
                for call, name, destination in (
                    (first_case, 'xxhash-0.8.3', xxhash),
                    (second_case, 'lua-5.4.9', lua),
                ):
                    self.assertEqual(call.args[0], [
                        '--case', name, '--oracle', str(reference), '--candidate', str(candidate),
                        '--cache', str(root / 'integration-cache'), '--report', str(destination),
                    ])
                    self.assertIs(call.kwargs['verify_reference'].func, verify)
                self.assertIs(first_case.kwargs['verify_reference'],
                              second_case.kwargs['verify_reference'])
                self.assertEqual(json.loads(summary.read_text())['cases'], {
                    'passed': False,
                    'xxhash': {'report': str(xxhash), 'passed': True},
                    'lua': {'report': str(lua), 'passed': False},
                })

                self.assertTrue(smoke.run(root, summary, 'revision', xxhash_report=xxhash))
                self.assertEqual(json.loads(summary.read_text())['cases'], {
                    'passed': True,
                    'xxhash': {'report': str(xxhash), 'passed': True},
                })
                self.assertEqual(integration.call_count, 3)

    def test_cli_routes_lua_to_a_distinct_report(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            summary = root / 'profile.json'
            xxhash = root / 'xxhash.json'
            lua = root / 'lua.json'
            argv = ['musl_smoke.py', '--report', str(summary), '--xxhash-report', str(xxhash)]
            with (
                patch.object(smoke.platform, 'system', return_value='Linux'),
                patch.object(smoke.platform, 'machine', return_value='x86_64'),
                patch.object(smoke, 'run', return_value=True) as run,
            ):
                with patch.object(sys, 'argv', [*argv, '--lua-report', str(lua)]):
                    self.assertEqual(smoke.main(), 0)
                self.assertEqual(run.call_args.kwargs, {'xxhash_report': xxhash, 'lua_report': lua})

                for conflict in (summary, xxhash):
                    with patch.object(sys, 'argv', [*argv, '--lua-report', str(conflict)]), \
                         redirect_stderr(io.StringIO()) as stderr, \
                         self.assertRaises(SystemExit) as error:
                        smoke.main()
                    self.assertEqual(error.exception.code, 2)
                    self.assertIn('--lua-report must differ', stderr.getvalue())
                self.assertEqual(run.call_count, 1)


if __name__ == '__main__':
    unittest.main()
