# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Keep an unapproved normal-exit policy proposal separate from safe errors."""
import base64
import gzip
import hashlib
import json
from pathlib import Path
import re
import unittest

import safety
import conformance as h
import nested_short_proposal as live

ROOT = Path(__file__).resolve().parents[1]


class NestedShortProposalTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        manifest = json.loads((ROOT / 'tests/research/nested-short-options-proposal.json').read_text())
        packed = (ROOT / 'tests/research/nested-short-options-proposal.json.gz').read_bytes()
        raw = gzip.decompress(packed)
        assert hashlib.sha256(packed).hexdigest() == manifest['sha256']
        assert hashlib.sha256(raw).hexdigest() == manifest['uncompressed_sha256']
        assert len(raw) == manifest['uncompressed_bytes']
        cls.proof = json.loads(raw)
        assert not manifest['approved'] and not cls.proof['approved']

    def record(self, name):
        return json.loads(bytes.fromhex(self.proof['files'][name]['hex']))

    def test_raw_normal_exit_disagreement_is_not_fault_or_conformance_credit(self):
        records = self.record('baseline.json')['records'] + self.record('boundary/baseline.json')['records']
        self.assertEqual(len(records), 32)
        different = [r for r in records if r['oracle_repeat_differences']]
        self.assertEqual({r['id'] for r in different}, {
            'nested-cursor-tail-4-recursive-short',
            'nested-cursor-tail-5-simple-short',
            'nested-cursor-tail-5-recursive-short',
            'nested-boundary-simple-p1-short'})
        for row in records:
            for observed in row['observations'][:2]:
                self.assertEqual(observed['returncode'], 0)
                self.assertFalse(observed['limit'] or observed['leftover_processes'])
        for row in different:
            outputs = [bytes.fromhex(o['stdout_hex']) for o in row['observations'][:2]]
            policy = [re.search(rb'FLAGS=\[([^]]*)\]', out)[1] for out in outputs]
            banners = [b'GNU Make 4.4.1\n' in out for out in outputs]
            self.assertTrue(policy[0] != policy[1] or banners[0] != banners[1])
        self.assertEqual(sum(any(r['model_errors'][:2]) for r in records), 10)
        self.assertEqual(len(safety.inventory()), 17)
        self.assertNotIn('flag-payload-ordered-14', {c.id for c in safety.inventory()})

    def test_proposed_errors_and_normal_controls_retain_all_raw_checks(self):
        for build in ('debug', 'release'):
            proposal = self.record('proposal-' + build + '.json')
            self.assertFalse(proposal['approved'])
            self.assertEqual(len(proposal['records']), 44)
            self.assertEqual(set(proposal['binary']), {p for p in self.proof['initial_binaries'] if p.endswith(build)})
            for path, digest in proposal['binary'].items():
                self.assertEqual(digest, self.proof['initial_binaries'][path])
            for row in proposal['records']:
                self.assertEqual(len(row['observations']), 2)
                for observed in row['observations']:
                    self.assertEqual(observed['returncode'], 2)
                    self.assertEqual((observed['stdout_hex'], observed['stderr_hex']),
                                     (row['expected']['stdout_hex'], row['expected']['stderr_hex']))
                    self.assertFalse(observed['limit'] or observed['leftover_processes'])
                    self.assertEqual(set(observed['filesystem']), {'work', 'work/Makefile', 'home', 'tmp'})
                    self.assertTrue(base64.b64decode(observed['filesystem']['work/Makefile']['content']).startswith(b'MAKEFLAGS = MAKEFLAGS=-s '))
            controls = self.record('controls-' + build + '.json')['records']
            self.assertEqual(len(controls), 14)
            for row in controls:
                self.assertEqual((row['model_errors'], row['oracle_repeat_differences'], row['candidate_differences'], row['bounds']),
                                 ([[], [], []], [], [], [False] * 3))
            final = self.record('proposal-review-' + build + '.json')
            self.assertEqual(len(final['records']), 60)
            for path, digest in final['binary'].items():
                self.assertEqual(digest, self.proof['binaries'][path])
            for row in final['records']:
                self.assertEqual(len(row['observations']), 2)
                for observed in row['observations']:
                    self.assertEqual((observed['returncode'], observed['stdout_hex'], observed['stderr_hex']),
                                     (2, row['expected']['stdout_hex'], row['expected']['stderr_hex']))
                    self.assertFalse(observed['limit'] or observed['leftover_processes'])
            final_controls = self.record('controls-review-' + build + '.json')['records']
            self.assertEqual(len(final_controls), 18)
            self.assertTrue(all(not any(r['model_errors']) and not r['candidate_differences']
                                and not r['oracle_repeat_differences'] and not any(r['bounds']) for r in final_controls))
        broad = self.record('controls-broad-prototype.json')['records']
        self.assertEqual(sum(bool(r['candidate_differences']) for r in broad), 4)
        patch = bytes.fromhex(self.proof['source_patch_hex'])
        self.assertEqual(hashlib.sha256(patch).hexdigest(), self.proof['source_patch_sha256'])

    def test_live_checker_rejects_changed_status_output_and_protected_files(self):
        case = h.Case(ROOT, 'review', 'proposal.S004', 'required', (), 5, {},
                      dict(exit_code=2, stdout_hex='', stderr_hex=b'expected\n'.hex()))
        protected = {'work': {'type': 'directory', 'mode': 493}}
        good = h.Observation(b'', b'expected\n', 2, None, protected)
        self.assertEqual(live.failures(case, good, protected), [])
        for changed in (h.Observation(b'bad', b'expected\n', 2, None, protected),
                        h.Observation(b'', b'bad', 2, None, protected),
                        h.Observation(b'', b'expected\n', 0, None, protected),
                        h.Observation(b'', b'expected\n', 2, None, {})):
            self.assertTrue(live.failures(case, changed, protected))


if __name__ == '__main__':
    unittest.main()
