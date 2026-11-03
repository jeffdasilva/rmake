# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Independent anchors and negative controls for the exception-only gate."""
import contextlib
import copy
import dataclasses
import io
import json
from pathlib import Path
import tempfile
import tomllib
import unittest
from unittest import mock

import conformance as harness
import safety


class SafetyTests(unittest.TestCase):
    def filesystem(self, case):
        files = safety.input_filesystem(case)
        if case.requirement == 'safety.S003':
            files['work/capture'] = {'type': 'file', 'mode': 0o755}
        return files

    def test_valid_boundaries_remain_ordinary_exact_matches(self):
        ledger = tomllib.loads((safety.ROOT / 'tests/requirements.toml').read_text())
        family = next(row for row in ledger['requirements'] if row['id'] == 'compatibility.safety-boundaries')
        self.assertEqual(family['candidate_required'], 99)
        self.assertEqual(family['fixture_count'], 99)
        for name in family['cases']:
            case = harness.load_case(safety.ROOT / 'tests/cases' / name)
            self.assertEqual(case.candidate, 'required')
            self.assertNotIn(b'safety error', bytes.fromhex(case.expected['stderr_hex']))
        case = harness.load_case(safety.ROOT / 'tests/cases/safety-control-shell-double-quote-normal')
        self.assertIn(b'<c "extra words">\n', bytes.fromhex(case.expected['stdout_hex']))
        self.assertEqual(harness.load_case(safety.ROOT / 'tests/cases/safety-control-export-local-shell').expected['exit_code'], 0)

    def test_every_approved_exception_has_separate_immutable_evidence(self):
        cases = safety.inventory()
        self.assertEqual(len(cases), 32)
        self.assertEqual({case.requirement for case in cases},
                         {'safety.' + code for code in safety.CODES})
        # The owner explicitly extended S004 to this formerly ordinary gap.
        self.assertIn('flag-payload-ordered-14', {case.id for case in cases})
        for case in cases:
            self.assertFalse((safety.ROOT / 'tests/cases' / case.id).exists())

    def test_normal_exit_approval_requires_exact_scope_and_real_policy_instability(self):
        approval = json.loads((safety.ROOT / 'tests/safety/nested-short-normal-approval.json').read_text())
        records = json.loads((safety.ROOT / approval['evidence']).read_text())['records']
        self.assertEqual(set(safety.validate_normal_approval(approval, records)), safety.NORMAL_SHORT_SCOPE)
        for kind in ('unapproved', 'decision', 'scope', 'missing', 'instability'):
            changed = copy.deepcopy(approval)
            if kind == 'unapproved': changed['approved'] = False
            elif kind == 'decision': changed['owner_decision'] = 'not reviewed'
            elif kind == 'scope': changed['scope'][0]['id'] = 'ordinary-unapproved-input'
            elif kind == 'missing': changed['scope'].pop()
            else: changed['instability_evidence'].pop()
            with self.subTest(kind=kind), self.assertRaises(harness.HarnessError):
                safety.validate_normal_approval(changed, records)
        changed = copy.deepcopy(records)
        record = next(r for r in changed if r['id'] == 'nested-cursor-tail-4-recursive-short')
        record['oracle_runs'][1]['stdout'] = (bytes.fromhex(record['oracle_runs'][0]['stdout']) + b'PID 999\n').hex()
        with self.assertRaisesRegex(harness.HarnessError, 'PID/address'):
            safety.validate_normal_approval(approval, changed)

    def test_normal_exit_records_cannot_borrow_the_fault_validator(self):
        original = json.loads(safety.MANIFEST.read_text())
        for kind in ('untagged-normal', 'tagged-fault', 'missing-approval'):
            changed = copy.deepcopy(original)
            if kind == 'untagged-normal': changed['exceptions'][-1].pop('approval')
            elif kind == 'tagged-fault': changed['exceptions'][0]['approval'] = safety.NORMAL_SHORT_APPROVAL
            else: changed.pop('normal_exit_approval')
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / 'manifest.json'
                path.write_text(json.dumps(changed))
                with self.assertRaises(harness.HarnessError): safety.inventory(path)

    def test_errors_reject_success_signals_limits_leftovers_and_wrong_bytes(self):
        for case in safety.inventory():
            good = harness.Observation(bytes.fromhex(case.expected['stdout_hex']),
                bytes.fromhex(case.expected['stderr_hex']), 2, None, self.filesystem(case))
            self.assertEqual(safety.errors(case, good), [])
            for mutation in ({'returncode': 0}, {'returncode': -6}, {'limit': 'timeout'},
                             {'leftover_processes': True}, {'stderr': b''},
                             {'stdout': b'unexpected effect\n'},
                             {'filesystem': dict(good.filesystem, forbidden={'type': 'file'})},
                             {'filesystem': {}},
                             {'stderr': good.stderr.replace(b'S00', b'X00')}):
                self.assertTrue(safety.errors(case, dataclasses.replace(good, **mutation)))

    def test_manifest_tampering_does_not_create_an_exemption(self):
        source = json.loads(safety.MANIFEST.read_text())
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'manifest.json'
            for kind in ('duplicate', 'evidence', 'input', 'success', 'missing'):
                changed = json.loads(json.dumps(source))
                if kind == 'duplicate': changed['exceptions'].append(changed['exceptions'][0])
                if kind == 'evidence': changed['exceptions'][0]['evidence_sha256'] = '0' * 64
                if kind == 'input': changed['exceptions'][0]['inputs']['Makefile'] = '0' * 64
                if kind == 'success': changed['exceptions'][0]['expected']['exit_code'] = 0
                if kind == 'missing': changed['exceptions'].pop()
                path.write_text(json.dumps(changed))
                with self.assertRaises(harness.HarnessError): safety.inventory(path)

    def test_variants_keep_exact_errors_under_ignore_keepgoing_and_parallel(self):
        case = safety.inventory()[0]
        cells = list(safety.variants(case))
        self.assertEqual([label for label, _ in cells],
                         ['normal', 'ignore', 'keep-going', 'parallel'])
        self.assertEqual([cell.args[len(case.args):] for _, cell in cells],
                         [(), ('-i',), ('-k',), ('-j4',)])
        self.assertTrue(all(cell.expected == case.expected for _, cell in cells))

    def test_gate_invokes_only_candidate_and_checks_repeated_filesystem_effects(self):
        case = safety.inventory()[0]
        good = harness.Observation(b'', bytes.fromhex(case.expected['stderr_hex']), 2, None, self.filesystem(case))
        candidate = Path('/candidate-only')
        with tempfile.TemporaryDirectory() as tmp:
            for inconsistent in (False, True):
                calls = []
                def execute(cell, root, binary):
                    calls.append(binary)
                    return dataclasses.replace(good, filesystem={'extra': {}}) if inconsistent and len(calls) == 2 else good
                destination = Path(tmp) / str(inconsistent)
                with mock.patch.object(safety, 'inventory', return_value=[case]), \
                     mock.patch.object(harness, 'executable_identity', return_value={'sha256': 'test'}), \
                     mock.patch.object(harness, 'execute', side_effect=execute), \
                     contextlib.redirect_stdout(io.StringIO()):
                    status = safety.main(['--candidate', str(candidate), '--artifacts', str(destination)])
                self.assertEqual(status, int(inconsistent))
                self.assertEqual(calls, [candidate] * 8)
                report = json.loads(next(destination.glob('*/report.json')).read_text())
                self.assertEqual(report['totals']['conformance_credit'], 0)
                self.assertEqual(report['totals']['fail'], int(inconsistent))

    def test_top_level_flows_gate_development_release_and_selfhost(self):
        source = (safety.ROOT / 'Makefile').read_text()
        self.assertIn('test-safety: build\n', source)
        self.assertIn('check: check-core test-oracle test-builtins test-diff test-safety', source)
        self.assertIn('tools/safety.py --candidate "$(M4_RELEASE)"', source)
        self.assertIn('"ORACLE=$(ORACLE)" test-safety', source)


if __name__ == '__main__':
    unittest.main()
