#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Tests for the independent runner, including deliberately corrupted outcomes."""
import contextlib
import copy
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

import conformance as runner


class ManifestTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.case = self.root / "example"
        self.case.mkdir()
        (self.case / "Makefile").write_text("all:\n\t@printf 'ok\\n'\n")
        self.base = 'id = "example"\nrequirement = "rule.explicit"\ncandidate = "pending"\n'
        (self.case / "case.toml").write_text(self.base)

    def write(self, extra):
        (self.case / "case.toml").write_text(self.base + extra)

    def test_defaults_and_expectations(self):
        self.write('[expected]\nexit_code = 0\nstdout = "ok\\n"\n')
        case = runner.load_case(self.case)
        self.assertEqual(case.args, ("-rR", "--no-print-directory"))
        self.assertEqual(case.expected["stdout"], "ok\n")
        self.assertEqual(case.candidate, "pending")

    def test_dot_make_variable_environment_retains_reserved_name_guards(self):
        self.write('[environment]\n".LIBPATTERNS" = "lib%.a"\n')
        self.assertEqual(runner.load_case(self.case).environment, {'.LIBPATTERNS': 'lib%.a'})
        for key in ('HOME', 'PATH', 'LC_ALL', 'name=bad', 'bad-name', '1bad'):
            self.write('[environment]\n' + json.dumps(key) + ' = "x"\n')
            with self.assertRaises(runner.HarnessError):
                runner.load_case(self.case)

    def test_hex_expectations_preserve_json_safe_strings(self):
        self.write('[expected]\nstdout_hex = "00fF80"\nstderr_hex = ""\n')
        case = runner.load_case(self.case)
        self.assertEqual(case.expected, {"stdout_hex": "00fF80", "stderr_hex": ""})
        self.assertEqual(json.loads(json.dumps(case.expected)), case.expected)
        observed = runner.Observation(b"\x00\xff\x80", b"", 0, None, {})
        self.assertEqual(runner.expectation_errors(case, observed), [])

    def test_hex_expectations_reject_non_ascii_odd_and_whitespace_values(self):
        for value in ("0", "abc", "gg", "ff ff", "ff\n", "０１", "é0", "0x00", 0, True, []):
            with self.subTest(value=value):
                self.write('[expected]\nstderr_hex = ' + json.dumps(value) + '\n')
                with self.assertRaises(runner.HarnessError):
                    runner.load_case(self.case)

    def test_text_and_hex_expectations_for_same_stream_are_exclusive(self):
        for stream in ("stdout", "stderr"):
            with self.subTest(stream=stream):
                self.write(f'[expected]\n{stream} = ""\n{stream}_hex = ""\n')
                with self.assertRaisesRegex(runner.HarnessError, "mutually exclusive"):
                    runner.load_case(self.case)
        self.write('[expected]\nstdout = "ok"\nstderr_hex = "ff"\n')
        runner.load_case(self.case)

    def test_unknown_fields_and_invalid_values(self):
        for extra in ('typo = true\n', 'timeout_seconds = 0\n', 'timeout_seconds = true\n',
                      'timeout_seconds = nan\n', 'args = "all"\n', 'args = [1]\n',
                      'output = "pty"\n', '[environment]\nHOME = "/tmp"\n',
                      '[environment]\nVALUE = 1\n', '[expected]\nexit_code = true\n',
                      '[expected]\nstdout = 1\n', '[expected]\nunknown = 0\n',
                      'steps = []\n'):
            with self.subTest(extra=extra):
                self.write(extra)
                with self.assertRaises(runner.HarnessError):
                    runner.load_case(self.case)

    def test_id_must_match_directory(self):
        (self.case / "case.toml").write_text('id = "other"\nrequirement = "rule"\n')
        with self.assertRaises(runner.HarnessError):
            runner.load_case(self.case)

    def test_invalid_id_cannot_escape_artifact_path(self):
        (self.case / "case.toml").write_text('id = "../other"\nrequirement = "rule"\n')
        with self.assertRaises(runner.HarnessError):
            runner.load_case(self.case)

    def test_escape_symlinks_rejected_without_reading_target(self):
        for destination in ("../../outside", "/etc/passwd"):
            link = self.case / "link"
            link.symlink_to(destination)
            with self.assertRaises(runner.HarnessError):
                runner.load_case(self.case)
            link.unlink()

    def test_internal_symlink_accepted(self):
        (self.case / "link").symlink_to("Makefile")
        runner.load_case(self.case)

    def test_cyclic_symlink_rejected(self):
        (self.case / "link").symlink_to("link")
        with self.assertRaises(runner.HarnessError):
            runner.load_case(self.case)

    def test_special_input_file_rejected(self):
        os.mkfifo(self.case / "pipe")
        with self.assertRaises(runner.HarnessError):
            runner.load_case(self.case)

    def test_symlink_manifest_rejected(self):
        (self.case / "case.toml").rename(self.case / "data.toml")
        (self.case / "case.toml").symlink_to("data.toml")
        with self.assertRaises(runner.HarnessError):
            runner.load_case(self.case)


class ObservationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def test_snapshot_tracks_content_modes_and_link_topology(self):
        (self.root / "a").write_bytes(b"data\x00\xff")
        (self.root / "a").chmod(0o640)
        os.link(self.root / "a", self.root / "b")
        (self.root / "external").symlink_to("/does/not/exist")
        observed = runner.snapshot({"work": self.root})
        self.assertEqual(observed["work/a"]["mode"], 0o640)
        self.assertEqual(observed["work/a"]["hardlinks"], ["work/a", "work/b"])
        self.assertEqual(observed["work/external"],
                         {"type": "symlink", "mode": 0o777, "target": "/does/not/exist"})
        before = observed["work/a"]
        os.utime(self.root / "a", ns=(1, 1))
        self.assertEqual(before, runner.snapshot({"work": self.root})["work/a"])
        (self.root / "a").write_bytes(b"changed")
        self.assertNotEqual(before, runner.snapshot({"work": self.root})["work/a"])

    def test_snapshot_never_follows_output_directory_symlink(self):
        (self.root / "escape").symlink_to("/proc")
        observed = runner.snapshot({"work": self.root})
        self.assertEqual(set(observed), {"work", "work/escape"})

    def test_snapshot_bounds(self):
        (self.root / "large").write_bytes(b"1234")
        with mock.patch.object(runner, "MAX_FILE_BYTES", 3):
            with self.assertRaises(runner.HarnessError):
                runner.snapshot({"work": self.root})
        with mock.patch.object(runner, "MAX_FILES", 1):
            with self.assertRaises(runner.HarnessError):
                runner.snapshot({"work": self.root})

    def test_comparator_mutants(self):
        original = runner.Observation(b"out\n", b"err\n", 0, None,
                                      {"out": {"mode": 0o644, "content": "eA=="}})
        self.assertEqual(runner.differences(original, copy.deepcopy(original)), [])
        mutants = {"stdout": b"out", "stderr": b"err!\n", "returncode": 2,
                   "limit": "timeout", "filesystem": {}, "leftover_processes": True}
        for field, changed in mutants.items():
            with self.subTest(field=field):
                mutant = copy.deepcopy(original)
                setattr(mutant, field, changed)
                self.assertIn(field, runner.differences(original, mutant))
        for field, changed in (("mode", 0o600), ("content", "eQ==")):
            mutant = copy.deepcopy(original)
            mutant.filesystem["out"][field] = changed
            self.assertIn("filesystem", runner.differences(original, mutant))

    def test_identical_timeouts_are_not_success(self):
        observed = runner.Observation(b"", b"", -9, "timeout", {})
        self.assertIn("execution-limit", runner.differences(observed, observed))

    def test_clean_environment_drops_inherited_makeflags_and_secrets(self):
        with mock.patch.dict(os.environ, {"MAKEFLAGS": "--bad", "SECRET_TOKEN": "do-not-inherit"}):
            environment = runner.clean_environment(self.root)
        self.assertNotIn("MAKEFLAGS", environment)
        self.assertNotIn("SECRET_TOKEN", environment)
        self.assertEqual(environment["LC_ALL"], "C")

    def test_hex_expected_bytes_detect_utf8_replacement_and_missing_nul(self):
        case = runner.Case(self.root, "case", "requirement", "required", (), 1, {},
                           {"stdout_hex": "00", "stderr_hex": "ff0a", "exit_code": 2})
        correct = runner.Observation(b"\x00", b"\xff\n", 2, None, {})
        self.assertEqual(runner.expectation_errors(case, correct), [])
        changed = runner.Observation(b"", "�\n".encode(), 2, None, {})
        self.assertEqual(runner.expectation_errors(case, changed),
                         ["oracle disagrees with expected.stdout_hex",
                          "oracle disagrees with expected.stderr_hex"])

    def test_expected_bytes_and_status_checked_independently(self):
        case = runner.Case(self.root, "case", "requirement", "pending", (), 1, {},
                           {"stdout": "ok\n", "stderr": "", "exit_code": 0})
        observed = runner.Observation(b"wrong\n", b"", 2, None, {})
        errors = runner.expectation_errors(case, observed)
        self.assertIn("oracle disagrees with expected.stdout", errors)
        self.assertIn("oracle disagrees with expected.exit_code", errors)


class ProcessTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for name in ("bin", "home", "tmp"):
            (self.root / name).mkdir()

    def run_script(self, script, *, timeout=2, merged=False):
        return runner.capture([sys.executable, "-c", script], self.root,
                              runner.clean_environment(self.root), timeout, merged)

    def test_separate_and_merged_descriptors(self):
        script = "import os; os.write(1,b'A'); os.write(2,b'B'); os.write(1,b'C')"
        out, err, status, limit, leftovers = self.run_script(script)
        self.assertEqual((out, err, status, limit, leftovers), (b"AC", b"B", 0, None, False))
        out, err, status, limit, leftovers = self.run_script(script, merged=True)
        self.assertEqual((out, err, status, limit, leftovers), (b"ABC", b"", 0, None, False))

    def test_nonzero_status_and_signals_not_collapsed(self):
        self.assertEqual(self.run_script("raise SystemExit(7)")[2], 7)
        self.assertEqual(self.run_script("import os,signal; os.kill(os.getpid(),signal.SIGTERM)")[2], -15)

    def test_timeout_kills_process_group(self):
        result = self.run_script("import subprocess,time; subprocess.Popen(['sleep','30']); time.sleep(30)",
                                 timeout=0.15)
        self.assertEqual(result[3], "timeout")
        self.assertEqual(result[2], -9)

    def test_output_is_bounded(self):
        with mock.patch.object(runner, "MAX_OUTPUT", 128):
            result = self.run_script("import os; os.write(1,b'x'*8192)")
        self.assertEqual(result[3], "output")
        self.assertEqual(len(result[0]) + len(result[1]), 128)

    def test_background_group_is_detected_and_cleaned(self):
        result = self.run_script("import subprocess; subprocess.Popen(['sleep','30'])")
        self.assertTrue(result[4])
        self.assertIsNone(result[3])

    def test_workspace_reset_uses_same_alias_and_drops_old_outputs(self):
        source = self.root / "fixture"
        source.mkdir()
        (source / "Makefile").write_text("original")
        (source / "case.toml").write_text("do not copy")
        os.link(source / "Makefile", source / "linked-input")
        case = runner.Case(source, "fixture", "req", "pending", (), 1, {}, {})
        executable = Path(sys.executable).resolve()
        runner.reset_workspace(case, self.root, executable)
        alias = self.root / "bin" / "make"
        self.assertEqual(alias.resolve(), executable)
        self.assertFalse((self.root / "work" / "case.toml").exists())
        self.assertEqual((self.root / "work" / "Makefile").stat().st_ino,
                         (self.root / "work" / "linked-input").stat().st_ino)
        self.assertNotEqual((source / "Makefile").stat().st_ino,
                            (self.root / "work" / "Makefile").stat().st_ino)
        (self.root / "work" / "output").write_text("first engine")
        (self.root / "work" / "Makefile").write_text("changed")
        runner.reset_workspace(case, self.root, executable)
        self.assertFalse((self.root / "work" / "output").exists())
        self.assertEqual((self.root / "work" / "Makefile").read_text(), "original")
        self.assertEqual(alias.resolve(), executable)

    def test_wrong_oracle_version_is_rejected(self):
        executable = self.root / "wrong-make"
        executable.write_text("#!/bin/sh\nprintf 'GNU Make 4.3\\n'\n")
        executable.chmod(0o755)
        with self.assertRaisesRegex(runner.HarnessError, "4.4.1"):
            runner.executable_identity(executable, oracle=True)

    def test_artifacts_preserve_raw_bytes_and_inputs(self):
        source = self.root / "source"
        source.mkdir()
        (source / "Makefile").write_text("original")
        case = runner.Case(source, "case", "req", "pending", (), 1, {}, {})
        reference = runner.Observation(b"\xff\n", b"", 0, None, {})
        candidate = runner.Observation(b"\x00\n", b"error", 2, None, {})
        saved = runner.save_artifact(self.root / "artifacts", case, self.root,
                                     {"oracle": {"sha256": "identity"}}, reference,
                                     candidate, ["stdout"], "PENDING")
        self.assertEqual((saved / "oracle.stdout").read_bytes(), b"\xff\n")
        self.assertEqual((saved / "candidate.stdout").read_bytes(), b"\x00\n")
        self.assertEqual((saved / "input" / "Makefile").read_text(), "original")
        self.assertEqual(json.loads((saved / "report.json").read_text())["status"], "PENDING")


class ReportingGateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.case = self.root / "example"
        self.case.mkdir()
        (self.case / "Makefile").write_text("all:\n")
        self.reference = runner.Observation(b"ok\n", b"", 0, None, {})

    def run_gate(self, observed, candidate="pending", self_test=False, repeat=1):
        (self.case / "case.toml").write_text(
            'id = "example"\nrequirement = "rule.explicit"\ncandidate = "' + candidate +
            '"\n[expected]\nexit_code = 0\nstdout = "ok\\n"\n')
        identity = {"path": "/bin/true", "sha256": "identity"}
        with mock.patch.object(runner.reference_oracle, "verify", return_value={"profile_id": "test"}), \
             mock.patch.object(runner, "executable_identity", return_value=identity), \
             mock.patch.object(runner, "execute", side_effect=[self.reference, observed] * repeat), \
             mock.patch.object(runner, "save_artifact", return_value=self.root / "evidence"), \
             contextlib.redirect_stdout(io.StringIO()) as output:
            arguments = ["--cases", str(self.root)]
            arguments += ["--self-test"] if self_test else ["--candidate", "/bin/true"]
            if repeat != 1:
                arguments += ["--repeat", str(repeat), "--case", "example"]
            status = runner.main(arguments)
        return status, output.getvalue()

    def test_repeated_observations_remain_independent_and_exact(self):
        status, output = self.run_gate(copy.deepcopy(self.reference), candidate="required", repeat=3)
        self.assertEqual(status, 0)
        self.assertIn("pass=3", output)
        self.assertIn("run 3/3", output)
        changed = runner.Observation(b"wrong\n", b"", 0, None, {})
        status, output = self.run_gate(changed, candidate="required", repeat=3)
        self.assertEqual(status, 1)
        self.assertIn("fail=3", output)

    def test_repetition_budget_and_exact_selection_are_required(self):
        for arguments in (["--repeat", "0"], ["--repeat", "1001"], ["--repeat", "2"]):
            with self.subTest(arguments=arguments), \
                 mock.patch.object(runner.reference_oracle, "verify") as verify, \
                 mock.patch.object(runner, "execute") as execute, \
                 contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(runner.main(["--self-test", *arguments]), 2)
                verify.assert_not_called()
                execute.assert_not_called()

    def test_direct_cli_rejects_unverified_oracle_profile(self):
        with mock.patch.object(runner.reference_oracle, "verify",
                               side_effect=runner.reference_oracle.OracleError("profile digest mismatch")), \
             mock.patch.object(runner, "execute") as execute, \
             contextlib.redirect_stderr(io.StringIO()) as error:
            status = runner.main(["--self-test"])
        self.assertEqual(status, 2)
        self.assertIn("profile digest mismatch", error.getvalue())
        execute.assert_not_called()

    def test_pending_mismatch_is_explicit_and_not_a_pass(self):
        observed = runner.Observation(b"", b"unimplemented\n", 2, None, {})
        status, output = self.run_gate(observed)
        self.assertEqual(status, 0)
        self.assertIn("pending=1", output)
        self.assertIn("pass=0", output)
        self.assertIn("not compatibility passes", output)

    def test_pending_match_fails_as_unexpected_pass(self):
        status, output = self.run_gate(copy.deepcopy(self.reference))
        self.assertEqual(status, 1)
        self.assertIn("XPASS", output)

    def test_required_mismatch_fails(self):
        observed = runner.Observation(b"wrong", b"", 0, None, {})
        status, output = self.run_gate(observed, candidate="required")
        self.assertEqual(status, 1)
        self.assertIn("FAIL", output)

    def test_self_check_passes_despite_pending_manifest(self):
        status, output = self.run_gate(copy.deepcopy(self.reference), self_test=True)
        self.assertEqual(status, 0)
        self.assertIn("pass=1", output)

    def test_pending_timeout_is_infrastructure_failure(self):
        observed = runner.Observation(b"", b"", -9, "timeout", {})
        status, output = self.run_gate(observed)
        self.assertEqual(status, 1)
        self.assertIn("FAIL", output)


if __name__ == "__main__":
    unittest.main()
