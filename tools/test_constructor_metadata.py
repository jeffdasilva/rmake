# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect independently qualified constructor phases and rejected-model evidence."""
import base64
import dataclasses
import json
from pathlib import Path
import tomllib
import unittest
import conformance as h

ROOT=Path(__file__).resolve().parents[1]

class ConstructorMetadataTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.evidence=json.loads((ROOT/'tests/research/constructor-metadata.json').read_text())
        cls.records={r['id']:r for r in cls.evidence['records']}
    def case(self,name):return h.load_case(ROOT/'tests/cases'/name)
    def stdout(self,name):return bytes.fromhex(self.case(name).expected['stdout_hex'])

    def test_required_inventory_retains_original_inputs_bounds_and_rejected_attempts(self):
        ledger=tomllib.loads((ROOT/'tests/requirements.toml').read_text())
        family=next(r for r in ledger['requirements'] if r['id']=='startup.constructor-metadata')
        self.assertEqual((family['fixture_count'],family['candidate_required']),(539,539))
        self.assertEqual(len(self.records),539)
        self.assertEqual(sum(bool(r['checks']['baseline']['candidate_differences']) for r in self.records.values()),346)
        self.assertEqual(len(self.evidence['incomplete_models']['records']),52)
        self.assertEqual(len(self.evidence['rejected_origin_exports']['records']),5)
        for r in self.records.values():
            case=self.case(r['id'])
            self.assertEqual((case.args,case.environment,case.expected,case.output),(tuple(r['args']),r['environment'],r['expected'],r['output']))
            for name,data in r['inputs'].items():
                p=case.path/name;self.assertEqual(p.stat().st_mode&0o777,data['mode'])
                if data['type']=='file':self.assertEqual(p.read_bytes(),bytes.fromhex(data['hex']))
                else:self.assertTrue(p.is_dir())
            for kind,check in r['checks'].items():
                self.assertEqual(check['oracle_repeat_differences'],[])
                self.assertEqual(check['model_errors'][:2],[[],[]])
                self.assertEqual(check['bounds'],[False,False,False])
                if kind!='baseline':self.assertEqual(check['candidate_differences'],[])

    def test_late_defaults_and_automatic_definitions_do_not_leak_into_metadata(self):
        for mode in ('none','target'):
            self.assertEqual(self.stdout(f'constructor-origin-0-rules-off-absent-{mode}'),b'EARLY=[undefined]\nLATE=[default]\n')
            self.assertEqual(self.stdout(f'constructor-origin-0-rules-off-environment-{mode}'),b'EARLY=[environment]\nLATE=[file]\n')
            for index in range(9,23):
                self.assertEqual(self.stdout(f'constructor-origin-{index}-variables-off-command-after-{mode}'),b'EARLY=[command line]\nLATE=[automatic]\n')
            record=self.records[f'constructor-imported-shell-{mode}-2']
            self.assertEqual(self.stdout(record['id']),b'CAPTURE=[x] CALLS=[yes:3] LATE=[file]\n')
            for observation in [*record['oracle'],record['debug'],record['release']]:
                self.assertEqual(base64.b64decode(observation['filesystem']['work/called']['content']),b'yes:3\n')

    def test_input_directories_survive_git_packaging(self):
        for name in self.records:
            root=self.case(name).path
            for directory in (p for p in root.rglob('*') if p.is_dir()):
                self.assertTrue(any(p.is_file() or p.is_symlink() for p in directory.rglob('*')),str(directory))

    def test_full_eval_keeps_unpublished_search_and_closes_all_historical_cells(self):
        historical=[r for r in self.records.values() if r['id'].startswith('include-meta-')]
        self.assertEqual(len(historical),19)
        self.assertEqual(sum(bool(r['checks']['baseline']['candidate_differences']) for r in historical),11)
        for mode in ('none','target'):
            for binding in ('none','simple-strong'):
                name=f'constructor-eval-old-search-{mode}-2-{binding}'
                self.assertEqual(self.stdout(name),b'VALUE=[cwd] FILES=[Makefile]\n')
                good=h.Observation(self.stdout(name),b'',0,None,{})
                self.assertTrue(h.expectation_errors(self.case(name),dataclasses.replace(good,stdout=b'VALUE=[old] FILES=[vars.mk Makefile]\n')))

    def test_completed_parent_descriptor_marker_proves_buffer_transition_order(self):
        selected={
            'none-none':b'make: cli\nCLI\nmake[2]: META\nMARK\nOUT\n',
            'none-target':b'make: cli\nCLI\nMARK\nmake[2]: META\nOUT\n',
            'target-none':b'make: cli\nCLI\nmake[2]: META\nMARK\nOUT\n',
            'target-target':b'MARK\nmake: cli\nCLI\nmake[2]: META\nOUT\n',
        }
        for modes,expected in selected.items():
            name=f'constructor-output-{modes}-2-normal-merged'
            self.assertEqual(self.stdout(name),expected)
            case=self.case(name);good=h.Observation(expected,b'',0,None,{})
            self.assertTrue(h.expectation_errors(case,dataclasses.replace(good,stdout=expected.replace(b'MARK\n',b''))))
        profiles=[r for r in self.records.values() if r['id'].startswith('constructor-output-')]
        self.assertEqual(len(profiles),128)
        for record in profiles:
            for observation in [*record['oracle'],record['debug'],record['release']]:
                self.assertEqual(bytes.fromhex(observation['stdout_hex']).count(b'MARK\n'),1)

    def test_metadata_mutation_does_not_reselect_process_recursion(self):
        for mode in ('none','target'):
            name=f'constructor-level-mutation-{mode}-environment'
            expected=b'PUBLIC=[7] ORIGIN=[file] EARLY=[4] LATE=[4]\n'
            self.assertEqual(self.stdout(name),expected)
            case=self.case(name);warning=b'make[3]: META\n'
            self.assertEqual(bytes.fromhex(case.expected['stderr_hex']),warning)
            good=h.Observation(expected,warning,0,None,{})
            self.assertTrue(h.expectation_errors(case,dataclasses.replace(good,stdout=expected.replace(b'[4]',b'[8]'))))
            name=f'constructor-level-mutation-{mode}-override-command'
            self.assertEqual(self.stdout(name),b'PUBLIC=[7] ORIGIN=[override] EARLY=[3] LATE=[3]\n')

    def test_cached_directory_refresh_and_public_overrides_have_separate_roles(self):
        for profile,expected in [('cached','work'),('directory-refresh','retired'),('command-origin','client'),('environment-override','client')]:
            record=self.records['constructor-cwd-'+profile]
            self.assertEqual(self.stdout(record['id']),f'NAME=[{expected}] ACTUAL=[retired]\n'.encode())
            for observation in [*record['oracle'],record['debug'],record['release']]:
                fs=observation['filesystem']
                self.assertEqual(fs['work'],{'type':'missing'})
                self.assertEqual(fs['home/retired/Makefile']['type'],'file')

if __name__=='__main__':unittest.main()
