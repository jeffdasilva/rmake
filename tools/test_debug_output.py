# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import ast
import base64
import errno
from dataclasses import replace
import gzip
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import conformance as h
import debug_output as d


class DebugOutputTests(unittest.TestCase):
    def setUp(self):
        self.runtime = Path('/predetermined/runtime')
        self.stdout = b"GNU Make 4.4.1\nReading makefile '@WORK@/Makefile'...\nConsidering target file 'all'.\nSuccessfully remade target file 'all'.\n"
        self.model = d.Model('unit', {'Makefile': b'all:;@:\n', 'seen': b'anchor\n'}, (), {},
                             {'exit_code': 0, 'stdout_template_hex': self.stdout.hex(), 'stderr_template_hex': ''},
                             b'anchor\nREAD\nDONE\n')
        self.case = h.Case(Path('/unused'), 'unit', 'debug.output', 'required', (), 5, {}, {})
        self.filesystem = {'work/seen': {'type':'file','mode':0o644,'content':base64.b64encode(self.model.seen).decode()}}
        self.good = h.Observation(d.render(self.stdout, self.runtime), b'', 0, None, self.filesystem)

    def assess(self, observations):
        return d.assess(self.model, self.case, self.runtime, self.filesystem, observations)

    def test_paths_are_predetermined_and_cosmetic_slots_are_rejected(self):
        self.assertEqual(d.render(b'@WORK@/child @BIN@', self.runtime), b'/predetermined/runtime/work/child /predetermined/runtime/bin/make')
        for template in (b'@VERSION@', b'@HELP@', b'@HOME@', b'@UNKNOWN@'):
            with self.assertRaisesRegex(h.HarnessError, 'unsupported debug template'): d.render(template, self.runtime)
        self.assertEqual(d.render(b'$(value @) @:', self.runtime), b'$(value @) @:')

    def test_shared_wrong_debug_header_paths_indentation_and_order_fail(self):
        self.assertTrue(self.assess([self.good]*3)['passed'])
        for stdout in (self.good.stdout.replace(b'GNU Make 4.4.1',b'rmake branded version'),
                       self.good.stdout.replace(b'/predetermined/runtime/work',b'/observed/wrong'),
                       self.good.stdout.replace(b'Considering',b' Considering'),
                       self.good.stdout.replace(b"Considering target file 'all'.\n",b''),
                       self.good.stdout+self.good.stdout,
                       self.good.stdout+b'Live child 0x123 pid 456\n'):
            result=self.assess([replace(self.good,stdout=stdout)]*3)
            self.assertFalse(result['passed']);self.assertEqual(result['candidate_differences'],[])
            self.assertTrue(all(result['expected_errors']))

    def test_candidate_header_is_not_cosmetically_normalized(self):
        candidate=replace(self.good,stdout=self.good.stdout.replace(b'GNU Make 4.4.1',b'rmake 0.1'))
        result=self.assess([self.good,self.good,candidate])
        self.assertFalse(result['passed']);self.assertIn('stdout',result['candidate_differences'])
        self.assertTrue(result['expected_errors'][2])

    def test_shared_wrong_filesystem_effects_and_modes_fail(self):
        for filesystem in ({}, {**self.filesystem,'work/extra':{'type':'file'}},
                           {'work/seen':{**self.filesystem['work/seen'],'mode':0o755}},
                           {'work/seen':{'type':'symlink','target':'elsewhere'}},
                           {'work/seen':{**self.filesystem['work/seen'],'content':base64.b64encode(b'anchor\nDONE\nREAD\n').decode()}}):
            result=self.assess([replace(self.good,filesystem=filesystem)]*3)
            self.assertFalse(result['passed']);self.assertEqual(result['filesystem_errors'],[True]*3)

    def test_repeats_status_stderr_and_process_bounds_are_strict(self):
        for bad in (replace(self.good,returncode=2),replace(self.good,stderr=b'extra'),
                    replace(self.good,limit='timeout'),replace(self.good,leftover_processes=True)):
            for index in range(3):
                observations=[self.good]*3;observations[index]=bad
                self.assertFalse(self.assess(observations)['passed'])
        self.assertTrue(self.assess([self.good]*2)['passed'])
        with self.assertRaises(h.HarnessError): self.assess([self.good])

    def test_preparation_preserves_declared_nanoseconds_and_uses_no_compiler(self):
        model=replace(self.model,files={**self.model.files,'child/source':b'raw'},mtimes_ns={'child/source':1_000_000_001},
                      updates={'generated.mk':b'ROOT := @WORK@\n'})
        with tempfile.TemporaryDirectory()as tmp,patch.object(d.h,'capture',side_effect=AssertionError('no compiler')):
            case,filesystem=d.prepare(model,Path(tmp)/'inputs',self.runtime)
            self.assertEqual((case.path/'child/source').stat().st_mtime_ns,1_000_000_001)
            self.assertEqual(base64.b64decode(filesystem['work/seen']['content']),model.seen)
            self.assertEqual(base64.b64decode(filesystem['work/generated.mk']['content']),b'ROOT := /predetermined/runtime/work\n')
            self.assertEqual(filesystem['home'],{'type':'directory','mode':0o755})
        for name in ('/absolute','../outside','a/../../outside','nul\0name',''):
            with self.assertRaises(h.HarnessError): d.safe_relative(name)
        with tempfile.TemporaryDirectory()as tmp:
            with self.assertRaisesRegex(h.HarnessError,'conflicting ordered'):
                d.prepare(replace(self.model,updates={'seen':b'contradiction'}),Path(tmp)/'inputs',self.runtime)

    def test_campaign_runs_gnu_twice_then_candidate_at_identical_reset_paths(self):
        for candidate,order in ((Path('/candidate'),['/oracle','/oracle','/candidate']),(None,['/oracle','/oracle'])):
            events=[]
            def execute(case,runtime,binary):
                self.assertEqual(runtime.name,'runtime');events.append(str(binary));return self.good
            with tempfile.TemporaryDirectory()as tmp,patch.object(d,'prepare',return_value=(self.case,self.filesystem)),patch.object(d.h,'execute',side_effect=execute),patch.object(d,'assess',return_value={'passed':True}):
                report=d.run_campaign([self.model],Path('/oracle'),candidate,Path(tmp))
            self.assertEqual(events,order);self.assertTrue(report['passed'])
            self.assertEqual(report['oracle_only'],candidate is None)

    def test_cli_rejects_failed_candidates_models_and_unknown_exact_ids(self):
        with tempfile.TemporaryDirectory()as tmp:
            report=Path(tmp)/'report.json'
            for oracle_only in (True,False):
                arguments=['--oracle','/oracle','--report',str(report),'--case','unit']+(['--oracle-only']if oracle_only else['--candidate','/candidate'])
                with patch.object(d,'models',return_value=[self.model]),patch.object(d.reference_oracle,'verify'),patch.object(d.h,'sha256_file',return_value='hash'),patch.object(d.h,'executable_identity',side_effect=lambda path,**kwargs:{'path':str(path)}),patch.object(d,'run_campaign',return_value={'passed':False,'totals':{'cases':1,'pass':0,'fail':1}}):
                    self.assertEqual(d.main(arguments),1)
                self.assertFalse(json.loads(report.read_text())['passed'])
            with patch.object(d,'models',return_value=[self.model]):
                self.assertEqual(d.main(['--oracle-only','--report',str(report),'--case','uni']),2)


    def test_explicit_version_slot_preserves_every_surrounding_debug_and_directory_byte(self):
        template=b"EARLY\nmake: *** unknown debug level specification 'z'.  Stop.\n@EXPLICIT_VERSION@make: Leaving directory '@WORK@'\nTRACE-TAIL\n"
        reference=b'GNU Make reference\n'
        model=replace(self.model,id='debug-invalid-before-version',args=('--version',),
                      expected={**self.model.expected,'stdout_template_hex':template.hex()},explicit_version_reference=reference)
        versions=[reference,reference,b'rmake candidate\n']
        observations=[replace(self.good,stdout=d.render(template,self.runtime,version))for version in versions]
        self.assertTrue(d.assess(model,self.case,self.runtime,self.filesystem,observations,versions)['passed'])
        candidate=observations[2].stdout
        for wrong in (b'rmake candidate\n'+candidate.replace(b'rmake candidate\n',b''),
                      candidate.replace(b"make: *** unknown debug level specification 'z'.  Stop.\n",b''),
                      candidate.replace(b'rmake candidate\n',b'GNU Make automatic debug header\nrmake candidate\n'),
                      candidate.replace(b'rmake candidate\n',b'')+b'rmake candidate\n'):
            result=d.assess(model,self.case,self.runtime,self.filesystem,[*observations[:2],replace(observations[2],stdout=wrong)],versions)
            self.assertFalse(result['passed']);self.assertTrue(result['expected_errors'][2])
        banner=replace(self.good,stdout=b'rmake candidate\n',filesystem=self.filesystem)
        self.assertTrue(d.valid_explicit_version(banner,self.filesystem))
        for marker in (b'Entering directory wrong\n',b'Considering target file bad\n',b'EARLY\n'):
            self.assertFalse(d.valid_explicit_version(replace(banner,stdout=banner.stdout+marker),self.filesystem))
        self.assertFalse(d.valid_explicit_version(banner,self.filesystem,reference))

    def test_explicit_version_branding_is_restricted_to_its_declared_boundary(self):
        text=(d.SOURCES/'models.toml').read_text().replace('id = "debug-invalid-before-version"','id = "wrong-boundary"')
        with tempfile.TemporaryDirectory()as tmp:
            root=Path(tmp);(root/'models.toml').write_text(text)
            with patch.object(d,'SOURCES',root):
                with self.assertRaisesRegex(h.HarnessError,'one declared boundary'):d.models()

    def test_proc_reader_handles_exit_after_open_without_hiding_other_errors(self):
        model = next(row for row in d.models() if row.id == 'debug-child-queued-parallel')
        function, = [node for node in ast.parse(model.files['worker.py']).body
                     if isinstance(node, ast.FunctionDef) and node.name == 'identity']
        namespace = {'Path': Path}
        exec(compile(ast.Module(body=[function], type_ignores=[]), 'worker.py', 'exec'), namespace)
        identity = namespace['identity']
        for error in (FileNotFoundError(errno.ENOENT, 'exited before open'),
                      ProcessLookupError(errno.ESRCH, 'exited after open')):
            with patch.object(Path, 'read_bytes', side_effect=error):
                self.assertIsNone(identity(1234))
        with patch.object(Path, 'read_bytes', side_effect=PermissionError(errno.EACCES, 'denied')):
            with self.assertRaises(PermissionError):
                identity(1234)

    def test_static_inventory_and_retained_history_hashes(self):
        models=d.models();self.assertEqual(len(models),88);self.assertEqual(len({model.id for model in models}),88)
        self.assertEqual(sum(model.id.startswith('debug-child-') for model in models),10)
        self.assertEqual(sum(model.id.startswith('debug-boundary-') for model in models),14)
        explicit=[model for model in models if model.explicit_version_reference is not None]
        self.assertEqual([model.id for model in explicit],['debug-invalid-before-version'])
        for model in models:
            stdout=bytes.fromhex(model.expected['stdout_template_hex'])
            self.assertNotIn(b'@VERSION@',stdout);self.assertNotIn(b'@HELP@',stdout)
        for path in d.SOURCES.glob('*.json'):
            manifest=json.loads(path.read_bytes());packed=path.with_suffix('.json.gz').read_bytes()
            self.assertEqual(hashlib.sha256(packed).hexdigest(),manifest['sha256'])
            self.assertEqual(hashlib.sha256(gzip.decompress(packed)).hexdigest(),manifest['raw_sha256'])


if __name__=='__main__':unittest.main()
