# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
from dataclasses import replace
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import conformance as h
import locale_profiles as p


class LocaleProfilesTests(unittest.TestCase):
    def test_original_matrix_preserves_all_phases_priorities_and_raw_names(self):
        data, names, models = p.models()
        self.assertEqual(len(models), 183)
        self.assertEqual(len({model['id'] for model in models}), 183)
        self.assertIn(b'\xff', names)
        self.assertIn('e\u0301'.encode(), names)
        self.assertEqual(sum(model['id'].startswith('recursive-') for model in models), 8)
        self.assertEqual(sum(model['id'].startswith('late-') for model in models), 40)
        self.assertEqual(sum(model['id'].startswith('invalid-') for model in models), 30)
        self.assertEqual(data['schema_version'], 1)

    def test_shared_wrong_streams_status_and_process_outcomes_cannot_pass(self):
        _, names, models = p.models()
        model = models[0]
        files = p.expected_files(bytes.fromhex(model['source_hex']), names)
        good = h.Observation(bytes.fromhex(model['stdout_hex']), b'', 0, None, files)
        self.assertEqual(p.assess(model, good, files), [])
        for bad in (replace(good, stdout=b'wrong'), replace(good, stderr=b'warning'),
                    replace(good, returncode=2), replace(good, limit='timeout'),
                    replace(good, leftover_processes=True)):
            self.assertTrue(p.assess(model, bad, files))

    def test_complete_filesystem_is_independent_of_both_engine_results(self):
        _, names, models = p.models()
        model = models[0]
        expected = p.expected_files(bytes.fromhex(model['source_hex']), names)
        first = 'work/names/' + names[0].decode()
        for mutation in ('extra', 'missing', 'content', 'mode'):
            files = json.loads(json.dumps(expected))
            if mutation == 'extra': files['work/extra'] = p.file_entry(b'extra')
            elif mutation == 'missing': del files[first]
            elif mutation == 'content': files[first] = p.file_entry(b'changed')
            else: files[first]['mode'] = 0o600
            bad = h.Observation(bytes.fromhex(model['stdout_hex']), b'', 0, None, files)
            self.assertTrue(p.assess(model, bad, expected))

    def test_empty_duplicate_or_escaping_models_are_not_successful_suites(self):
        original = json.loads(p.MODELS.read_text())
        for defect in ('empty', 'duplicate', 'path', 'nul'):
            data = json.loads(json.dumps(original))
            if defect == 'empty': data['models'] = []
            elif defect == 'duplicate': data['models'].append(data['models'][0])
            elif defect == 'path': data['filenames_hex'].append(b'../escape'.hex())
            else: data['filenames_hex'].append(b'raw\0name'.hex())
            with tempfile.TemporaryDirectory() as tmp:
                manifest = Path(tmp) / 'models.json'; manifest.write_text(json.dumps(data))
                with patch.object(p, 'MODELS', manifest), self.assertRaises(h.HarnessError):
                    p.models()

    def test_unknown_selection_never_verifies_or_executes_a_binary(self):
        with patch.object(p.oracle, 'verify') as verify, patch.object(h, 'capture') as execute:
            with self.assertRaises(SystemExit) as error:
                p.main(['--case', 'unknown', '--report', '/not-written'])
            self.assertEqual(error.exception.code, 2)
            verify.assert_not_called(); execute.assert_not_called()


if __name__ == '__main__':
    unittest.main()
