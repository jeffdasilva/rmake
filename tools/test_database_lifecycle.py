# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Negative controls for the original lifecycle protocol and witness gate."""
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
import database_lifecycle as lifecycle
import database_output as database


class DatabaseLifecycleTests(unittest.TestCase):
    runtime = Path('/predetermined/reset')

    def execution(self, model):
        stdout = database.render_paths(model.stdout, self.runtime)
        for name in database.clock_fields(model):
            stdout = stdout.replace(f'@{name}@'.encode(), b'Sun Sep 13 12:26:40 2020')
        filesystem = {'work/seen': {'type': 'file', 'mode': 0o644,
                                    'content': base64.b64encode(model.seen).decode()}}
        return database.Execution(h.Observation(stdout, model.stderr, model.expected_status, None, filesystem),
                                  (1600000000.1, 1600000000.9), {}, {}, (1600000000, 1600000000))

    def assess(self, model, good, altered):
        return database.assess(model, self.runtime, good.observation.filesystem, [altered] * 3)

    def test_generic_named_sources_are_separate_from_adopted_stdin_and_old_inventory(self):
        generic = lifecycle.models('generic'); stdin = lifecycle.models('stdin')
        self.assertEqual((len(generic), len(stdin), len(lifecycle.models())), (6, 8, 14))
        self.assertTrue(all('--temp-stdin=owned.mk' not in model.args for model in generic))
        self.assertTrue(all('--temp-stdin=owned.mk' in model.args for model in stdin))
        self.assertEqual(sum(bool(model.compile_shared) for model in generic), 5)
        self.assertEqual(sum(bool(model.compile_shared) for model in stdin), 4)
        self.assertEqual(len(database.models('all')), 44)

    def test_matching_wrong_source_flags_and_cache_counters_cannot_pass(self):
        model = next(model for model in lifecycle.models('stdin') if model.id.endswith('-normal'))
        good = self.execution(model)
        self.assertTrue(self.assess(model, good, good)['passed'])
        stdout = good.observation.stdout
        wrong_flags = stdout.replace(b'owned.mk:\n#  Implicit rule search has not been done.',
                                     b'owned.mk:\n#  Implicit rule search has been done.\n#  File is secondary (prerequisite of .SECONDARY).')
        wrong_counter = stdout.replace(b'# strcache performance:', b'# wrong strcache performance:')
        for value in (wrong_flags, wrong_counter):
            self.assertNotEqual(stdout, value)
            observed = replace(good, observation=replace(good.observation, stdout=value))
            self.assertFalse(self.assess(model, good, observed)['passed'])

    def test_empty_fraction_environment_order_and_lost_cleanup_are_literal(self):
        model = next(model for model in lifecycle.models('generic') if 'whole-second' in model.id)
        good = self.execution(model)
        wrong = good.observation.stdout.replace(b'#  Last modified 2020-09-13 12:26:42\n',
                                               b'#  Last modified 2020-09-13 12:26:42.000000000\n')
        self.assertNotEqual(wrong, good.observation.stdout)
        self.assertFalse(self.assess(model, good, replace(good, observation=replace(good.observation, stdout=wrong)))['passed'])
        model = next(model for model in lifecycle.models('generic') if 'vector-order' in model.id)
        good = self.execution(model)
        self.assertTrue(self.assess(model, good, good)['passed'])
        wrong_seen = model.seen.replace(b'|PATH|HOME|TMPDIR', b'|HOME|PATH|TMPDIR')
        wrong_fs = {'work/seen': {**good.observation.filesystem['work/seen'],
                                 'content': base64.b64encode(wrong_seen).decode()}}
        observed = replace(good, observation=replace(good.observation, filesystem=wrong_fs))
        self.assertFalse(self.assess(model, good, observed)['passed'])
        model = next(model for model in lifecycle.models('stdin') if model.id.endswith('-normal'))
        good = self.execution(model)
        wrong_fs = {**good.observation.filesystem, 'work/owned.mk': {'type': 'file'}}
        observed = replace(good, observation=replace(good.observation, filesystem=wrong_fs))
        self.assertFalse(self.assess(model, good, observed)['passed'])

    def test_failed_candidate_and_unknown_case_fail_closed_with_a_report(self):
        identity = {'oracle': {'path': '/oracle'}, 'candidate': {'path': '/candidate'}}
        with tempfile.TemporaryDirectory() as temporary:
            report = Path(temporary) / 'failed.json'
            with patch.object(lifecycle.reference_oracle, 'verify'), \
                 patch.object(lifecycle.database, 'expected_preparation'), \
                 patch.object(h, 'executable_identity', side_effect=lambda _path, oracle=False: identity['oracle' if oracle else 'candidate']), \
                 patch.object(lifecycle, 'run_campaign', return_value={'passed': False, 'totals': {'pass': 0, 'fail': 14}}):
                status = lifecycle.main(['--candidate', '/candidate', '--report', str(report)])
            self.assertEqual(status, 1)
            self.assertFalse(json.loads(report.read_bytes())['passed'])
            unknown = Path(temporary) / 'unknown.json'
            self.assertEqual(lifecycle.main(['--candidate', '/candidate', '--case', 'missing', '--report', str(unknown)]), 2)
            self.assertIn('unknown database lifecycle cases', json.loads(unknown.read_bytes())['error'])

    def test_retained_complete_evidence_has_outer_and_per_file_hashes(self):
        directory = lifecycle.MODELS.parent
        for path in directory.glob('*.json'):
            if path in (lifecycle.MODELS, lifecycle.ERROR_MODELS, lifecycle.PIPE_MODELS, lifecycle.PIPE_REFINEMENT):
                continue
            manifest = json.loads(path.read_bytes())
            packed = path.with_suffix('.json.gz').read_bytes()
            raw = gzip.decompress(packed)
            self.assertEqual(hashlib.sha256(packed).hexdigest(), manifest['sha256'])
            self.assertEqual(hashlib.sha256(raw).hexdigest(), manifest['raw_sha256'])
            self.assertEqual(len(raw), manifest['raw_bytes'])
            for record in json.loads(raw)['files'].values():
                content = bytes.fromhex(record['hex'])
                self.assertEqual(hashlib.sha256(content).hexdigest(), record['sha256'])

    def test_exec_failure_keeps_status_path_cleanup_and_finalizer_absence_strict(self):
        model = next(model for model in lifecycle.models('stdin') if model.id.endswith('-exec-failure'))
        model = lifecycle.expected_model(model, self.runtime)
        good = self.execution(model)
        self.assertTrue(self.assess(model, good, good)['passed'])
        self.assertIn(b'/predetermined/reset/work/./missing-exec', model.stderr)
        wrong_seen = model.seen + b'FINI=removed\n'
        wrong_fs = {**good.observation.filesystem,
                    'work/seen': {**good.observation.filesystem['work/seen'],
                                  'content': base64.b64encode(wrong_seen).decode()}}
        for wrong in (replace(good.observation, returncode=2),
                      replace(good.observation, stderr=b'missing-exec: *** Stop.\n'),
                      replace(good.observation, stderr=model.stderr.replace(b'/work/./', b'/work/')),
                      replace(good.observation, filesystem=wrong_fs),
                      replace(good.observation, filesystem={**good.observation.filesystem,
                                                           'work/owned.mk': {'type': 'file'}})):
            self.assertFalse(self.assess(model, good, replace(good, observation=wrong))['passed'])

    def test_pipe_cleanup_witness_rejects_exposed_or_unbound_auth_descriptors(self):
        model = next(model for model in lifecycle.models('stdin') if 'pipe-exec-failure' in model.id)
        model = lifecycle.expected_model(model, self.runtime)
        good = self.execution(model)
        self.assertTrue(self.assess(model, good, good)['passed'])
        for replacement in (b'CLOEXEC_BAD=2', b'AUTH-MISSING', b'AUTH-POPULATION-ERROR', b'ADVERTISED-MISSING'):
            witness = model.seen.replace(b'CLOEXEC_BAD=0', replacement)
            self.assertNotEqual(witness, model.seen)
            filesystem = {'work/seen': {**good.observation.filesystem['work/seen'],
                                        'content': base64.b64encode(witness).decode()}}
            wrong = replace(good, observation=replace(good.observation, filesystem=filesystem))
            self.assertFalse(self.assess(model, good, wrong)['passed'])


    def test_refinement_cannot_change_the_sealed_status_or_advertised_pair_witness(self):
        actual = json.loads(lifecycle.PIPE_REFINEMENT.read_bytes())
        model = next(model for model in lifecycle.models('stdin') if 'pipe-exec-failure' in model.id)
        self.assertIn(b'ADVERTISED-MISSING', model.files['unlink_pipe.c'])
        actual['models'][0]['expected_status'] = 2
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'refinement.json'
            path.write_text(json.dumps(actual))
            with patch.object(lifecycle, 'PIPE_REFINEMENT', path):
                with self.assertRaisesRegex(h.HarnessError, 'only change observer inputs'):
                    lifecycle.models('stdin')


if __name__ == '__main__':
    unittest.main()
