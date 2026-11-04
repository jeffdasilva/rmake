# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
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
import directory_reporting as d


class DirectoryReportingTests(unittest.TestCase):
    def setUp(self):
        self.runtime = Path('/predetermined/runtime')
        self.model = d.Model('unit', {'Makefile': b'all:;@:\n', 'seen': b'anchor\n'}, (), {},
                             {'exit_code': 0, 'stdout_template_hex': b"make: Entering directory '@WORK@'\nPAYLOAD\nmake: Leaving directory '@WORK@'\n".hex(), 'stderr_template_hex': ''}, b'anchor\nDONE\n')
        self.case = h.Case(Path('/unused'), 'unit', 'directory.reporting', 'required', (), 5, {}, {})
        self.filesystem = {'work/seen': {'type': 'file', 'content': base64.b64encode(self.model.seen).decode()}}
        self.good = h.Observation(d.render(bytes.fromhex(self.model.expected['stdout_template_hex']), self.runtime), b'', 0, None, self.filesystem)

    def assess(self, observations, model=None, banners=None):
        return d.assess(model or self.model, self.case, self.runtime, self.filesystem,
                        observations, banners or [{} for _ in observations])

    def test_templates_use_only_predetermined_paths_and_do_not_reinterpret_replacements(self):
        self.assertEqual(d.render(b'@WORK@/sub @HOME@ @TMP@', self.runtime), b'/predetermined/runtime/work/sub /predetermined/runtime/home /predetermined/runtime/tmp')
        self.assertEqual(d.render(b'A@VERSION@Z', self.runtime, {b'@VERSION@': b'@WORK@'}), b'A@WORK@Z')
        with self.assertRaisesRegex(h.HarnessError, 'missing declared'): d.render(b'@HELP@', self.runtime)

    def test_shared_wrong_directory_paths_and_group_boundaries_fail(self):
        self.assertTrue(self.assess([self.good] * 3)['passed'])
        mutations = [self.good.stdout.replace(b'/predetermined/runtime/work', b'/observed/wrong'),
                     self.good.stdout.split(b'PAYLOAD')[0],
                     b'PAYLOAD\n' + self.good.stdout.replace(b'PAYLOAD\n', b''),
                     self.good.stdout + self.good.stdout,
                     self.good.stdout.replace(b'Entering', b'Leaving')]
        for stdout in mutations:
            result = self.assess([replace(self.good, stdout=stdout)] * 3)
            self.assertFalse(result['passed'])
            self.assertEqual(result['candidate_differences'], [])
            self.assertTrue(all(result['expected_errors']))

    def test_shared_filesystem_state_and_status_cannot_pass(self):
        for bad in (replace(self.good, filesystem={}),
                    replace(self.good, filesystem={**self.filesystem, 'work/extra': {'type': 'file'}}),
                    replace(self.good, filesystem={'work/seen': {'type': 'symlink', 'target': 'wrong'}}),
                    replace(self.good, returncode=2), replace(self.good, stderr=b'unexpected')):
            self.assertFalse(self.assess([bad] * 3)['passed'])
        wrong_seen = {'work/seen': {'type': 'file', 'content': base64.b64encode(b'anchor\n').decode()}}
        self.assertFalse(self.assess([replace(self.good, filesystem=wrong_seen)] * 3)['passed'])

    def test_repeat_candidate_and_containment_are_mandatory(self):
        for bad in (replace(self.good, limit='timeout'), replace(self.good, leftover_processes=True),
                    replace(self.good, stdout=b'wrong')):
            for index in range(3):
                observations = [self.good] * 3
                observations[index] = bad
                self.assertFalse(self.assess(observations)['passed'])
        self.assertTrue(self.assess([self.good] * 2)['passed'])
        with self.assertRaises(h.HarnessError): self.assess([self.good])

    def test_cosmetic_banner_is_one_exact_slot_with_strict_directory_boundaries(self):
        template = b"make: Entering directory '@WORK@'\n@VERSION@make: Leaving directory '@WORK@'\n"
        model = replace(self.model, expected={**self.model.expected, 'stdout_template_hex': template.hex()})
        banners = [{b'@VERSION@': b'GNU Make reference\n'}] * 2 + [{b'@VERSION@': b'rmake candidate\n'}]
        observations = [replace(self.good, stdout=d.render(template, self.runtime, banner)) for banner in banners]
        self.assertTrue(self.assess(observations, model, banners)['passed'])
        for stdout in (observations[2].stdout + b'extra\n', observations[2].stdout.replace(b'rmake candidate\n', b''),
                       b'rmake candidate\n' + observations[2].stdout.replace(b'rmake candidate\n', b'')):
            self.assertFalse(self.assess([*observations[:2], replace(observations[2], stdout=stdout)], model, banners)['passed'])
        banner = replace(self.good, stdout=b'rmake candidate\n', filesystem=self.filesystem)
        self.assertTrue(d.valid_banner(b'@VERSION@', banner, self.filesystem))
        self.assertFalse(d.valid_banner(b'@VERSION@', replace(banner, stdout=banner.stdout+b'Entering directory wrong\n'), self.filesystem))
        self.assertFalse(d.valid_banner(b'@VERSION@', replace(banner, filesystem={}), self.filesystem))

    def test_prepared_filesystem_is_input_based_and_ordinary_cases_never_compile(self):
        with tempfile.TemporaryDirectory() as tmp, patch.object(d.h, 'capture', side_effect=AssertionError('ordinary compilation')):
            case, compilation, filesystem = d.prepare(self.model, Path(tmp)/'inputs', self.runtime, None)
            self.assertEqual(compilation, [])
            self.assertEqual((case.path/'Makefile').read_bytes(), b'all:;@:\n')
            self.assertEqual(base64.b64decode(filesystem['work/seen']['content']), self.model.seen)
            self.assertEqual(filesystem['home'], {'type':'directory','mode':0o755})
            self.assertEqual(filesystem['work/Makefile']['sha256'], hashlib.sha256(b'all:;@:\n').hexdigest())
        for name in ('/absolute', '../outside', 'a/../../outside', 'nul\0name', ''):
            with self.assertRaises(h.HarnessError): d.safe_relative(name)

    def test_native_finalizer_compiles_once_with_bounded_capture(self):
        source = next(model.modules[0].source for model in d.models() if model.modules)
        model = replace(self.model, modules=(d.Module(source, 'probe.so'),))
        events = []
        def compile(command, cwd, environment, timeout):
            events.append((command, timeout));Path(command[-1]).write_bytes(b'original-fixture');return b'',b'',0,None,False
        with tempfile.TemporaryDirectory() as tmp, patch.object(d.h, 'capture', side_effect=compile):
            _, compilation, filesystem = d.prepare(model, Path(tmp)/'inputs', self.runtime, '/compiler')
        self.assertEqual(len(events), 1)
        self.assertEqual(events[0][1], 15)
        self.assertEqual(compilation[0]['module_sha256'], hashlib.sha256(b'original-fixture').hexdigest())
        self.assertEqual(filesystem['work/probe.so']['mode'], 0o755)
        with tempfile.TemporaryDirectory() as tmp, patch.object(d.h, 'capture', return_value=(b'',b'error',1,None,False)):
            with self.assertRaisesRegex(h.HarnessError, 'compilation failed'): d.prepare(model, Path(tmp)/'inputs', self.runtime, '/compiler')

    def test_campaign_resets_same_paths_and_runs_gnu_twice_then_candidate(self):
        for candidate, order in ((Path('/candidate'), ['/oracle','/oracle','/candidate']), (None, ['/oracle','/oracle'])):
            events = []
            def execute(case, runtime, binary):
                self.assertEqual(runtime.name, 'runtime');events.append(str(binary));return self.good
            with tempfile.TemporaryDirectory() as tmp, patch.object(d, 'prepare', return_value=(self.case, [], self.filesystem)), patch.object(d.h, 'execute', side_effect=execute):
                with patch.object(d, 'assess', return_value={'passed':True}):
                    report=d.run_campaign([self.model], Path('/oracle'), candidate, None, Path(tmp))
            self.assertEqual(events, order)
            self.assertTrue(report['passed'])

    def test_cli_compiler_is_optional_for_ordinary_selection_and_failures_are_nonzero(self):
        model = d.models()[0]
        with tempfile.TemporaryDirectory() as tmp:
            report=Path(tmp)/'report.json'
            arguments=['--oracle','/oracle','--oracle-only','--report',str(report),'--case',model.id]
            with patch.object(d, 'models', return_value=[model]), patch.object(d.shutil, 'which', side_effect=AssertionError('ordinary compiler lookup')), patch.object(d.reference_oracle, 'verify'), patch.object(d.h,'executable_identity',return_value={'path':'/oracle'}), patch.object(d,'run_campaign',return_value={'passed':False,'totals':{'cases':1,'pass':0,'fail':1}}):
                self.assertEqual(d.main(arguments), 1)
            self.assertFalse(json.loads(report.read_text())['passed'])
            with patch.object(d, 'models', return_value=[model]):
                self.assertEqual(d.main([*arguments,'--case','unknown']), 2)

    def test_original_inventory_and_retained_history_hashes(self):
        models=d.models();self.assertEqual(len(models),40);self.assertEqual(len({m.id for m in models}),40)
        self.assertEqual(sum(bool(model.modules) for model in models),2)
        self.assertEqual(sum(b'@VERSION@' in bytes.fromhex(model.expected['stdout_template_hex']) for model in models),1)
        for path in d.SOURCES.glob('*.json'):
            manifest=json.loads(path.read_bytes());packed=path.with_suffix('.json.gz').read_bytes()
            self.assertEqual(hashlib.sha256(packed).hexdigest(),manifest['sha256'])
            self.assertEqual(hashlib.sha256(gzip.decompress(packed)).hexdigest(),manifest['raw_sha256'])


if __name__ == '__main__': unittest.main()
