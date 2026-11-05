# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import base64
import ast
from dataclasses import replace
import hashlib
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import Mock

import conformance as h
import job_debug as d


def entry(content):
    return {'type': 'file', 'mode': 0o644, 'size': len(content),
            'sha256': hashlib.sha256(content).hexdigest(),
            'content': base64.b64encode(content).decode()}


class JobDebugTests(unittest.TestCase):
    def setUp(self):
        self.model = d.models()[0]
        self.runtime = Path('/predetermined/runtime')
        self.host = {'make_pid': 1000, 'workers': {'one': {'pid': 2000, 'parent': 1000, 'start': 45}}}
        self.stdout = (b'Putting child 0x2000 (all) PID 2000 on the chain.\n'
                       b'Live child 0x2000 (all) PID 2000 \n'
                       b'Reaping winning child 0x2000 PID 2000 \n'
                       b'Removing child 0x2000 PID 2000 from chain.\n')
        files = {'work/seen': entry(b'anchor\nSTART-one\nEND-one\n'),
                 'work/one.identity': entry(b'pid=2000 ppid=1000 start=45 target=one\n')}
        for suffix in ('verified', 'started', 'done'):
            files['work/one.' + suffix] = entry(b'yes\n')
        self.good = h.Observation(self.stdout, b'', 0, None, files)

    def assess(self, observation, host=None):
        return d.assess(self.model, observation, host or self.host, self.runtime, {})

    def test_exact_positive_and_shared_wrong_streams_fail(self):
        self.assertEqual(self.assess(self.good), [])
        for stdout in (self.stdout.replace(b'Live child', b' Live child'),
                       self.stdout.replace(b'winning', b'losing'),
                       self.stdout.replace(b'(all)', b'(wrong)'),
                       self.stdout.replace(b'2000 \n', b'2000\n'),
                       self.stdout + b'Live child 0x2000 (all) PID 2000 \n',
                       self.stdout.replace(b'0x2000', b'0x0'),
                       self.stdout.replace(b'0x2000', b'0x2001'),
                       self.stdout.replace(b'Live child 0x2000', b'Live child 0x4000')):
            self.assertTrue(self.assess(replace(self.good, stdout=stdout)))

    def test_french_literals_keep_the_same_strict_identity_constraints(self):
        stdout = (
            "Ajout du processus fils 0x2000 (all) PID 2000 à la chaîne.\n"
            "Processus fils actif 0x2000 (all) PID 2000 \n"
            "Récolte du processus fils gagnant 0x2000 PID 2000 \n"
            "Retrait du processus fils 0x2000 PID 2000 de la chaîne.\n"
        ).encode()
        protocol = d.FRENCH
        good = replace(self.good, stdout=stdout)
        self.assertEqual(d.assess(self.model, good, self.host, self.runtime, {}, protocol), [])
        for wrong in (self.stdout, stdout.replace(b'2000 \n', b'2000\n'),
                      stdout.replace(b'actif 0x2000', b'actif 0x4000'),
                      stdout.replace(b'PID 2000', b'PID 2001')):
            observation = replace(good, stdout=wrong)
            self.assertTrue(d.assess(self.model, observation, self.host, self.runtime, {}, protocol))

    def test_shared_wrong_pid_in_stdout_and_worker_wire_cannot_override_host_proof(self):
        files = dict(self.good.filesystem)
        files['work/one.identity'] = entry(b'pid=3000 ppid=1000 start=45 target=one\n')
        bad = replace(self.good, stdout=self.stdout.replace(b'PID 2000', b'PID 3000'), filesystem=files)
        self.assertTrue(self.assess(bad))
        host = {'make_pid': 1000, 'workers': {'one': {'pid': 2000, 'parent': 999, 'start': 45}}}
        self.assertIn('invalid host identity evidence', self.assess(self.good, host))

    def test_complete_files_status_bounds_and_stderr_are_mandatory(self):
        for bad in (replace(self.good, filesystem={}),
                    replace(self.good, filesystem={**self.good.filesystem, 'work/extra': entry(b'extra')}),
                    replace(self.good, stderr=b'extra'), replace(self.good, returncode=2),
                    replace(self.good, limit='timeout'), replace(self.good, leftover_processes=True)):
            self.assertTrue(self.assess(bad))

    def test_controller_path_and_pid_come_only_from_predetermined_inputs(self):
        model = next(m for m in d.models() if m.id == 'jobs-parallel-fifo')
        first = replace(model, events=(model.events[0],))
        good = b'Using jobserver controller fifo:/predetermined/runtime/tmp/GMfifo1000\n'
        self.assertEqual(d.protocol_errors(first, good, self.host, self.runtime), [])
        for wrong in (good.replace(b'1000', b'3000'), good.replace(b'/predetermined', b'/observed'),
                      good.replace(b'fifo:', b''), good + b'extra\n'):
            self.assertTrue(d.protocol_errors(first, wrong, self.host, self.runtime))

    def test_multiline_recipe_requires_same_job_identity_across_distinct_pids(self):
        model = next(m for m in d.models() if m.id == 'jobs-serial-multiline')
        host = {'make_pid': 1000, 'workers': {**self.host['workers'], 'two': {'pid': 2001, 'parent': 1000, 'start': 46}}}
        lines = self.stdout.splitlines(keepends=True)
        stdout = b''.join(lines[:3]) + b''.join(lines[1:]).replace(b'PID 2000', b'PID 2001')
        self.assertEqual(d.protocol_errors(model, stdout, host, self.runtime), [])
        bad = b''.join(lines[:3]) + b''.join(lines[1:]).replace(b'PID 2000', b'PID 2001').replace(b'0x2000', b'0x4000')
        self.assertTrue(d.protocol_errors(model, bad, host, self.runtime))

    def test_observer_receives_actual_child_pid_and_immutable_stdout(self):
        observed = []
        def observe(pid, stdout):
            self.assertIsInstance(stdout, bytes)
            if stdout: observed.append((pid, stdout))
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            out, err, status, limit, leftovers = h.capture(
                [sys.executable, '-c', 'import os,time;print(os.getpid(),flush=True);time.sleep(.08)'],
                root, h.clean_environment(root), 2, observe=observe)
        self.assertEqual((err, status, limit, leftovers), (b'', 0, None, False))
        self.assertTrue(observed)
        self.assertEqual({pid for pid, _ in observed}, {int(out)})

    def test_observer_failure_kills_and_reaps_child(self):
        observed = []
        def fail(pid, _stdout):
            observed.append(pid)
            raise RuntimeError('observer-stop')
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaisesRegex(RuntimeError, 'observer-stop'):
                h.capture([sys.executable, '-c', 'import time;time.sleep(20)'],
                          root, h.clean_environment(root), 2, observe=fail)
        self.assertEqual(len(observed), 1)
        self.assertFalse(Path('/proc', str(observed[0])).exists())

    def test_models_keep_exact_lifecycle_and_real_transport_availability(self):
        models = d.models()
        self.assertEqual(len(models), 9)
        self.assertEqual(len({m.id for m in models}), 9)
        self.assertEqual({m.style for m in models if m.borrow}, {'fifo', 'pipe'})
        self.assertTrue(all(len(m.events) == 18 for m in models if m.style))

    def test_reaped_process_identity_handles_both_linux_disappearance_errors(self):
        tree = ast.parse(d.WORKER.read_text())
        function = next(node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == 'identity')
        for error in (FileNotFoundError(), ProcessLookupError()):
            path = Mock()
            path.return_value.read_bytes.side_effect = error
            namespace = {'Path': path}
            exec(compile(ast.Module(body=[function], type_ignores=[]), str(d.WORKER), 'exec'), namespace)
            self.assertIsNone(namespace['identity'](123))


if __name__ == '__main__':
    unittest.main()
