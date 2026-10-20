#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Stateful runner checks, including real timestamp-only faulty executables."""
import contextlib
import copy
import dataclasses
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import stateful as runner


class FixtureTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.cases = self.root / "cases"
        self.path = self.cases / "example"
        self.path.mkdir(parents=True)
        (self.path / "Makefile").write_text("out: ;\n")
        self.manifest = ('id = "example"\ndescription = "Original unit fixture"\n'
                         'provenance = "Original rmake test"\n'
                         'initial = [{op="write", path="out", content="x", mtime_ns=1600000000000000000}]\n'
                         '[[steps]]\nname = "noop"\nunchanged = ["out"]\n'
                         'expected = {exit_code=0, stdout="ok\\n", stderr=""}\n')
        self.write_manifest(self.manifest)

    def write_manifest(self, content):
        (self.path / "case.toml").write_text(content)

    def script(self, name, body):
        path = self.root / name
        path.write_text("#!/usr/bin/python3\nimport os,sys\nfrom pathlib import Path\n"
                        "if '--version' in sys.argv:\n print('GNU Make 4.4.1'); raise SystemExit\n" + body)
        path.chmod(0o755)
        return path


class ManifestTests(FixtureTest):
    def test_original_inventory_is_required_and_bounded(self):
        paths = sorted(p for p in (runner.ROOT / "tests/stateful").iterdir() if p.is_dir())
        cases = [runner.load_scenario(p) for p in paths]
        self.assertEqual(len(cases), 40)
        self.assertEqual(sum(len(s.steps) for s in cases), 96)
        self.assertTrue(all(s.case.candidate == "required" for s in cases))
        self.assertTrue(all(s.case.timeout == 5 for s in cases))
        self.assertTrue(all(s.steps and s.anchors for s in cases))

    def test_invalid_manifest_values(self):
        changes = [
            ('id = "example"', 'id = "other"'),
            ('id = "example"', 'id = "../example"'),
            ('description = "Original unit fixture"', 'description = 1'),
            ('[[steps]]', 'unknown = 1\n[[steps]]'),
            ('name = "noop"', 'name = "noop"\nunknown = 1'),
            ('name = "noop"', 'name = "noop"\nargs = "-q"'),
            ('name = "noop"', 'name = "noop"\nargs = [true]'),
            ('name = "noop"', 'name = "noop"\nargs = [' + ','.join('"x"' for _ in range(33)) + ']'),
            ('exit_code=0', 'exit_code=true'),
            ('stdout="ok\\n"', 'stdout=1'),
            ('stderr=""', 'wrong=""'),
            ('unchanged = ["out"]', 'unchanged = "out"'),
            ('unchanged = ["out"]', 'unchanged = ["../out"]'),
            ('op="write"', 'op="shell"'),
            ('path="out"', 'path="/tmp/out"'),
            ('mtime_ns=1600000000000000000', 'mtime_ns=true'),
            ('mtime_ns=1600000000000000000', 'mtime_ns=9999999999999999999'),
            ('content="x", ', ''),
            ('name = "noop"', 'name = "noop"\nmtime_order = [["out","!=","Makefile"]]'),
            ('name = "noop"', 'name = "noop"\nexact_mtime = {"../out"=1}'),
        ]
        for old, new in changes:
            with self.subTest(new=new):
                self.write_manifest(self.manifest.replace(old, new))
                with self.assertRaises(runner.harness.HarnessError):
                    runner.load_scenario(self.path)

    def test_manifest_size_symlinks_and_step_count_are_bounded(self):
        self.write_manifest("#" * (runner.MAX_MANIFEST + 1))
        with self.assertRaises(runner.harness.HarnessError):
            runner.load_scenario(self.path)
        self.write_manifest(self.manifest)
        (self.path / "other").write_text(self.manifest)
        (self.path / "case.toml").unlink()
        (self.path / "case.toml").symlink_to("other")
        with self.assertRaises(runner.harness.HarnessError):
            runner.load_scenario(self.path)
        (self.path / "case.toml").unlink()
        self.write_manifest(self.manifest)
        with mock.patch.object(runner, "MAX_STEPS", 0):
            with self.assertRaises(runner.harness.HarnessError):
                runner.load_scenario(self.path)

    def test_action_paths_and_counts_are_bounded(self):
        for value in ("", "/abs", "../x", "a/../x", "a//x", "a/./x", "a\\x", "a\0x"):
            with self.subTest(value=value):
                with self.assertRaises(runner.harness.HarnessError):
                    runner.relative_path(value)
        with self.assertRaises(runner.harness.HarnessError):
            runner.actions([dict(op="remove", path="x")] * (runner.MAX_ACTIONS + 1))
        with self.assertRaises(runner.harness.HarnessError):
            runner.actions([dict(op="remove", path="x", content="extra")])


class TimestampTests(FixtureTest):
    def test_actions_roundtrip_nanoseconds_and_preserve_content_on_utime(self):
        runner.apply_actions(self.root, (runner.Action("write", "data", "x", runner.BASE_NS),))
        path = self.root / "data"
        self.assertEqual(path.stat().st_mtime_ns, runner.BASE_NS)
        runner.apply_actions(self.root, (runner.Action("utime", "data", mtime_ns=runner.BASE_NS + 1),))
        self.assertEqual(path.stat().st_mtime_ns, runner.BASE_NS + 1)
        self.assertEqual(path.read_bytes(), b"x")
        runner.apply_actions(self.root, (runner.Action("remove", "data"),))
        self.assertFalse(path.exists())

    def test_coarse_timestamp_roundtrip_is_rejected(self):
        path = self.root / "data"
        path.write_bytes(b"x")
        runner.set_time(path, runner.BASE_NS)
        with mock.patch.object(runner.os, "utime"):
            with self.assertRaisesRegex(runner.harness.HarnessError, "preserve requested nanoseconds"):
                runner.set_time(path, runner.BASE_NS + 1)

    def test_actions_cannot_follow_output_symlinks(self):
        work = self.root / "work"
        work.mkdir()
        outside = self.root / "outside"
        outside.mkdir()
        (outside / "data").write_bytes(b"safe")
        (work / "link").symlink_to(outside, target_is_directory=True)
        for op in (runner.Action("write", "link/data", "changed", runner.BASE_NS),
                   runner.Action("utime", "link/data", mtime_ns=runner.BASE_NS),
                   runner.Action("remove", "link/data")):
            with self.subTest(op=op):
                with self.assertRaises(runner.harness.HarnessError):
                    runner.apply_actions(work, (op,))
        self.assertEqual((outside / "data").read_bytes(), b"safe")

    def test_write_action_rejects_special_output_files(self):
        os.mkfifo(self.root / "pipe")
        with self.assertRaises(runner.harness.HarnessError):
            runner.apply_actions(self.root, (runner.Action("write", "pipe", "x", runner.BASE_NS),))

    def test_timestamp_file_limit(self):
        data = self.root / "data"
        data.write_bytes(b"x")
        files = runner.harness.snapshot({"work": self.root})
        with mock.patch.object(runner, "MAX_TIME_FILES", 0):
            with self.assertRaises(runner.harness.HarnessError):
                runner.mtimes({"work": self.root}, files)

    def observed(self, before=None, after=None):
        before = before if before is not None else {"work/out": runner.BASE_NS}
        after = after if after is not None else dict(before)
        files = {"work/out": {"type": "file", "mode": 0o644, "content": "eA==", "size": 1}}
        observation = runner.harness.Observation(b"ok\n", b"", 0, None, copy.deepcopy(files))
        return runner.StepObservation(observation, files, before, after,
                                      runner.time_relations(before, after, (runner.BASE_NS,)))

    def test_comparison_detects_timestamp_drift_without_content_change(self):
        original = self.observed()
        mutant = self.observed(after={"work/out": runner.BASE_NS + 1})
        self.assertEqual(runner.harness.differences(original.observation, mutant.observation), [])
        self.assertIn("step-1.mtime-relations", runner.differences([original], [mutant]))

    def test_deterministic_anchor_equality_is_not_normalized_away(self):
        before = {"work/out": runner.BASE_NS - 1}
        self.assertNotEqual(runner.time_relations(before, {"work/out": runner.BASE_NS}, (runner.BASE_NS,)),
                            runner.time_relations(before, {"work/out": runner.BASE_NS + 1}, (runner.BASE_NS,)))

    def test_wallclock_absolute_values_and_unrelated_write_order_are_not_compared(self):
        before = {"work/input": runner.BASE_NS}
        left = {**before, "work/a": runner.BASE_NS + 10, "work/b": runner.BASE_NS + 10}
        right = {**before, "work/a": runner.BASE_NS + 20, "work/b": runner.BASE_NS + 21}
        self.assertEqual(runner.time_relations(before, left, (runner.BASE_NS,)),
                         runner.time_relations(before, right, (runner.BASE_NS,)))
        declared = (("a", "<=", "b"),)
        self.assertEqual(runner.time_relations(before, left, (runner.BASE_NS,), declared),
                         runner.time_relations(before, right, (runner.BASE_NS,), declared))
        right["work/a"] += 2
        self.assertNotEqual(runner.time_relations(before, left, (runner.BASE_NS,), declared),
                            runner.time_relations(before, right, (runner.BASE_NS,), declared))

    def test_other_mutations_and_identical_limits_fail(self):
        original = self.observed()
        changes = {"stdout": b"wrong", "stderr": b"error", "returncode": 2,
                   "filesystem": {}, "leftover_processes": True, "limit": "timeout"}
        for name, value in changes.items():
            mutant = copy.deepcopy(original)
            setattr(mutant.observation, name, value)
            with self.subTest(field=name):
                self.assertTrue(runner.differences([original], [mutant]))
        limited = copy.deepcopy(original)
        limited.observation.limit = "timeout"
        self.assertIn("step-1.execution-limit", runner.differences([limited], [limited]))
        self.assertIn("step-count", runner.differences([original], []))

    def test_expected_unchanged_and_exact_time_are_independently_checked(self):
        scenario = runner.load_scenario(self.path)
        original = self.observed()
        self.assertEqual(runner.expectation_errors(scenario, [original]), [])
        mutated = self.observed(after={"work/out": runner.BASE_NS + 1})
        self.assertTrue(runner.expectation_errors(scenario, [mutated]))
        step = dataclasses.replace(scenario.steps[0], unchanged=(), exact_mtime={"out": runner.BASE_NS})
        scenario = dataclasses.replace(scenario, steps=(step,))
        self.assertTrue(runner.expectation_errors(scenario, [mutated]))
        step = dataclasses.replace(step, exact_mtime={}, mtime_order=(("out", "=", "absent"),))
        self.assertTrue(runner.expectation_errors(dataclasses.replace(scenario, steps=(step,)), [original]))


class ExecutionTests(FixtureTest):
    def test_persistent_steps_and_identical_alias_reset_between_engines(self):
        body = ("p=Path('state')\ncount=int(p.read_text())+1 if p.exists() else 1\n"
                "p.write_text(str(count))\nos.utime(p,ns=(1600000000000000000+count,)*2)\n"
                "print(sys.argv[0]+'|'+str(count))\n")
        reference = self.script("reference", body)
        candidate = self.script("candidate", body)
        scenario = runner.load_scenario(self.path)
        scenario = dataclasses.replace(scenario, steps=(scenario.steps[0], scenario.steps[0]))
        workspace = self.root / "workspace"
        workspace.mkdir()
        left = runner.execute(scenario, workspace, reference)
        right = runner.execute(scenario, workspace, candidate)
        self.assertEqual(runner.differences(left, right), [])
        self.assertEqual(left[0].observation.stdout, f"{workspace}/bin/make|1\n".encode())
        self.assertEqual(left[1].observation.stdout, f"{workspace}/bin/make|2\n".encode())
        self.assertEqual(left[1].before_mtime_ns["work/state"], runner.BASE_NS + 1)
        self.assertEqual(left[1].after_mtime_ns["work/state"], runner.BASE_NS + 2)
        self.assertFalse((self.path / "state").exists())

    def test_timeout_stops_remaining_steps(self):
        binary = self.script("slow", "import time\ntime.sleep(30)\n")
        scenario = runner.load_scenario(self.path)
        scenario = dataclasses.replace(scenario, case=dataclasses.replace(scenario.case, timeout=0.05),
                                       steps=(scenario.steps[0], scenario.steps[0]))
        workspace = self.root / "workspace"
        workspace.mkdir()
        observed = runner.execute(scenario, workspace, binary)
        self.assertEqual(len(observed), 1)
        self.assertEqual(observed[0].observation.limit, "timeout")
        self.assertTrue(runner.expectation_errors(scenario, observed))

    def test_real_noop_write_and_one_ns_mutants_fail_with_reproduction_evidence(self):
        reference = self.script("reference", "print('ok')\n")
        for index, mutation in enumerate(("p=Path('out'); p.write_bytes(p.read_bytes())\n",
                                         "os.utime('out',ns=(1600000000000000001,)*2)\n")):
            with self.subTest(mutation=mutation):
                candidate = self.script(f"candidate-{index}", mutation + "print('ok')\n")
                artifacts = self.root / f"artifacts-{index}"
                with mock.patch.object(runner.oracle, "verify", return_value={"profile_id": "unit"}) as verify:
                    with contextlib.redirect_stdout(io.StringIO()):
                        status = runner.main(["--oracle", str(reference), "--candidate", str(candidate),
                                              "--cases", str(self.cases), "--artifacts", str(artifacts)])
                self.assertEqual(status, 1)
                verify.assert_called_once()
                evidence = next(p for p in artifacts.iterdir() if (p / "report.json").is_file())
                report = json.loads((evidence / "report.json").read_text())
                self.assertEqual(report["differences"], ["step-1.mtime-relations"])
                self.assertEqual(report["oracle"][0]["after_mtime_ns"]["work/out"], runner.BASE_NS)
                self.assertIn("sha256", report["identities"]["candidate"])
                self.assertIn("LC_ALL", report["environment"])
                self.assertEqual(report["steps"][0]["args"], list(runner.DEFAULT_ARGS))
                self.assertTrue((evidence / "input/example/Makefile").is_file())
                self.assertTrue((evidence / "input/example/case.toml").is_file())
                self.assertEqual((evidence / "candidate-01.stdout").read_bytes(), b"ok\n")
                self.assertIn("--cases", report["replay"])

    def test_snapshot_failure_preserves_completed_steps_partial_bytes_and_artifact(self):
        binary = self.script("bytes", "os.write(1, bytes([255, 0]))\n")
        scenario = runner.load_scenario(self.path)
        scenario = dataclasses.replace(scenario, steps=(scenario.steps[0], scenario.steps[0]))
        workspace = self.root / "workspace"
        workspace.mkdir()
        original_snapshot = runner.harness.snapshot
        calls = 0

        def fail_second_after(roots):
            nonlocal calls
            calls += 1
            if calls == 5:
                raise runner.harness.HarnessError("deliberate observation bound")
            return original_snapshot(roots)

        with mock.patch.object(runner.harness, "snapshot", side_effect=fail_second_after):
            with self.assertRaises(runner.ExecutionFailure) as raised:
                runner.execute(scenario, workspace, binary)
        failure = raised.exception
        self.assertEqual(len(failure.completed), 1)
        self.assertEqual(failure.partial["phase"], "observe-after")
        self.assertEqual(failure.partial["stdout_base64"], "/wA=")
        self.assertEqual(failure.partial["before_mtime_ns"]["work/out"], runner.BASE_NS)
        artifacts = self.root / "errors"
        with mock.patch.object(runner.oracle, "verify", return_value={"profile_id": "unit"}):
            with mock.patch.object(runner, "execute", side_effect=failure):
                with contextlib.redirect_stdout(io.StringIO()):
                    status = runner.main(["--oracle", str(binary), "--self-test", "--cases", str(self.cases),
                                          "--artifacts", str(artifacts)])
        self.assertEqual(status, 1)
        report = json.loads((next(p for p in artifacts.iterdir() if (p / "report.json").is_file()) / "report.json").read_text())
        self.assertIn("deliberate observation bound", report["execution_failure"]["message"])
        self.assertEqual(report["execution_failure"]["partial"]["stdout_base64"], "/wA=")
        self.assertEqual(len(report["oracle"]), 1)
        self.assertIn("input_sha256", report)

    def test_success_campaign_records_configuration_and_identity(self):
        binary = self.script("okay", "print('ok')\n")
        report_path = self.root / "success.json"
        with mock.patch.object(runner.oracle, "verify", return_value={"profile_id": "unit"}):
            with contextlib.redirect_stdout(io.StringIO()):
                result = runner.main(["--oracle", str(binary), "--self-test", "--cases", str(self.cases),
                                      "--report", str(report_path)])
        self.assertEqual(result, 0)
        report = json.loads(report_path.read_text())
        self.assertEqual(report["result"], "PASS")
        self.assertEqual(report["steps_per_engine"], 1)
        self.assertEqual(report["cases"][0]["step_configuration"][0]["expected"]["stdout"], "ok\n")
        self.assertEqual(report["cases"][0]["initial"][0]["mtime_ns"], runner.BASE_NS)
        self.assertEqual(report["cases"][0]["oracle_steps"], 1)
        self.assertEqual(report["cases"][0]["candidate_steps"], 1)
        self.assertIn("input/Makefile", report["cases"][0]["file_sha256"])
        self.assertIn("sha256", report["identities"]["oracle"])

    def test_oracle_verification_failure_prevents_execution(self):
        with mock.patch.object(runner.oracle, "verify", side_effect=runner.oracle.OracleError("wrong digest")):
            with mock.patch.object(runner.harness, "executable_identity") as identity:
                with contextlib.redirect_stderr(io.StringIO()):
                    result = runner.main(["--self-test", "--cases", str(self.cases)])
        self.assertEqual(result, 2)
        identity.assert_not_called()

    def test_unknown_case_and_too_many_selectors_fail_before_oracle(self):
        for extra in (["--case", "absent"], ["--case", "example"] * (runner.MAX_CASES + 1)):
            with mock.patch.object(runner.oracle, "verify") as verify:
                with contextlib.redirect_stderr(io.StringIO()):
                    result = runner.main(["--self-test", "--cases", str(self.cases), *extra])
                self.assertEqual(result, 2)
                verify.assert_not_called()


if __name__ == "__main__":
    unittest.main()
