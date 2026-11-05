# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Independent negative controls for the numeric full-protocol gate."""
import base64
from dataclasses import replace
import gzip
import hashlib
import json
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch

import conformance as h
import database_numeric as numeric


class DatabaseNumericTests(unittest.TestCase):
    def setUp(self):
        self.model = numeric.models()[0]
        self.runtime = Path('/predetermined/reset')
        self.filesystem = {'work/seen': {'type': 'file', 'mode': 0o644,
                                        'content': base64.b64encode(self.model.seen).decode()}}
        self.metadata = {name: {'device': 42, 'inode': 99 + index, 'mode': stat.S_IFDIR | 0o755}
                         for index, name in enumerate(self.model.directories)}
        stdout = numeric.database.render_paths(self.model.stdout, self.runtime)
        for kind in ('OPEN', 'CLOSE'):
            stdout = stdout.replace(f'@CTIME_{kind}_0@'.encode(), b'Sun Sep 13 12:26:40 2020')
        for index, name in enumerate(self.model.directories):
            stdout = stdout.replace(f'@DEVICE_{index}@'.encode(), str(self.metadata[name]['device']).encode())
            stdout = stdout.replace(f'@INODE_{index}@'.encode(), str(self.metadata[name]['inode']).encode())
        self.good = numeric.database.Execution(
            h.Observation(stdout, self.model.stderr, self.model.expected_status, None, self.filesystem),
            (1600000000.1, 1600000000.9), self.metadata, self.metadata, (1600000000, 1600000000))
        self.case = h.Case(Path('/unused'), self.model.id, 'native.fenv', 'required',
                           self.model.args, 5, self.model.environment, {})

    def assess(self, executions):
        return numeric.database.assess(self.model, self.runtime, self.filesystem, executions)

    def test_inventory_keeps_all_modes_and_registration_counters_literal(self):
        models = numeric.models()
        self.assertEqual(len(models), 6)
        self.assertEqual({model.id.rsplit('-', 1)[1] for model in models}, {'nearest', 'upward', 'downward'})
        self.assertEqual(len({model.id for model in models}), 6)
        for model in models:
            self.assertIn(b'load ./fenv.so\n', model.files['Makefile'])
            self.assertIn(b'$(fp_get )', model.files['Makefile'])
            self.assertIn(b'query:', model.seen)
            self.assertIn(b'/ strings = 23 / storage = 255 B' if '-minimal-' in model.id
                          else b'/ strings = 374 / storage = 4297 B', model.stdout)
        up = next(model for model in models if '-builtins-upward' in model.id)
        self.assertIn(b'5 (5.5%) terminal.', up.stdout)
        with patch.object(numeric.h, 'sha256_file', return_value='changed'):
            with self.assertRaisesRegex(h.HarnessError, 'source identity changed'):
                numeric.models()

    def test_shared_wrong_numeric_spelling_or_registration_counts_cannot_pass(self):
        self.assertTrue(self.assess([self.good] * 3)['passed'])
        for before, after in ((b'Load=41/1024=4%', b'Load=41/1024=5%'),
                              (b'Collisions=0/19=0%', b'Collisions=0/19=-0%'),
                              (b'/ strings = 23 / storage = 255 B', b'/ strings = 12 / storage = 143 B'),
                              (b'lookups = 28 /', b'lookups = 17 /')):
            self.assertIn(before, self.good.observation.stdout)
            wrong = replace(self.good, observation=replace(self.good.observation,
                            stdout=self.good.observation.stdout.replace(before, after)))
            report = self.assess([wrong] * 3)
            self.assertFalse(report['passed'])
            self.assertEqual(h.differences(wrong.observation, wrong.observation), [])
            self.assertTrue(all(report['expected_errors']))

    def test_independent_witness_and_complete_filesystem_reject_shared_wrong_effects(self):
        changed = {**self.filesystem, 'work/seen': {**self.filesystem['work/seen'],
                   'content': base64.b64encode(self.model.seen[::-1]).decode()}}
        for filesystem in (changed, {**self.filesystem, 'work/extra': {'type': 'file'}}, {}):
            wrong = replace(self.good, observation=replace(self.good.observation, filesystem=filesystem))
            self.assertFalse(self.assess([wrong] * 3)['passed'])

    def test_streams_status_clocks_identities_repeats_and_cleanup_are_strict(self):
        for observation in (replace(self.good.observation, stderr=b'wrong'),
                            replace(self.good.observation, returncode=2),
                            replace(self.good.observation, limit='timeout'),
                            replace(self.good.observation, limit='output'),
                            replace(self.good.observation, leftover_processes=True)):
            for index in range(3):
                runs = [self.good] * 3; runs[index] = replace(self.good, observation=observation)
                self.assertFalse(self.assess(runs)['passed'])
        for bad in (replace(self.good, libc_time_interval=(1600000001, 1600000001)),
                    replace(self.good, directories_after={}),
                    replace(self.good, directories_before={})):
            self.assertFalse(self.assess([bad] * 3)['passed'])
        with self.assertRaises(h.HarnessError): self.assess([self.good])

    def test_original_module_compiles_once_with_libm_and_fixed_input_times(self):
        commands = []
        def compile_fixture(command, *_args):
            commands.append(command)
            Path(command[command.index('-o') + 1]).write_bytes(b'compiled-original')
            return b'', b'', 0, None, False
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(numeric.fenv.h, 'capture', side_effect=compile_fixture), \
             patch.object(numeric.fenv, 'expected_filesystem', return_value=self.filesystem):
            case, compilation, filesystem = numeric.prepare(self.model, Path(temporary) / 'input',
                                                             self.runtime, Path('/oracle'), '/compiler with spaces')
            self.assertEqual((case.path / 'module-0.c').read_bytes(), numeric.fenv.SOURCE.read_bytes())
            self.assertTrue(all(path.stat().st_mtime_ns == self.model.fixed_mtime_ns
                                for path in case.path.rglob('*') if path.is_file()))
            self.assertEqual(compilation['module_sha256'], hashlib.sha256(b'compiled-original').hexdigest())
            self.assertEqual(filesystem, self.filesystem)
        self.assertEqual(len(commands), 1)
        self.assertEqual(commands[0][0], '/compiler with spaces')
        self.assertEqual(commands[0][-1], '-lm')

    def test_campaign_compiles_before_gnu2_and_candidate_resets(self):
        for candidate, expected in ((Path('/candidate'), ['compile', '/oracle', '/oracle', '/candidate']),
                                    (None, ['compile', '/oracle', '/oracle'])):
            events = []
            def prepare(*_args):
                events.append('compile'); return self.case, {'once': True}, self.filesystem
            def execute(*args, **kwargs):
                events.append(str(args[-1]))
                return replace(self.good, observation=replace(self.good.observation,
                    stdout=self.good.observation.stdout.replace(bytes(self.runtime), bytes(args[2]))))
            with tempfile.TemporaryDirectory() as temporary, \
                 patch.object(numeric, 'prepare', side_effect=prepare), \
                 patch.object(numeric.database, 'execute', side_effect=execute):
                report = numeric.run_campaign([self.model], Path('/oracle'), candidate, '/compiler', Path(temporary))
            self.assertTrue(report['passed'])
            self.assertEqual(events, expected)
            self.assertEqual(report['oracle_only'], candidate is None)

    def test_failed_candidate_and_unknown_selection_return_nonzero(self):
        with tempfile.TemporaryDirectory() as temporary:
            report = Path(temporary) / 'report.json'
            args = ['--oracle', '/oracle', '--candidate', '/candidate', '--report', str(report)]
            with patch.object(numeric.shutil, 'which', return_value='/compiler'), \
                 patch.object(numeric.reference_oracle, 'verify'), \
                 patch.object(numeric.database, 'expected_preparation'), \
                 patch.object(numeric.h, 'executable_identity', side_effect=lambda path, **_: {'path': str(path)}), \
                 patch.object(numeric, 'run_campaign', return_value={'passed': False, 'totals': {'fail': 1}}):
                self.assertEqual(numeric.main(args), 1)
                self.assertFalse(json.loads(report.read_bytes())['passed'])
            self.assertEqual(numeric.main([*args, '--case', 'unknown']), 2)
            self.assertIn('unknown database numeric cases', json.loads(report.read_bytes())['error'])

    def test_retained_raw_history_and_every_embedded_file_are_checksum_protected(self):
        path = numeric.ROOT / 'tests/database-output/numeric-history.json'
        manifest = json.loads(path.read_bytes())
        blob = path.with_suffix('.json.gz').read_bytes()
        self.assertEqual(hashlib.sha256(blob).hexdigest(), manifest['sha256'])
        raw = gzip.decompress(blob)
        self.assertEqual(hashlib.sha256(raw).hexdigest(), manifest['raw_sha256'])
        self.assertEqual(len(raw), manifest['raw_bytes'])
        data = json.loads(raw)
        for file in data['files'].values():
            self.assertEqual(hashlib.sha256(bytes.fromhex(file['content_hex'])).hexdigest(), file['sha256'])
        self.assertIn('first-observations.json', data['files'])
        self.assertIn('qualified-observations.json', data['files'])
        self.assertIn('fixed-observations.json', data['files'])
        self.assertIn('fixed-source.json.gz', data['files'])


if __name__ == '__main__':
    unittest.main()
