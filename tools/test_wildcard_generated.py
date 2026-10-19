# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Independent anchors, mutation detection and replay for generated filenames."""
import ast
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import conformance
import wildcard_generated as generator


class WildcardGeneratedTests(unittest.TestCase):
    def test_python_311_and_deterministic_bounded_programs(self):
        for filename in (Path(generator.__file__), Path(__file__)):
            ast.parse(filename.read_text(), feature_version=(3, 11))
        program = generator.generate(441031, 0)
        self.assertEqual(program, generator.generate(441031, 0))
        self.assertNotEqual(program, generator.generate(441031, 1))
        self.assertEqual(len(program.patterns), 32)
        self.assertLess(len(program.files), 100)
        self.assertLess(len(program.stdout), 100000)
        self.assertEqual(program.makefile.count(b'$(info q'), 32)

    def test_independent_model_retains_pattern_order_dot_rules_and_directories(self):
        files = {'a.c': b'a', 'b.c': b'b', '.hidden': b'h', 'd/f.c': b'f', 'd/.hidden': b'h'}
        self.assertEqual(generator.query('b* a* *.c', files), b'b.c a.c a.c b.c')
        self.assertEqual(generator.query('.* d/.*', files), b'. .. .hidden d/. d/.. d/.hidden')
        self.assertEqual(generator.query('d* d/*.c [!a]*', files), b'd d/f.c b.c d')

    def test_write_phase_changes_complete_filesystem_and_expected_observations(self):
        program = generator.generate(441031, 4)
        anchor = generator.expected(program)
        self.assertEqual(anchor.filesystem['work/created.txt'], generator.regular(b'generated\n'))
        self.assertEqual(anchor.filesystem['work/Makefile'], generator.regular(program.makefile))
        self.assertEqual(anchor.filesystem['work/protected.txt'], generator.regular(program.files['protected.txt']))
        self.assertIn(b'created.txt', anchor.stdout.split(b'after=[')[1])
        self.assertEqual(anchor.stderr, b'')
        self.assertEqual(anchor.returncode, 0)

    def campaign(self, mutate=None):
        program = generator.generate(441031, 2, 8)
        with tempfile.TemporaryDirectory() as temporary:
            artifacts = Path(temporary) / 'artifacts'
            roots = []
            def execute(case, root, binary):
                roots.append(root)
                observed = generator.expected(program)
                if mutate:
                    mutate(observed)
                return observed
            with mock.patch.object(conformance, 'execute', side_effect=execute), contextlib.redirect_stdout(io.StringIO()):
                status = generator.run([program], Path('/oracle'), Path('/candidate'), artifacts,
                                       {'oracle': {'path': '/oracle'}, 'candidate': {'path': '/candidate'}})
            report = json.loads(next(artifacts.glob('campaign-*.json')).read_text())
            return status, roots, report

    def test_same_alias_and_exact_replay_are_retained(self):
        status, roots, report = self.campaign()
        self.assertEqual(status, 0)
        self.assertEqual(roots[0], roots[1])
        self.assertEqual(report['cases'][0]['replay'], {'seed': 441031, 'start_index': 2, 'cases': 1, 'expressions': 8})
        self.assertEqual(report['unexecuted'], 0)
        self.assertEqual(report['identities']['oracle']['path'], '/oracle')

    def test_agreeing_wrong_engines_fail_output_status_and_filesystem_anchors(self):
        for mutate in (lambda obs: setattr(obs, 'stdout', b'wrong'),
                       lambda obs: setattr(obs, 'returncode', 1),
                       lambda obs: obs.filesystem.pop('work/created.txt'),
                       lambda obs: setattr(obs, 'leftover_processes', True)):
            with self.subTest(mutate=mutate):
                status, _, report = self.campaign(mutate)
                self.assertEqual(status, 1)
                self.assertEqual(report['failed'], 1)
                self.assertIn('artifact', report['cases'][0])

    def test_invalid_controls_are_rejected_before_running_the_oracle(self):
        for args in (['--cases', '0'], ['--cases', '5001'], ['--expressions', '129'], ['--seed', '-1'], ['--start-index', '-1']):
            with mock.patch.object(generator.oracle, 'verify') as verify, contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit):
                    generator.main(['--self-test', *args])
                verify.assert_not_called()


if __name__ == '__main__':
    unittest.main()
