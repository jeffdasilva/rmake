# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Keep comparison methodology and frozen workload identities reviewable."""

import hashlib
import unittest

import benchmark
import conformance


class BenchmarkTests(unittest.TestCase):
    def test_builtin_transition_workloads_reach_cleanup_with_unused_graph_nodes(self):
        workloads = list(benchmark.builtin_transition_workloads())
        self.assertEqual(workloads, list(benchmark.selected_workloads('builtin-transitions')))
        for (name, source), count in zip(workloads, (0, 1000, 10000)):
            self.assertEqual(benchmark.workload_arguments(name), ['--no-print-directory'])
            self.assertEqual(source.count(b'unused'), count)
            self.assertEqual(source.count(b'MAKEFLAGS += -R'), 1)
            self.assertIn(b'before=$(origin CC)/$(origin .LIBPATTERNS)', source)
            self.assertIn(b'after=$(origin CC)/$(origin .LIBPATTERNS)', source)

    def test_builtin_cleanup_timing_cannot_pass_with_startup_defaults_disabled(self):
        import dataclasses
        expected = b'before=default/default\nafter=undefined/undefined/default/0\n'
        for name, source in benchmark.builtin_transition_workloads():
            case = conformance.load_case(benchmark.ROOT / 'tests/cases' / name)
            self.assertEqual(case.args, ('--no-print-directory',))
            self.assertEqual((case.path / 'Makefile').read_bytes(), source)
            self.assertEqual(bytes.fromhex(case.expected['stdout_hex']), expected)
            good = conformance.Observation(expected, b'', 0, None, {})
            self.assertEqual(conformance.expectation_errors(case, good), [])
            for stdout in (expected.replace(b'before=default/default', b'before=undefined/undefined'),
                           expected.replace(b'after=undefined/undefined', b'after=default/default')):
                self.assertTrue(conformance.expectation_errors(case, dataclasses.replace(good, stdout=stdout)))

    def test_eval_workloads_observe_every_buffer_with_exact_case_anchors(self):
        import dataclasses
        workloads = list(benchmark.eval_workloads())
        self.assertEqual(workloads, list(benchmark.selected_workloads('eval-arguments')))
        for (name, source), count in zip(workloads, (0, 1000)):
            case = conformance.load_case(benchmark.ROOT / 'tests/cases' / name)
            self.assertEqual((case.path / 'Makefile').read_bytes(), source)
            self.assertEqual(case.args, tuple(benchmark.workload_arguments(name)))
            values = b'|'.join(f'value{index}'.encode() for index in range(count))
            out = b'values=[' + values + b"]\nmake: 'all' is up to date.\n"
            good = conformance.Observation(out, b'', 0, None, {})
            self.assertEqual(conformance.expectation_errors(case, good), [])
            self.assertEqual(source.count(b'$(EV_'), count)
            self.assertEqual(len(case.args), count + 2)
            if count:
                wrong = out.replace(b'value500', b'WRONG')
                self.assertTrue(conformance.expectation_errors(case, dataclasses.replace(good, stdout=wrong)))

    def test_startup_effect_workloads_observe_all_eager_side_bindings(self):
        import dataclasses
        workloads = list(benchmark.startup_effect_workloads())
        self.assertEqual(workloads, list(benchmark.selected_workloads('startup-effects')))
        for (name, source), count in zip(workloads, (0, 1000)):
            case = conformance.load_case(benchmark.ROOT / 'tests/cases' / name)
            self.assertEqual((case.path / 'Makefile').read_bytes(), source)
            self.assertEqual(case.args, tuple(benchmark.workload_arguments(name)))
            self.assertEqual(source.count(b'$(SIDE_'), count)
            self.assertEqual(len(case.args), count + 2)
            out = b'values=[' + b'|'.join(f'value{i}'.encode() for i in range(count)) + b"]\nmake: 'all' is up to date.\n"
            good = conformance.Observation(out, b'', 0, None, {})
            self.assertEqual(conformance.expectation_errors(case, good), [])
            if count:
                self.assertIn('CHECK_500:=$(eval SIDE_500:=value500)', case.args)
                self.assertTrue(conformance.expectation_errors(case, dataclasses.replace(good, stdout=out.replace(b'value500', b'WRONG'))))

    def test_initial_flag_workloads_observe_every_effect_and_raw_dollar(self):
        import dataclasses
        for (name, source), count in zip(benchmark.initial_flag_workloads(), (0, 1000)):
            case = conformance.load_case(benchmark.ROOT / 'tests/cases' / name)
            self.assertEqual((case.path / 'Makefile').read_bytes(), source)
            self.assertEqual(case.environment, benchmark.workload_environment(name))
            self.assertEqual(case.args, tuple(benchmark.workload_arguments(name)))
            self.assertEqual(source.count(b'$(IV_'), count)
            out = b'sentinel=[$]\nvalues=[' + b'|'.join(f'value{i}'.encode() for i in range(count)) + b"]\nmake: 'all' is up to date.\n"
            good = conformance.Observation(out, b'', 0, None, {})
            self.assertEqual(conformance.expectation_errors(case, good), [])
            self.assertTrue(conformance.expectation_errors(case, dataclasses.replace(good, stdout=out.replace(b'sentinel=[$]', b'sentinel=[]'))))
            if count:
                self.assertTrue(conformance.expectation_errors(case, dataclasses.replace(good, stdout=out.replace(b'value500', b'WRONG'))))

    def test_pair_order_alternates_without_changing_sample_membership(self):
        self.assertEqual(benchmark.paired_order(0, True), ["oracle", "candidate"])
        self.assertEqual(benchmark.paired_order(1, True), ["candidate", "oracle"])
        self.assertEqual(benchmark.paired_order(1, False), ["oracle"])
        self.assertEqual(benchmark.paired_order(0, True, True), ["oracle", "baseline", "candidate"])
        self.assertEqual(benchmark.paired_order(1, True, True), ["candidate", "baseline", "oracle"])

    def test_startup_workloads_observe_every_argument_without_recipe_processes(self):
        workloads = list(benchmark.startup_workloads())
        self.assertEqual(workloads, list(benchmark.selected_workloads('startup-arguments')))
        for (name, source), count in zip(workloads, (0, 1000, 5000)):
            arguments = benchmark.workload_arguments(name)
            self.assertEqual(arguments[:2], ['-rR', '--no-print-directory'])
            self.assertEqual(len(arguments), count + 2)
            self.assertEqual(source.count(b'$(ARG_'), count)
            self.assertNotIn(b'$(shell', source)
            self.assertNotIn(b'\t', source)
            if count:
                self.assertIn(f'$(ARG_{count - 1})'.encode(), source)
                self.assertEqual(arguments[-1], f'ARG_{count - 1}=value{count - 1}')

    def test_large_argv_has_an_exact_oracle_anchor_for_middle_values(self):
        import dataclasses
        case = conformance.load_case(benchmark.ROOT / 'tests/cases/startup-arguments-5000')
        expected = bytes.fromhex(case.expected['stdout_hex'])
        self.assertIn(b'|value2500|', expected)
        self.assertEqual(case.args, tuple(benchmark.workload_arguments(case.id)))
        sources = dict(benchmark.startup_workloads())
        self.assertEqual((case.path / 'Makefile').read_bytes(), sources[case.id])
        good = conformance.Observation(expected, b'', 0, None, {})
        self.assertEqual(conformance.expectation_errors(case, good), [])
        self.assertTrue(conformance.expectation_errors(case, dataclasses.replace(
            good, stdout=expected.replace(b'|value2500|', b'|omitted|'))))

    def test_workloads_are_stable_and_cover_distinct_costs(self):
        first = list(benchmark.workloads())
        self.assertEqual(first, list(benchmark.workloads()))
        self.assertEqual([name for name, _ in first], ["tiny-noop", "large-noop", "expansion"])
        self.assertEqual(len({hashlib.sha256(source).digest() for _, source in first}), 3)
        self.assertEqual(first[1][1].count(b": ;"), 10000)
        self.assertEqual(first[2][1].count(b"$(subst"), 5000)

    def test_filename_suite_keeps_raw_blanks_and_needs_no_recipe_processes(self):
        import tempfile
        from pathlib import Path
        workloads = list(benchmark.rule_filename_workloads())
        self.assertEqual(workloads, list(benchmark.selected_workloads('rule-filenames')))
        self.assertEqual([name for name, _ in workloads], [
            'rule-filenames-quoted-2000', 'rule-filenames-glob-1000',
            'rule-filenames-secondary-1000', 'rule-filenames-implicit-500'])
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name, source in workloads:
                fixture = benchmark.prepare_files(root, name)
                self.assertEqual(fixture['recipe_subprocesses'], 0)
                self.assertNotIn(b'$(shell', source)
                self.assertNotIn(b'\t', source)
                if 'implicit' in name:
                    self.assertEqual(len(list(root.glob('rule *.in'))), 500)
                elif fixture['files']:
                    self.assertEqual(len(list((root / 'rule-inputs').glob('input *.in'))), 1000)
                self.assertEqual(benchmark.workload_arguments(name), ['-rR', '--no-print-directory'])

    def test_rule_tilde_workloads_prepare_fixed_home_without_recipe_children(self):
        import tempfile
        from pathlib import Path
        workloads = list(benchmark.rule_tilde_workloads())
        self.assertEqual(workloads, list(benchmark.selected_workloads('rule-tilde')))
        self.assertEqual(len(workloads), 4)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name, source in workloads:
                fixture = benchmark.prepare_files(root, name)
                self.assertEqual(len(list((root / 'tilde-inputs').iterdir())), fixture['files'])
                self.assertEqual(fixture['recipe_subprocesses'], 0)
                self.assertTrue(source.startswith(b'unexport HOME\nHOME=tilde-inputs\n'))
                self.assertNotIn(b'\t', source)
                self.assertNotIn(b'$(shell', source)

    def test_raw_samples_and_extremes_are_retained(self):
        self.assertEqual(benchmark.summary([5, 1, 3]), {
            "samples_ns": [5, 1, 3], "median_ns": 3, "min_ns": 1, "max_ns": 5})

    def test_span_suite_covers_early_exit_and_full_scan_consumers(self):
        spans = list(benchmark.span_workloads())
        self.assertEqual([name for name, _ in spans], [
            "wordlist-prefix", "wordlist-middle", "wordlist-full", "wordlist-outside",
            "let-remainder", "patsubst-literal"])
        self.assertEqual(len({hashlib.sha256(source).digest() for _, source in spans}), 6)
        for _, source in spans:
            self.assertEqual(source.count(b"one  two\t"), 100000)
        self.assertEqual(list(benchmark.selected_workloads("word-spans")), spans)
        self.assertEqual(list(benchmark.selected_workloads("all")), list(benchmark.workloads()) + list(benchmark.startup_workloads()) + list(benchmark.eval_workloads()) + list(benchmark.startup_effect_workloads()) + list(benchmark.initial_flag_workloads()) + list(benchmark.builtin_transition_workloads()) + spans + list(benchmark.wildcard_workloads()) + list(benchmark.binding_workloads()) + list(benchmark.implicit_search_workloads()) + list(benchmark.parallel_branch_workloads()) + list(benchmark.target_context_workloads()) + list(benchmark.archive_workloads()) + list(benchmark.archive_sequence_workloads()) + list(benchmark.vpath_workloads()) + list(benchmark.secondary_workloads()) + list(benchmark.implicit_rejection_workloads()) + list(benchmark.gpath_workloads()) + list(benchmark.intermediate_workloads()) + list(benchmark.lazy_record_workloads()) + list(benchmark.vpath_record_workloads()) + list(benchmark.directory_search_workloads()) + list(benchmark.archive_name_workloads()) + list(benchmark.symlink_time_workloads()) + list(benchmark.rule_filename_workloads()) + list(benchmark.rule_tilde_workloads()) + list(benchmark.library_workloads()))
        self.assertEqual(list(benchmark.selected_workloads("core")), list(benchmark.workloads()))

    def test_implicit_search_has_real_targets_and_no_process_launches(self):
        workloads = list(benchmark.implicit_search_workloads())
        self.assertEqual(workloads, list(benchmark.implicit_search_workloads()))
        self.assertEqual([name for name, _ in workloads], ['implicit-unmatched-64', 'implicit-unmatched-256'])
        for (_, source), count in zip(workloads, (64, 256)):
            self.assertEqual(source.count(b'.out'), 401)
            self.assertEqual(source.count(b'.unused'), count)
            self.assertTrue(source.endswith(b'%.out:;\n'))
        self.assertEqual(list(benchmark.selected_workloads('implicit-search')), workloads)

    def test_implicit_rejections_have_known_empty_goals_and_no_effects(self):
        workloads = list(benchmark.implicit_rejection_workloads())
        self.assertEqual(workloads, list(benchmark.selected_workloads("implicit-rejections")))
        self.assertEqual([name for name, _ in workloads],
                         ["implicit-rejected-64", "implicit-rejected-256"])
        for (_, source), count in zip(workloads, (64, 256)):
            self.assertEqual(source.count(b"%.out: absent"), count)
            self.assertNotIn(b"$(", source)
            self.assertNotIn(b"\t", source)
            self.assertEqual(source.count(b"rejected63.out"), 2)

    def test_secondary_suite_covers_ready_lists_empty_clauses_and_many_targets(self):
        workloads = list(benchmark.secondary_workloads())
        self.assertEqual(workloads, list(benchmark.selected_workloads("secondary")))
        self.assertEqual([name for name, _ in workloads], [
            "secondary-ready-256", "secondary-ready-1024", "secondary-empty-256",
            "secondary-targets-1000"])
        for _, source in workloads:
            self.assertTrue(source.startswith(b".SECONDEXPANSION:\n"))
            self.assertNotIn(b"$(shell", source)
            self.assertNotIn(b"\t", source)
        self.assertEqual(workloads[2][1].count(b"all: $$(strip)"), 256)

    def test_vpath_workloads_cover_repeated_selection_and_real_files(self):
        import tempfile
        from pathlib import Path
        workloads = list(benchmark.vpath_workloads())
        self.assertEqual([name for name, _ in workloads], [
            'vpath-unmatched-64', 'vpath-unmatched-256', 'vpath-first-hit-64',
            'vpath-archive-64'])
        self.assertEqual(workloads, list(benchmark.selected_workloads('vpath-search')))
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for (name, source), rules in zip(workloads, (65, 257, 64, 65)):
                self.assertEqual(source.count(b'vpath '), rules)
                self.assertNotIn(b'$(shell', source)
                fixture = benchmark.prepare_files(root, name)
                if name == 'vpath-archive-64':
                    self.assertEqual(source.count(b'vpath-lib.a('), fixture['members'])
                    self.assertFalse((root / 'vpath-lib.a').exists())
                    self.assertTrue((root / 'inputs/vpath-lib.a').read_bytes().startswith(b'!<arch>\n'))
                else:
                    self.assertEqual(fixture['files'], 1000)
                    self.assertEqual((root / 'inputs/vpath-node0999.src').read_bytes(), b'original\n')

    def test_unclosed_archive_sequences_cover_two_input_sizes_without_effects(self):
        workloads = list(benchmark.archive_sequence_workloads())
        self.assertEqual(workloads, list(benchmark.selected_workloads('archive-sequences')))
        for (name, source), count in zip(workloads, (4096, 16384)):
            self.assertEqual(name, f'archive-sequence-unclosed-{count}')
            self.assertEqual(source.count(b'absent.a(member '), count)
            self.assertNotIn(b'$(shell', source)
            self.assertTrue(source.endswith(b'all:;\n'))

    def test_directory_search_inputs_are_fixed_time_and_misses_remain_absent(self):
        from pathlib import Path
        import tempfile
        workloads = list(benchmark.selected_workloads('directory-search'))
        self.assertEqual([name for name, _ in workloads],
                         ['directory-search-implicit-1000', 'directory-search-vpath-misses-1000'])
        for name, source in workloads:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                path = Path(directory)
                fixture = benchmark.prepare_files(path, name)
                files = [file for file in path.rglob('*') if file.is_file()]
                self.assertEqual(len(files), fixture['files'])
                self.assertTrue(all(file.read_bytes() == b'original\n'
                                    and file.stat().st_mtime_ns == fixture['mtime_ns']
                                    for file in files))
                self.assertFalse(list(path.glob('cache-missing*')))
                self.assertNotIn(b'$(shell', source)
                if 'implicit' in name:
                    self.assertEqual(len(list(path.glob('*.out'))), 1000)
                    self.assertEqual(len(list((path / 'inputs').iterdir())), 1000)
                else:
                    self.assertEqual(len([item for item in path.iterdir() if item.is_dir()]), 8)
                    self.assertEqual(len(files), 512)

    def test_vpath_records_prepare_current_inputs_and_separate_outputs(self):
        from pathlib import Path
        import tempfile
        workloads = list(benchmark.selected_workloads('vpath-records'))
        self.assertEqual([name for name, _ in workloads],
                         ['vpath-record-explicit-1000', 'vpath-record-aliases-1000',
                          'vpath-record-implicit-400'])
        for name, source in workloads:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                path = Path(directory)
                fixture = benchmark.prepare_files(path, name)
                files = list((path / 'records').iterdir())
                outputs = list(path.glob('*.out'))
                self.assertEqual(len(files), fixture['inputs'])
                self.assertEqual(len(outputs), fixture['outputs'])
                self.assertTrue(all(file.read_bytes() == b'original\n'
                                    and file.stat().st_mtime_ns == fixture['mtime_ns']
                                    for file in files + outputs))
                self.assertFalse(list(path.glob('*.in')))
                self.assertEqual(source.count(b'.in: ;'), fixture['inputs'])
                self.assertNotIn(b'$(shell', source)

    def test_lazy_record_inputs_keep_outputs_current_and_peers_absent(self):
        from pathlib import Path
        import tempfile
        workloads = list(benchmark.selected_workloads('lazy-records'))
        self.assertEqual([name for name, _ in workloads],
                         ['lazy-record-grouped-1000', 'lazy-record-double-1000'])
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, source in workloads:
                fixture = benchmark.prepare_files(root, name)
                self.assertEqual(len(list(root.glob('*.out'))), fixture['outputs'])
                self.assertEqual((root / 'record-input').stat().st_mtime_ns,
                                 fixture['input_mtime_ns'])
                self.assertEqual((root / 'record-node0999.out').stat().st_mtime_ns,
                                 fixture['output_mtime_ns'])
                self.assertLess(fixture['input_mtime_ns'], fixture['output_mtime_ns'])
                self.assertEqual(list(root.glob('*.mid')), [])
                self.assertEqual(list(root.glob('*.peer')), [])
                self.assertEqual(source.count(b'$$(strip)'), 1000)
        self.assertEqual(workloads[0][1].count(b' &: '), 1000)
        self.assertEqual(workloads[1][1].count(b'.mid::'), 2000)

    def test_intermediate_workloads_have_existing_outputs_and_missing_middles(self):
        from pathlib import Path
        import tempfile
        workloads = list(benchmark.selected_workloads('intermediates'))
        self.assertEqual([name for name, _ in workloads],
                         ['intermediate-secondary-1000', 'intermediate-not-intermediate-1000'])
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, source in workloads:
                fixture = benchmark.prepare_files(root, name)
                self.assertEqual((root / 'lazy-input').stat().st_mtime_ns,
                                 fixture['input_mtime_ns'])
                self.assertEqual((root / 'lazy-node0999.out').stat().st_mtime_ns,
                                 fixture['output_mtime_ns'])
                self.assertLess(fixture['input_mtime_ns'], fixture['output_mtime_ns'])
                self.assertEqual(len(list(root.glob('*.out'))), fixture['outputs'])
                self.assertEqual(list(root.glob('*.mid')), [])
                self.assertNotIn(b'$(shell', source)
                self.assertEqual(source.count(b': lazy-input ;'), 1000)
        self.assertTrue(workloads[0][1].startswith(b'.SECONDARY:\n'))
        self.assertTrue(workloads[1][1].startswith(b'.NOTINTERMEDIATE:\n.INTERMEDIATE:'))

    def test_gpath_inputs_are_real_noop_files_with_fixed_timestamps(self):
        from pathlib import Path
        import tempfile
        workloads = list(benchmark.selected_workloads('gpath'))
        self.assertEqual([name for name, _ in workloads],
                         ['gpath-explicit-1000', 'gpath-aliases-1000', 'gpath-implicit-400', 'gpath-mixed-1000'])
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, source in workloads:
                fixture = benchmark.prepare_files(root, name)
                count = 400 if name == 'gpath-implicit-400' else 1000
                extra = 1000 if name == 'gpath-mixed-1000' else 0
                self.assertEqual(fixture['files'], count * 2 + extra)
                if extra:
                    filename = root / 'other' / 'ordinary-node0999.src'
                    self.assertEqual(filename.read_bytes(), b'original\n')
                    self.assertEqual(filename.stat().st_mtime_ns, fixture['mtime_ns'])
                self.assertIn(b'GPATH = inputs\n', source)
                for extension in ('out', 'in'):
                    filename = root / 'inputs' / f'gpath-node{count - 1:04d}.{extension}'
                    self.assertEqual(filename.read_bytes(), b'original\n')
                    self.assertEqual(filename.stat().st_mtime_ns, fixture['mtime_ns'])
        self.assertIn(b'inputs/gpath-node0999.out', workloads[1][1])
        self.assertIn(b'%.out: %.in ;', workloads[2][1])
        self.assertIn(b'$(filter inputs/%,$^)', workloads[3][1])
        self.assertIn(b'$(filter other/%,$^)', workloads[3][1])

    def test_symlink_time_workloads_pin_arguments_links_and_noop_dates(self):
        import tempfile
        from pathlib import Path
        workloads = list(benchmark.selected_workloads('symlink-times'))
        self.assertEqual([name for name, _ in workloads], [
            'symlink-times-plain-2000', 'symlink-times-links-1000',
            'symlink-times-chain-500', 'symlink-times-dangling-1000'])
        with tempfile.TemporaryDirectory() as temporary:
            for index, (name, source) in enumerate(workloads):
                root = Path(temporary) / str(index)
                root.mkdir()
                fixture = benchmark.prepare_files(root, name)
                self.assertEqual(benchmark.workload_arguments(name), ['-rR', '--no-print-directory', '-L'])
                self.assertNotIn(b'$(shell', source)
                if 'plain' in name:
                    self.assertEqual(len(list(root.iterdir())), fixture['regular_files'])
                    self.assertEqual((root / 'sym-plain-1999').stat().st_mtime_ns, fixture['mtime_ns'])
                    continue
                link = root / 'sym-links/in-0000'
                self.assertTrue(link.is_symlink())
                self.assertEqual(link.lstat().st_mtime_ns, fixture['link_mtime_ns'])
                self.assertEqual((root / 'sym-0000.out').stat().st_mtime_ns, fixture['output_mtime_ns'])
                self.assertGreater(fixture['output_mtime_ns'], fixture['link_mtime_ns'])
                if 'dangling' in name:
                    self.assertFalse(link.exists())
                    self.assertEqual(fixture['referents'], 0)
                else:
                    self.assertTrue(link.is_file())
                    self.assertLess(link.stat().st_mtime_ns, fixture['output_mtime_ns'])
                if 'chain' in name:
                    self.assertEqual(link.readlink(), Path('hop-0000'))
                    self.assertEqual((root / 'sym-links/hop-0000').lstat().st_mtime_ns, fixture['hop_mtime_ns'])

    def test_archive_name_workloads_prepare_both_formats_and_wildcard_control(self):
        import tempfile
        from pathlib import Path
        workloads = list(benchmark.selected_workloads('archive-names'))
        self.assertEqual([name for name, _ in workloads], [
            'archive-names-gnu-128', 'archive-names-bsd-128', 'archive-names-glob-256'])
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name, source in workloads:
                fixture = benchmark.prepare_files(root, name)
                data = (root / 'lib.a').read_bytes()
                self.assertTrue(data.startswith(b'!<arch>\n'))
                self.assertEqual(hashlib.sha256(data).hexdigest(), fixture['archive_sha256'])
                self.assertIn(b'long-archive-member-0127.o', data)
                self.assertNotIn(b'$(shell', source)
                if fixture['format'] == 'bsd':
                    self.assertEqual(data.count(b'#1/26'), 128)
                else:
                    self.assertEqual(data[8:24], b'//              ')
                if name.endswith('128'):
                    self.assertEqual(source.count(b'lib.a('), fixture['members'])
                else:
                    self.assertEqual(source.count(b'$(wildcard'), 64)
                    self.assertEqual(fixture['members'], 256)

    def test_library_workloads_are_fixed_existing_inputs_without_children(self):
        import tempfile
        from pathlib import Path
        workloads = list(benchmark.library_workloads())
        self.assertEqual(len(workloads), 3)
        self.assertEqual(workloads, list(benchmark.selected_workloads('libraries')))
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name, source in workloads:
                fixture = benchmark.prepare_files(root, name)
                self.assertEqual(source.count(b'-l'), fixture['libraries'] + source.count(b'bench-library-'))
                self.assertNotIn(b'$(shell', source)
                self.assertNotIn(b'\t', source)
                self.assertEqual(fixture['recipe_subprocesses'], 0)
                self.assertEqual(len(list(root.rglob('bench-library-*'))), fixture['files'])
                for file in root.rglob('bench-library-*'):
                    self.assertEqual(file.read_bytes(), b'input\n')
                    self.assertEqual(file.stat().st_mtime_ns, 1600000000000000000)

    def test_archive_workloads_include_sparse_object_payloads(self):
        import tempfile
        from pathlib import Path
        workloads = list(benchmark.archive_workloads())
        self.assertEqual([name for name, _ in workloads], [
            'archive-noop-64', 'archive-noop-256', 'archive-payload-64-4096',
            'archive-payload-64-65536'])
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name, source in workloads:
                fixture = benchmark.prepare_files(root, name)
                self.assertEqual(source.count(b'lib.a('), fixture['members'])
                self.assertEqual((root / 'lib.a').stat().st_size,
                                 8 + fixture['members'] * (60 + fixture['member_bytes']
                                                          + fixture['member_bytes'] % 2))
                self.assertNotIn(b'$(shell', source)

    def test_target_contexts_cover_automatic_values_and_both_scope_kinds(self):
        workloads = list(benchmark.target_context_workloads())
        self.assertEqual([name for name, _ in workloads], [
            'target-context-none', 'target-context-explicit', 'target-context-pattern'])
        self.assertEqual(workloads, list(benchmark.selected_workloads('target-contexts')))
        for _, source in workloads:
            self.assertEqual(source.count(b'missing automatic target'), 3000)
            self.assertEqual(source.count(b'$(abspath input)'), 3000)
            self.assertNotIn(b'$(shell', source)
        self.assertEqual(workloads[1][1].count(b'VALUE = scoped'), 3000)
        self.assertEqual(workloads[2][1].count(b'VALUE = scoped'), 1)

    def test_wildcard_suite_distinguishes_cold_cached_complex_and_literal_queries(self):
        import tempfile
        from pathlib import Path
        workloads = list(benchmark.wildcard_workloads())
        self.assertEqual([name for name, _ in workloads],
                         ['wildcard-cold', 'wildcard-warm', 'wildcard-complex', 'wildcard-literal'])
        self.assertEqual([source.count(b'$(wildcard') for _, source in workloads], [1, 200, 200, 1])
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.assertIsNone(benchmark.prepare_files(root, 'tiny-noop'))
            fixture = benchmark.prepare_files(root, 'wildcard-cold')
            self.assertEqual(fixture['files'], 1000)
            self.assertEqual(len(list((root / 'inputs').iterdir())), 1000)
            self.assertEqual((root / 'inputs/file0999.c').read_bytes(), b'original\n')


if __name__ == "__main__":
    unittest.main()
