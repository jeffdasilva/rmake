# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Negative controls for process image qualification."""
from dataclasses import replace
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import conformance as h
import process_restarts as p


class ProcessRestartTests(unittest.TestCase):
    def test_case_inventory_and_fixture_identity(self):
        self.assertEqual({row['kind'] for row in p.models()}, {'warning', 'constructor', 'replacement'})
        with patch.object(h, 'sha256_file', return_value='unexpected'):
            with self.assertRaisesRegex(h.HarnessError, 'fixture identity changed'):
                p.models()

    def test_shared_wrong_result_and_unexpected_files_do_not_qualify(self):
        files = {'work/seen': p.file_entry(b'anchor\nREAD\nREAD\nGOAL\n')}
        stderr = b'two exact warnings\n'
        observed = h.Observation(b'', stderr, 0, None, files, False)
        self.assertEqual(p.assess(observed, files, stderr), [])
        for wrong in (
            replace(observed, stdout=b'printed'),
            replace(observed, stderr=b'one warning\n'),
            replace(observed, returncode=2),
            replace(observed, limit='timeout'),
            replace(observed, leftover_processes=True),
            replace(observed, filesystem={**files, 'tmp/leaked': p.file_entry(b'', 0o600)}),
            replace(observed, filesystem={'work/seen': p.file_entry(b'anchor\nREAD\nGOAL\n')}),
        ):
            self.assertTrue(p.assess(wrong, files, stderr))

    def test_replacement_changes_the_alias_without_writing_through_it(self):
        with tempfile.TemporaryDirectory() as temporary:
            runtime = Path(temporary)
            (runtime / 'work').mkdir()
            (runtime / 'bin').mkdir()
            sentinel = runtime / 'reference'
            sentinel.write_bytes(b'preserve reference')
            alias = runtime / 'bin/make'
            alias.symlink_to(sentinel)
            replacement = runtime / 'work/replacement.sh'
            replacement.write_bytes(b'replacement script')
            replacement.chmod(0o755)
            command = [sys.executable, str(p.FIXTURES / 'replace.py')]
            subprocess.run(command, cwd=runtime / 'work', check=True, timeout=5)
            self.assertFalse(alias.is_symlink())
            self.assertFalse(replacement.exists())
            self.assertEqual(alias.read_bytes(), b'replacement script')
            self.assertEqual(alias.stat().st_mode & 0o777, 0o755)
            self.assertEqual(sentinel.read_bytes(), b'preserve reference')

    def test_incorrect_gnu_repeat_is_a_failure_even_when_candidate_matches(self):
        row = p.models()[0]
        with tempfile.TemporaryDirectory() as temporary, patch.object(h, 'capture') as capture:
            root = Path(temporary)
            gnu = root / 'gnu'
            candidate = root / 'candidate'
            seen = []

            def execute(*args):
                runtime = root / 'runtime'
                binary = (runtime / 'bin/make').readlink()
                seen.append(binary)
                (runtime / 'work/generated.mk').write_bytes(b'READY:=yes\n')
                (runtime / 'work/seen').write_bytes(bytes.fromhex(row['seen_hex']))
                warning = (f'make: MAKE_TMPDIR value {runtime / "work/not-dir"}: not a directory\n').encode()
                return b'', warning * (1 if len(seen) == 2 else 2), 0, None, False

            capture.side_effect = execute
            report = p.run([row], gnu, candidate, None, root)
            self.assertEqual(seen, [gnu, gnu, candidate])
            self.assertEqual(report['totals'], {'cases': 1, 'pass': 0, 'fail': 1})
            self.assertEqual([bool(run['errors']) for run in report['records'][0]['observations']],
                             [False, True, False])

    def test_unknown_selector_records_failure_before_oracle_execution(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(p.oracle, 'verify') as verify:
            report = Path(temporary) / 'result.json'
            status = p.main(['--oracle-only', '--case', 'missing', '--report', str(report)])
            verify.assert_not_called()
            self.assertEqual(status, 2)
            self.assertIn('unknown exact process restart case', json.loads(report.read_bytes())['error'])
