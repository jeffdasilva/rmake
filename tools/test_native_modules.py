# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Ensure native qualification cannot pass shared mistakes or ignored failures."""
import base64
from dataclasses import replace
import gzip
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import conformance as h
import native_modules as n


class NativeModuleTests(unittest.TestCase):
    def setUp(self):
        self.model = n.expression_model('unit', b'literal', b'literal')
        self.case = h.Case(Path('/unused'), self.model.id, 'native.modules', 'required',
                           self.model.args, 5, self.model.environment, self.model.expected)
        self.good = h.Observation(bytes.fromhex(self.model.expected['stdout_hex']), b'', 0, None,
                                  {'work/seen': {'type': 'file', 'content': base64.b64encode(self.model.seen).decode()}})

    def test_model_error_fails_even_when_every_engine_agrees(self):
        self.assertTrue(n.assess(self.model, self.case, [self.good] * 3)['passed'])
        for observation in (replace(self.good, stdout=b'wrong'), replace(self.good, stderr=b'wrong'),
                            replace(self.good, returncode=2)):
            result = n.assess(self.model, self.case, [observation] * 3)
            self.assertFalse(result['passed'])
            self.assertEqual(result['candidate_differences'], [])
            self.assertTrue(all(result['expected_errors']))

    def test_ordered_effects_fail_even_when_every_engine_agrees(self):
        model = replace(self.model, seen=b'first\nsecond\nthird\n')
        for wrong in (b'first\nthird\nsecond\n', b'first\nsecond\n', b'first\nsecond\nsecond\nthird\n'):
            observation = replace(self.good, filesystem={'work/seen': {'type': 'file', 'content': base64.b64encode(wrong).decode()}})
            result = n.assess(model, self.case, [observation] * 3)
            self.assertFalse(result['passed'])
            self.assertEqual(result['seen_matches'], [False] * 3)
        for entry in ({}, {'type': 'symlink', 'target': 'elsewhere'}, {'type': 'file', 'content': '!!'}):
            self.assertFalse(n.seen_matches(model, replace(self.good, filesystem={'work/seen': entry})))

    def test_repeat_candidate_and_resource_failures_are_strict(self):
        wrong = replace(self.good, filesystem={**self.good.filesystem, 'work/extra': {'type': 'file'}})
        self.assertFalse(n.assess(self.model, self.case, [self.good, wrong, self.good])['passed'])
        self.assertFalse(n.assess(self.model, self.case, [self.good, self.good, wrong])['passed'])
        for bad in (replace(self.good, limit='timeout'), replace(self.good, limit='output'),
                    replace(self.good, leftover_processes=True)):
            for index in range(3):
                observations = [self.good] * 3
                observations[index] = bad
                self.assertFalse(n.assess(self.model, self.case, observations)['passed'])
        self.assertTrue(n.assess(self.model, self.case, [self.good, self.good])['passed'])
        with self.assertRaises(h.HarnessError): n.assess(self.model, self.case, [self.good])

    def test_compile_once_then_execute_same_inputs_in_declared_order(self):
        for candidate, expected in ((Path('/candidate'), ['/oracle', '/oracle', '/candidate']), (None, ['/oracle', '/oracle'])):
            events = []
            def prepare(model, directory, compiler):
                events.append('compile')
                return self.case, [{'compiler': compiler}]
            def execute(case, root, binary):
                self.assertIs(case, self.case)
                self.assertEqual(root.name, 'runtime')
                events.append(str(binary))
                return self.good
            with tempfile.TemporaryDirectory() as directory, patch.object(n, 'prepare', side_effect=prepare), patch.object(n.h, 'execute', side_effect=execute):
                report = n.run_campaign([self.model], Path('/oracle'), candidate, '/compiler', Path(directory))
            self.assertEqual(events, ['compile', *expected])
            self.assertTrue(report['passed'])
            self.assertEqual(report['oracle_only'], candidate is None)
            if candidate is None: self.assertIn('zero conformance', report['classification'])

    def test_prepare_preserves_compiled_inputs_links_and_nanosecond_order(self):
        model = replace(self.model, mtimes_ns={'probe_api.so': 1_000_000_001},
                        symlinks={'aliases/probe_api.so': '../probe_api.so'})
        calls = []
        def compiler(command, cwd, environment, timeout):
            calls.append((command, cwd, environment, timeout))
            Path(command[-1]).write_bytes(b'original-compiled-fixture')
            return b'', b'', 0, None, False
        with tempfile.TemporaryDirectory() as temporary, patch.object(n.h, 'capture', side_effect=compiler):
            destination = Path(temporary) / 'inputs'
            case, records = n.prepare(model, destination, '/compiler with spaces')
            self.assertEqual((destination / 'Makefile').read_bytes(), model.files['Makefile'])
            self.assertEqual((destination / 'probe_api.so').stat().st_mtime_ns, 1_000_000_001)
            self.assertEqual((destination / 'aliases/probe_api.so').readlink(), Path('../probe_api.so'))
            self.assertEqual(case.path, destination)
            self.assertEqual(records[0]['module_sha256'], hashlib.sha256(b'original-compiled-fixture').hexdigest())
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0][0][0], '/compiler with spaces')
        self.assertEqual(calls[0][3], 15)
        with tempfile.TemporaryDirectory() as temporary, patch.object(n.h, 'capture', return_value=(b'', b'compiler error', 1, None, False)):
            with self.assertRaisesRegex(h.HarnessError, 'compilation failed'):
                n.prepare(self.model, Path(temporary) / 'inputs', '/compiler')
        for path in ('/absolute', '../outside', 'a/../../outside', 'nul\0name'):
            with self.assertRaises(h.HarnessError): n.safe_relative(path)

    def test_cli_returns_nonzero_for_failed_candidate_and_model_campaign(self):
        with tempfile.TemporaryDirectory() as temporary:
            report = Path(temporary) / 'report.json'
            for oracle_only in (False, True):
                arguments = ['--oracle', '/oracle', '--report', str(report)] + (['--oracle-only'] if oracle_only else ['--candidate', '/candidate'])
                with patch.object(n.shutil, 'which', return_value='/compiler'), patch.object(n.reference_oracle, 'verify'), patch.object(n.h, 'executable_identity', side_effect=lambda path, **_: {'path': str(path)}), patch.object(n, 'run_campaign', return_value={'passed': False, 'totals': {'cases': 1, 'pass': 0, 'fail': 1}}):
                    self.assertEqual(n.main(arguments), 1)
                self.assertFalse(json.loads(report.read_text())['passed'])
            with patch.object(n.shutil, 'which', return_value=None):
                self.assertEqual(n.main(['--oracle-only', '--report', str(report)]), 2)
            self.assertIn('compiler executable not found', json.loads(report.read_text())['error'])

    def test_original_inventory_covers_native_lifetime_and_fatal_boundaries(self):
        models = n.models()
        self.assertEqual(len(models), 246)
        self.assertEqual(len({m.id for m in models}), 246)
        self.assertEqual(sum(m.id.startswith('native-setup-result-') for m in models), 5)
        self.assertEqual(sum(m.id.startswith('native-loader-debug-') for m in models), 6)
        self.assertEqual(sum(m.id.startswith('native-directory-epoch-') for m in models), 5)
        self.assertEqual(sum(m.id.startswith('native-startup-') for m in models), 31)
        self.assertEqual(sum(m.id.startswith('native-stdio-') for m in models), 16)
        self.assertEqual(sum(m.id.startswith('native-registration-') for m in models), 41)
        self.assertEqual(sum(m.id.startswith('native-deep-') for m in models), 7)
        self.assertEqual(sum(m.id.startswith('native-target-read-') for m in models), 8)
        self.assertEqual(sum(m.id.startswith('native-secondary-scope-') for m in models), 4)
        self.assertEqual(sum(m.id.startswith('native-retained-double-branch-') for m in models), 2)
        self.assertEqual(sum(m.id.startswith('native-fatal-worker-') for m in models), 4)
        self.assertEqual(sum(m.id.startswith('native-destructor-api-') for m in models), 3)
        self.assertEqual({m.output for m in models}, {'separate', 'merged'})
        self.assertTrue(all(m.files['seen'] == b'anchor\n' for m in models))
        for source in n.SOURCES.glob('*.c'):
            self.assertNotIn('"subst"', source.read_text())
            self.assertNotIn('swap_a', source.read_text())
        for model in models:
            if model.id.startswith('native-fatal-worker-'):
                self.assertEqual(model.seen, b'anchor\nbefore\nworker-start\ncallback-entered\nworker-done\n')
                self.assertEqual(model.expected['exit_code'], 2)

    def test_startup_exit_witnesses_reject_lost_stale_or_premature_state(self):
        models = {model.id: model for model in n.startup_exit_models()}
        checks = (
            ('native-startup-gnumakeflags-help', lambda seen: seen.split(b'STATE ', 1)[0]),
            ('native-startup-restart-help', lambda seen: seen.replace(b'PASS=[second]:file', b'PASS=[first]:file')),
            ('native-startup-restart-bad-sync', lambda seen: seen.replace(b'RESTART=[1]:environment', b'RESTART=[]:undefined')),
            ('native-startup-cli-version', lambda seen: seen.replace(b'CURDIR=[empty]:undefined', b'CURDIR=[matches]:file')),
            ('native-startup-gnumakeflags-help-pending', lambda seen: seen.replace(b'PENDING=0', b'PENDING=14')),
        )
        for ident, corrupt in checks:
            model = models[ident]
            case = replace(self.case, id=ident, args=model.args, environment=model.environment,
                           expected=model.expected)
            good = h.Observation(bytes.fromhex(model.expected['stdout_hex']),
                                 bytes.fromhex(model.expected['stderr_hex']),
                                 model.expected['exit_code'], None,
                                 {'work/seen': {'type': 'file', 'content': base64.b64encode(model.seen).decode()}})
            self.assertTrue(n.assess(model, case, [good] * 3)['passed'], ident)
            wrong_seen = corrupt(model.seen)
            self.assertNotEqual(wrong_seen, model.seen, ident)
            wrong = replace(good, filesystem={'work/seen': {'type': 'file', 'content': base64.b64encode(wrong_seen).decode()}})
            result = n.assess(model, case, [wrong] * 3)
            self.assertFalse(result['passed'], ident)
            self.assertEqual(result['candidate_differences'], [], ident)
            self.assertEqual(result['expected_errors'], [[], [], []], ident)
            self.assertEqual(result['seen_matches'], [False, False, False], ident)

    def test_retained_initial_wrong_hypotheses_and_source_hashes(self):
        for stem in n.SOURCES.glob('*.json'):
            manifest = json.loads(stem.read_text())
            packed = Path(str(stem) + '.gz').read_bytes()
            self.assertEqual(hashlib.sha256(packed).hexdigest(), manifest['sha256'])
            raw = gzip.decompress(packed)
            self.assertEqual(hashlib.sha256(raw).hexdigest(), manifest['raw_sha256'])
        first = json.loads(gzip.decompress((n.SOURCES / 'first-candidate.json.gz').read_bytes()))
        first_report = json.loads(bytes.fromhex(first['report_hex']))
        self.assertEqual(first_report['totals'], {'cases': 55, 'pass': 53, 'fail': 2})
        link = first['source_archive']
        self.assertEqual(hashlib.sha256((n.SOURCES / link['path']).read_bytes()).hexdigest(), link['sha256'])
        stem = n.SOURCES / 'initial-models'
        manifest = json.loads(stem.with_suffix('.json').read_text())
        packed = stem.with_suffix('.json.gz').read_bytes()
        self.assertEqual(hashlib.sha256(packed).hexdigest(), manifest['sha256'])
        raw = gzip.decompress(packed)
        self.assertEqual(hashlib.sha256(raw).hexdigest(), manifest['raw_sha256'])
        history = json.loads(raw)
        report = json.loads(bytes.fromhex(history['report_hex']))
        self.assertEqual(report['totals'], {'cases': 49, 'pass': 47, 'fail': 2})
        self.assertEqual({r['id'] for r in report['records'] if not r['passed']}, {'native-missing-optional', 'native-missing-optional-then-required'})
        for value in history['sources'].values():
            self.assertEqual(hashlib.sha256(bytes.fromhex(value['hex'])).hexdigest(), value['sha256'])


if __name__ == '__main__': unittest.main()
