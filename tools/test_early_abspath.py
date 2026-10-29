# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect independently modeled startup directory availability and effects."""
import base64
import dataclasses
import json
from pathlib import Path
import tomllib
import unittest
import conformance as h
ROOT=Path(__file__).resolve().parents[1]

class EarlyAbsolutePathTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.evidence=json.loads((ROOT/'tests/research/early-abspath.json').read_text())
        cls.records={r['id']:r for r in cls.evidence['records']}
    def case(self,name):return h.load_case(ROOT/'tests/cases'/name)
    def stdout(self,name):return bytes.fromhex(self.case(name).expected['stdout_hex'])

    def test_inventory_preserves_all_input_roles_and_frozen_observations(self):
        ledger=tomllib.loads((ROOT/'tests/requirements.toml').read_text())
        family=next(r for r in ledger['requirements'] if r['id']=='startup.abspath-phases')
        self.assertEqual((family['fixture_count'],family['candidate_required']),(132,132))
        self.assertEqual(sum(bool(r['checks']['baseline']['candidate_differences']) for r in self.records.values()),76)
        for r in self.records.values():
            case=self.case(r['id'])
            self.assertEqual((case.args,case.environment,case.expected),(tuple(r['args']),r['environment'],r['expected']))
            for name,data in r['inputs'].items():
                path=case.path/name
                self.assertEqual(path.stat().st_mode&0o777,data['mode'])
                if data['type']=='file':self.assertEqual(path.read_bytes(),bytes.fromhex(data['hex']))
                else:self.assertTrue(any(p.is_file() for p in path.rglob('*')))
            for kind,check in r['checks'].items():
                self.assertEqual(check['oracle_repeat_differences'],[])
                self.assertEqual(check['model_errors'][:2],[[],[]])
                self.assertEqual(check['bounds'],[False,False,False])
                if kind!='baseline':self.assertEqual(check['candidate_differences'],[])

    def test_initial_relative_words_disappear_while_absolute_words_still_normalize(self):
        for role in ('gnu','make','simple'):
            label='SAVED' if role=='simple' else 'EARLY'
            for mode in ('none','target'):
                for directory in ('same','chdir'):
                    relative=f'early-abspath-{role}-{mode}-{directory}-0'
                    self.assertEqual(self.stdout(relative),f'{label}=[]\nLATE=[after]\n'.encode())
                    mixed=f'early-abspath-{role}-{mode}-{directory}-4'
                    expected=f'{label}=[/three /five]\nLATE=[after]\n'.encode()
                    self.assertEqual(self.stdout(mixed),expected)
                    good=h.Observation(expected,b'',0,None,{})
                    self.assertTrue(h.expectation_errors(self.case(mixed),dataclasses.replace(good,stdout=expected.replace(b'/three /five',b'/work/one /three /four /five'))))
        self.assertEqual(self.stdout('early-abspath-recursive-none-same'),b'VALUE=[child]\nLATE=[after]\n')

    def test_argument_file_effects_and_computed_name_errors_retain_their_phase(self):
        for record in self.records.values():
            if not record['id'].startswith('early-abspath-argument-effect-'):continue
            self.assertIn(b'EFFECT=[ARG]\n',bytes.fromhex(record['expected']['stdout_hex']))
            for observation in [*record['oracle'],record['debug'],record['release']]:
                self.assertEqual(base64.b64decode(observation['filesystem']['work/seen']['content']),b'ARG\n')
        for mode in ('none','target'):
            for directory in ('same','chdir'):
                name=f'early-abspath-name-relative-{mode}-{directory}'
                case=self.case(name)
                error=b'make: *** empty variable name.  Stop.\n'
                self.assertEqual(case.expected,dict(exit_code=2,stdout_hex='',stderr_hex=error.hex()))
                self.assertTrue(h.expectation_errors(case,h.Observation(b'',b'',0,None,{})))
                self.assertEqual(self.stdout(f'early-abspath-name-absolute-{mode}-{directory}'),b'VALUE=[ok]\nLATE=[after]\n')

    def test_rereads_restore_pending_phase_and_generate_exactly_one_include(self):
        selected=[r for r in self.records.values() if r['id'].startswith('early-abspath-restart-')]
        self.assertEqual(len(selected),12)
        for record in selected:
            output=bytes.fromhex(record['expected']['stdout_hex'])
            self.assertIn(b'PASS=[]\n',output);self.assertIn(b'PASS=[1]\n',output)
            relative='inc/generated.mk' if '-Cinc' in record['args'] else 'generated.mk'
            for observation in [*record['oracle'],record['debug'],record['release']]:
                self.assertEqual(base64.b64decode(observation['filesystem']['work/'+relative]['content']),b'# generated\n')

if __name__=='__main__':unittest.main()
