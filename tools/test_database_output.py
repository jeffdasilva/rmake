# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Negative controls for strict database snapshots and typed GNU nondeterminism."""
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
import database_output as d


class DatabaseOutputTests(unittest.TestCase):
    def setUp(self):
        self.runtime = Path('/predetermined/reset')
        self.model = d.models()[0]
        self.filesystem = {'work/seen': {'type': 'file', 'mode': 0o644,
                                        'content': base64.b64encode(self.model.seen).decode()}}
        self.metadata = {name: {'device': 42, 'inode': 99 + index, 'mode': stat.S_IFDIR | 0o755}
                         for index, name in enumerate(self.model.directories)}
        stdout = d.render_paths(self.model.stdout, self.runtime)
        stdout = stdout.replace(b'@CTIME_OPEN@', b'Sun Sep 13 12:26:40 2020').replace(b'@CTIME_CLOSE@', b'Sun Sep 13 12:26:40 2020')
        for index, name in enumerate(self.model.directories):
            stdout = stdout.replace(f'@DEVICE_{index}@'.encode(), str(self.metadata[name]['device']).encode())
            stdout = stdout.replace(f'@INODE_{index}@'.encode(), str(self.metadata[name]['inode']).encode())
        self.good = d.Execution(h.Observation(stdout, self.model.stderr, self.model.expected_status, None, self.filesystem),
                                (1600000000.1, 1600000000.9), self.metadata, self.metadata, (1600000000, 1600000000))

    def assess(self, executions):
        return d.assess(self.model, self.runtime, self.filesystem, executions)

    def altered_stdout(self, content):
        return replace(self.good, observation=replace(self.good.observation, stdout=content))

    def test_multiple_dumps_keep_every_literal_byte_and_all_clock_pairs(self):
        def clock_index(template, index):
            return template.replace(b'@CTIME_OPEN@', f'@CTIME_OPEN_{index}@'.encode()).replace(
                b'@CTIME_CLOSE@', f'@CTIME_CLOSE_{index}@'.encode())
        model = replace(self.model, dump_count=2,
                        stdout=clock_index(self.model.stdout, 0) + clock_index(self.model.stdout, 1))
        good = self.altered_stdout(self.good.observation.stdout * 2)
        self.assertTrue(d.assess(model, self.runtime, self.filesystem, [good] * 3)['passed'])
        for wrong in (self.good.observation.stdout, self.good.observation.stdout * 3,
                      good.observation.stdout.replace(b'Collisions=1/72=1%', b'Collisions=0/72=0%', 1),
                      good.observation.stdout[::-1]):
            observed = self.altered_stdout(wrong)
            self.assertFalse(d.assess(model, self.runtime, self.filesystem, [observed] * 3)['passed'])
        with self.assertRaises(h.HarnessError):
            d.compile_template(replace(model, stdout=model.stdout.replace(b'@CTIME_CLOSE_1@', b'@CTIME_CLOSE_0@')), self.runtime)

    def test_additive_phase_models_keep_original_inventory_and_native_sources_explicit(self):
        self.assertEqual(len(d.models()), 14)
        phases = d.models('phases')
        self.assertEqual(len(phases), 19)
        self.assertEqual(len(d.models('all')), 44)
        self.assertEqual({m.dump_count for m in phases}, {1, 2})
        self.assertEqual(sum(bool(m.compile_shared) for m in phases), 2)
        self.assertTrue(all(source in model.files for model in phases for source, _ in model.compile_shared))

    def test_search_models_pin_full_names_cache_storage_and_goal_flags(self):
        search = d.models('search')
        self.assertEqual(len(search), 8)
        self.assertEqual(len({model.id for model in d.models('all')}), 44)
        self.assertTrue(all(not model.compile_shared for model in search))
        model = next(model for model in search if model.id == 'database-search-repeated-aliased')
        metadata = {name: {'device': 42, 'inode': 99 + index, 'mode': stat.S_IFDIR | 0o755}
                    for index, name in enumerate(model.directories)}
        stdout = d.render_paths(model.stdout, self.runtime)
        stdout = stdout.replace(b'@CTIME_OPEN@', b'Sun Sep 13 12:26:40 2020').replace(
            b'@CTIME_CLOSE@', b'Sun Sep 13 12:26:40 2020')
        for index, name in enumerate(model.directories):
            stdout = stdout.replace(f'@DEVICE_{index}@'.encode(), str(metadata[name]['device']).encode())
            stdout = stdout.replace(f'@INODE_{index}@'.encode(), str(metadata[name]['inode']).encode())
        good = replace(self.good, observation=replace(self.good.observation, stdout=stdout, stderr=model.stderr,
                       returncode=model.expected_status), directories_before=metadata, directories_after=metadata)
        self.assertFalse(d.stream_errors(model, self.runtime, good))
        for old, new in ((b'search/input:', b'input:'),
                         (b'#  Command line target.\n', b''),
                         (b'# files hash-table stats:', b'# wrong files hash-table stats:'),
                         (b'# strcache performance:', b'# wrong strcache performance:'),
                         (b'storage = 165 B', b'storage = 164 B'),
                         (b'Collisions=0/34=0%', b'Collisions=0/35=0%')):
            wrong = stdout.replace(old, new)
            self.assertNotEqual(stdout, wrong)
            execution = replace(good, observation=replace(good.observation, stdout=wrong))
            self.assertTrue(d.stream_errors(model, self.runtime, execution))

    def test_library_models_require_fixed_input_mtimes_without_a_seen_fixture(self):
        models = d.models('library')
        self.assertEqual({model.id for model in models}, {
            'library-vpath-shelf', 'library-vpath-alternate-pattern', 'library-current-directory-control',
        })
        for model in models:
            self.assertTrue(model.fixed_input_mtimes)
            with tempfile.TemporaryDirectory() as tmp:
                case, filesystem = d.prepare(model, Path(tmp) / 'inputs', self.runtime)
                self.assertNotIn('work/seen', filesystem)
                self.assertEqual({name: (case.path / name).stat().st_mtime_ns for name in model.files},
                                 dict.fromkeys(model.files, model.fixed_mtime_ns))
        model = replace(self.model, fixed_input_mtimes=True)
        exact = dict.fromkeys(model.files, model.fixed_mtime_ns)
        good = replace(self.good, input_mtimes=(exact, exact))
        self.assertFalse(d.stream_errors(model, self.runtime, good))
        changed = {**exact, 'Makefile': model.fixed_mtime_ns + 1}
        self.assertIn('fixed input file mtimes', d.stream_errors(
            model, self.runtime, replace(good, input_mtimes=(exact, changed))))

    def test_native_fixture_compiles_once_before_engine_runs_and_full_snapshot_contains_it(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            model = replace(self.model, files={'Makefile': b'all:;\n', 'probe.c': b'/* original */\n'},
                            compile_shared=(('probe.c', 'probe.so'),))
            def compile_fixture(command, *_args):
                Path(command[-1]).write_bytes(b'original-compiled-fixture')
                return b'', b'', 0, None, False
            records = []
            with patch.object(d.shutil, 'which', return_value='/compiler'), patch.object(d.h, 'capture', side_effect=compile_fixture) as capture:
                case, filesystem = d.prepare(model, root / 'inputs', self.runtime, records)
            self.assertEqual(capture.call_count, 1)
            self.assertEqual((case.path / 'probe.so').stat().st_mtime_ns, model.fixed_mtime_ns)
            self.assertEqual(records[0]['output_sha256'], filesystem['work/probe.so']['sha256'])
            self.assertEqual(base64.b64decode(filesystem['work/probe.so']['content']), b'original-compiled-fixture')

    def test_declared_input_removal_changes_only_expected_full_filesystem(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            model = replace(self.model, files={'Makefile': b'all:;\n', 'obsolete': b'old'}, removed_files=('obsolete',))
            case, filesystem = d.prepare(model, root / 'inputs', self.runtime)
            self.assertTrue((case.path / 'obsolete').exists())
            self.assertNotIn('work/obsolete', filesystem)
            for removed in ('absent', '../outside', 'Makefile/child'):
                with self.assertRaises(h.HarnessError):
                    d.prepare(replace(model, removed_files=(removed,)), root / removed.replace('/', '-'), self.runtime)

    def test_full_literal_protocol_rejects_shared_wrong_stats_values_order_and_paths(self):
        self.assertTrue(self.assess([self.good] * 3)['passed'])
        stdout = self.good.observation.stdout
        wrong = (stdout.replace(b'Load=40/1024=4%', b'Load=41/1024=4%'),
                 stdout.replace(b'Collisions=1/72=1%', b'Collisions=0/72=0%'),
                 stdout.replace(b'lookups = 13 / hit rate = 23%', b'lookups = 12 / hit rate = 23%'),
                 stdout.replace(b'MAKE_VERSION := 4.4.1', b'MAKE_VERSION := 4.4.2'),
                 stdout.replace(b'/predetermined/reset', b'/output/derived/root'),
                 stdout.replace(b'#  Last modified 2020-09-13 12:26:40.123456789', b'#  Last modified 2020-09-13 12:26:41.123456789'),
                 stdout.replace(b'# Directories\n', b'# Files\n'), stdout + b'extra\n')
        for content in wrong:
            self.assertNotEqual(content, stdout)
            result = self.assess([self.altered_stdout(content)] * 3)
            self.assertFalse(result['passed'])
            self.assertTrue(all(result['expected_errors']))
            self.assertIn('independent-literal-or-relational-model', result['candidate_differences'])

    def test_only_declared_physical_identity_slots_may_vary_and_require_independent_stat(self):
        meta = {name: {**value, 'device': 500, 'inode': 700 + index} for index, (name, value) in enumerate(self.metadata.items())}
        stdout = self.good.observation.stdout
        for index, name in enumerate(self.model.directories):
            stdout = stdout.replace(f'(device 42, inode {99 + index})'.encode(), f'(device 500, inode {700 + index})'.encode())
        correct = replace(self.altered_stdout(stdout), directories_before=meta, directories_after=meta)
        self.assertTrue(self.assess([self.good, correct, correct])['passed'])
        for wrong in (self.altered_stdout(stdout), replace(correct, directories_before=self.metadata),
                      replace(correct, directories_after={}), replace(correct, directories_before={})):
            result = self.assess([wrong] * 3)
            self.assertFalse(result['passed']); self.assertTrue(all(result['expected_errors']))
        # Arbitrary identity-looking user text is literal, not a semantic slot.
        extra = b'# user text (device 42, inode 99):\n'
        model = replace(self.model, stdout=self.model.stdout + extra)
        good = self.altered_stdout(self.good.observation.stdout + extra)
        self.assertTrue(d.assess(model, self.runtime, self.filesystem, [good] * 3)['passed'])
        wrong = self.altered_stdout(self.good.observation.stdout + extra.replace(b'inode 99', b'inode 100'))
        self.assertFalse(d.assess(model, self.runtime, self.filesystem, [wrong] * 3)['passed'])

    def test_clock_fields_require_exact_calendar_padding_interval_and_order(self):
        self.assertEqual(d.parse_clock(b'Sun Sep 13 12:26:40 2020'), 1600000000)
        for value in (b'Mon Sep 13 12:26:40 2020', b'Sun Sep 13 25:26:40 2020',
                      b'Sun Sep 03 12:26:40 2020', b'Sun Sep  3 12:26:40 2020', b'2020-09-13 12:26:40'):
            with self.assertRaises(ValueError): d.parse_clock(value)
        wrong_clock = self.good.observation.stdout.replace(b'Sun Sep 13 12:26:40 2020', b'Sun Sep 13 12:26:41 2020')
        self.assertFalse(self.assess([self.altered_stdout(wrong_clock)] * 3)['passed'])
        malformed = self.good.observation.stdout.replace(b'Sun Sep 13 12:26:40 2020', b'Mon Sep 13 12:26:40 2020')
        self.assertFalse(self.assess([self.altered_stdout(malformed)] * 3)['passed'])
        early = self.good.observation.stdout.replace(b'printed on Sun Sep 13 12:26:40 2020', b'printed on Sun Sep 13 12:26:41 2020')
        wrong = replace(self.altered_stdout(early), interval=(1600000000, 1600000002), libc_time_interval=(1600000000, 1600000002))
        self.assertFalse(self.assess([wrong] * 3)['passed'])
        duplicate = self.good.observation.stdout + b'# Finished Make data base on Sun Sep 13 12:26:40 2020\n'
        self.assertFalse(self.assess([self.altered_stdout(duplicate)] * 3)['passed'])

    def test_clock_sampling_uses_same_libc_domain_without_widening_tolerance(self):
        coarse = replace(self.good, interval=(1600000001.001, 1600000001.004),
                         libc_time_interval=(1600000000, 1600000000))
        self.assertTrue(self.assess([coarse] * 3)['passed'])
        next_second = coarse.observation.stdout.replace(b'Sun Sep 13 12:26:40 2020', b'Sun Sep 13 12:26:41 2020')
        wrong = replace(coarse, observation=replace(coarse.observation, stdout=next_second))
        self.assertFalse(self.assess([wrong] * 3)['passed'])
        self.assertFalse(self.assess([replace(coarse, libc_time_interval=None)] * 3)['passed'])
        with patch.object(d.LIBC, 'time', return_value=123):
            self.assertEqual(d.libc_time(), 123)
        with patch.object(d.LIBC, 'time', return_value=-1):
            with self.assertRaises(h.HarnessError): d.libc_time()

    def test_ordered_seen_complete_filesystem_and_modes_are_independent(self):
        for filesystem in ({}, {**self.filesystem, 'work/unexpected': {'type': 'file'}},
                           {'work/seen': {**self.filesystem['work/seen'], 'mode': 0o755}},
                           {'work/seen': {**self.filesystem['work/seen'], 'content': base64.b64encode(b'READ\nanchor\n').decode()}}):
            wrong = replace(self.good, observation=replace(self.good.observation, filesystem=filesystem))
            result = self.assess([wrong] * 3)
            self.assertFalse(result['passed']); self.assertEqual(result['filesystem_errors'], [True] * 3)

    def test_repeat_status_stderr_and_bounds_are_strict(self):
        for observation in (replace(self.good.observation, returncode=2), replace(self.good.observation, stderr=b'extra'),
                            replace(self.good.observation, limit='timeout'), replace(self.good.observation, leftover_processes=True)):
            for index in range(3):
                records = [self.good] * 3
                records[index] = replace(self.good, observation=observation)
                self.assertFalse(self.assess(records)['passed'])
        self.assertTrue(self.assess([self.good] * 2)['passed'])
        with self.assertRaises(h.HarnessError): self.assess([self.good])

    def test_typed_template_rejects_unknown_duplicate_undeclared_and_moved_slots(self):
        for token in (b'@VERSION@', b'@HELP@', b'@UNKNOWN@', b'@INODE_99@'):
            with self.assertRaises(h.HarnessError): d.compile_template(replace(self.model, stdout=self.model.stdout + token), self.runtime)
        for token in (b'@CTIME_OPEN@', b'@DEVICE_0@'):
            with self.assertRaises(h.HarnessError): d.compile_template(replace(self.model, stdout=self.model.stdout + token), self.runtime)
        with self.assertRaises(h.HarnessError): d.compile_template(replace(self.model, stdout=self.model.stdout.replace(b'@CTIME_CLOSE@', b'literal')), self.runtime)
        for name in ('', '/absolute', '../outside', 'a/../../outside', 'nul\0name', '.'):
            with self.assertRaises(h.HarnessError): d.safe_relative(name)
        self.assertEqual(d.safe_relative('.', dot=True), Path('.'))

    def test_preparation_preserves_static_nanoseconds_and_prohibits_compiler_effects(self):
        with tempfile.TemporaryDirectory() as tmp, patch.object(d.h, 'capture', side_effect=AssertionError('no compiler')):
            case, filesystem = d.prepare(self.model, Path(tmp) / 'inputs', self.runtime)
            self.assertEqual((case.path / 'Makefile').stat().st_mtime_ns, self.model.fixed_mtime_ns)
            self.assertEqual(base64.b64decode(filesystem['work/seen']['content']), self.model.seen)
            self.assertEqual(filesystem['home'], {'type': 'directory', 'mode': 0o755})

    def test_campaign_executes_gnu_twice_and_candidate_at_the_same_declared_paths(self):
        for candidate, expected in ((Path('/candidate'), ['/oracle', '/oracle', '/candidate']), (None, ['/oracle', '/oracle'])):
            order = []
            def execute(model, case, runtime, binary, *, expected=None):
                self.assertIs(model, self.model); self.assertEqual(runtime.name, 'runtime')
                order.append(str(binary)); return self.good
            with tempfile.TemporaryDirectory() as tmp, patch.object(d, 'prepare', return_value=(h.Case(Path('/unused'), self.model.id, 'database.output', 'required', self.model.args, 8, self.model.environment, {}), self.filesystem)), patch.object(d, 'execute', side_effect=execute), patch.object(d, 'assess', return_value={'passed': True}):
                report = d.run_campaign([self.model], Path('/oracle'), candidate, Path(tmp))
            self.assertEqual(order, expected); self.assertEqual(report['oracle_only'], candidate is None)

    def test_cli_failing_model_and_candidate_always_return_nonzero_and_report(self):
        with tempfile.TemporaryDirectory() as tmp:
            report = Path(tmp) / 'report.json'
            for oracle_only in (True, False):
                args = ['--oracle', '/oracle', '--report', str(report), '--case', self.model.id] + (['--oracle-only'] if oracle_only else ['--candidate', '/candidate'])
                with patch.object(d, 'models', return_value=[self.model]), patch.object(d.reference_oracle, 'verify'), patch.object(d, 'expected_preparation'), patch.object(d.h, 'sha256_file', return_value='hash'), patch.object(d.h, 'executable_identity', side_effect=lambda path, **kwargs: {'path': str(path)}), patch.object(d, 'run_campaign', return_value={'passed': False, 'totals': {'cases': 1, 'pass': 0, 'fail': 1}}):
                    self.assertEqual(d.main(args), 1)
                self.assertFalse(json.loads(report.read_text())['passed'])
            with patch.object(d, 'models', return_value=[self.model]):
                self.assertEqual(d.main(['--oracle-only', '--case', 'database-min', '--report', str(report)]), 2)

    def test_inventory_is_bounded_and_retained_archives_are_checksummed(self):
        models = d.models()
        self.assertEqual(len(models), 14); self.assertEqual(len({model.id for model in models}), 14)
        self.assertTrue(all('-p' in model.args or '-qp' in model.args for model in models))
        self.assertTrue(all(model.stdout.count(b'@CTIME_OPEN@') == 1 and model.stdout.count(b'@CTIME_CLOSE@') == 1 for model in models))
        self.assertEqual(next(model for model in models if model.id == 'database-read-fatal').stderr, b'Makefile:4: *** DB_STOP.  Stop.\n')
        for path in d.SOURCES.glob('*.json'):
            if path.name in (*d.JSON_SUITES.values(), 'numeric.json', 'expected-inputs.json'):
                continue
            manifest = json.loads(path.read_bytes()); packed = path.with_suffix('.json.gz').read_bytes()
            raw = gzip.decompress(packed)
            self.assertEqual(hashlib.sha256(packed).hexdigest(), manifest['sha256'])
            self.assertEqual(hashlib.sha256(raw).hexdigest(), manifest['raw_sha256'])
            self.assertEqual(len(raw), manifest['raw_bytes'])


if __name__ == '__main__': unittest.main()
