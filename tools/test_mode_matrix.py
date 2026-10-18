#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Original mode-matrix inventory, anchor, capture, and replay regressions."""
import contextlib
import copy
import dataclasses
import io
import itertools
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import conformance
import mode_matrix as matrix
import oracle


def observation(cell):
    files = {"work/protected-input.txt": matrix.regular_file(matrix.PROTECTED)}
    status, stdout, first, target = matrix.ANCHORS.get((cell.shape, cell.flags), (0, b"", None, None))
    for name, content in (("first.txt", first), ("all", target)):
        if content is not None:
            files["work/" + name] = matrix.regular_file(content)
    return conformance.Observation(stdout, b"", status, None, files)


class MatrixTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.artifacts = self.root / "artifacts"
        self.cell = matrix.select_cells(["mode-ordinary-plain-default"])[0]
        self.reference = observation(self.cell)
        self.identities = {"oracle": {"path": "/oracle", "sha256": "oracle-digest", "profile": {"verified": True}},
                           "candidate": {"path": "/candidate", "sha256": "candidate-digest"}}

    def run_campaign(self, effects, **options):
        arguments = {"cells": (self.cell,), "reference": Path("/oracle"),
                     "candidate": Path("/candidate"), "identities": self.identities,
                     "artifacts": self.artifacts, "profile": Path("/profile.json"),
                     "invocation": ["python3", "tools/mode_matrix.py", "--candidate", "/candidate"]}
        arguments.update(options)
        with mock.patch.object(conformance, "execute", side_effect=effects) as execute, \
             contextlib.redirect_stdout(io.StringIO()) as output:
            status = matrix.run_campaign(**arguments)
        return status, output.getvalue(), execute

    def report(self, artifacts=None):
        folder = artifacts or self.artifacts
        return json.loads(next((folder / "campaigns").glob("*.json")).read_text())

    def test_inventory_is_exact_unique_original_cartesian_product(self):
        cells = matrix.inventory()
        self.assertEqual(len(cells), 512)
        self.assertEqual(len({cell.case_id for cell in cells}), 512)
        flags = {tuple(flag for flag, enabled in zip("nqtis", flags) if enabled)
                 for flags in itertools.product((False, True), repeat=5)}
        expected = {(kind, shape, selected) for kind in ("ordinary", "oneshell")
                    for shape in matrix.SHAPES for selected in flags}
        self.assertEqual({(cell.kind, cell.shape, cell.flags) for cell in cells}, expected)
        self.assertEqual(cells, matrix.inventory())
        self.assertEqual(sum((cell.shape, cell.flags) in matrix.ANCHORS for cell in cells), 10)
        self.assertEqual(self.cell.makefile, b"# Original independent mode-combination probe\nall:\n"
                         b"\t@printf '%s\\n' first > first.txt\n\t@printf '%s\\n' second\n")

    def test_case_filter_replays_exact_cells_in_stable_order(self):
        ids = ["mode-oneshell-errexit-nqtis", self.cell.case_id, self.cell.case_id]
        selected = matrix.select_cells(ids)
        self.assertEqual([cell.case_id for cell in selected], [self.cell.case_id, ids[0]])
        self.assertEqual(selected[1].args, ("-rR", "--no-print-directory", "-n", "-q", "-t", "-i", "-s"))
        with self.assertRaisesRegex(conformance.HarnessError, "unknown case"):
            matrix.select_cells(["mode-ordinary-missing-default"])

    def test_materialization_preserves_source_args_and_protected_input(self):
        case = matrix.materialize(self.cell, self.root)
        self.assertEqual((case.path / "Makefile").read_bytes(), self.cell.makefile)
        self.assertEqual(case.args, self.cell.args)
        self.assertEqual(case.candidate, "required")
        self.assertEqual(case.timeout, 5)
        self.assertEqual((case.path / "protected-input.txt").read_bytes(), matrix.PROTECTED)
        self.assertEqual((case.path / "protected-input.txt").stat().st_mode & 0o777, 0o644)
        self.assertEqual(json.loads((case.path / "matrix.json").read_text()), self.cell.metadata())

    def test_fixed_anchors_detect_wrong_outputs_status_and_files(self):
        for cell in matrix.inventory():
            if (cell.shape, cell.flags) not in matrix.ANCHORS:
                continue
            good = observation(cell)
            self.assertEqual(matrix.anchor_errors(cell, good), [])
            for field, bad in (("stdout", b"wrong"), ("stderr", b"wrong"), ("returncode", 17)):
                with self.subTest(case=cell.case_id, field=field):
                    wrong = copy.deepcopy(good)
                    setattr(wrong, field, bad)
                    self.assertTrue(matrix.anchor_errors(cell, wrong))
            wrong = copy.deepcopy(good)
            wrong.filesystem["work/first.txt"] = matrix.regular_file(b"unexpected")
            self.assertTrue(matrix.anchor_errors(cell, wrong))

    def test_protected_input_checks_content_type_mode_and_links_for_every_cell(self):
        for cell in matrix.inventory():
            good = observation(cell)
            self.assertEqual(matrix.anchor_errors(cell, good), [])
            for change in ({"content": "wrong"}, {"mode": 0o600}, {"type": "symlink"},
                           {"hardlinks": ["work/protected-input.txt", "work/alias"]}):
                wrong = copy.deepcopy(good)
                wrong.filesystem["work/protected-input.txt"].update(change)
                self.assertIn("protected input changed", matrix.anchor_errors(cell, wrong))
        wrong = copy.deepcopy(self.reference)
        del wrong.filesystem["work/protected-input.txt"]
        status, output, _ = self.run_campaign([wrong, copy.deepcopy(wrong)])
        self.assertEqual(status, 1)
        self.assertIn("protected input changed", output)

    def test_success_report_retains_identities_workload_argv_and_source(self):
        status, output, execute = self.run_campaign([self.reference, copy.deepcopy(self.reference)])
        self.assertEqual(status, 0)
        self.assertIn("passed=1, failed=0, unexecuted=0", output)
        self.assertEqual(execute.call_args_list[0].args[1], execute.call_args_list[1].args[1])
        self.assertTrue(execute.call_args_list[0].args[1].is_absolute())
        report = self.report()
        self.assertEqual(report["selected_cases"], [self.cell.case_id])
        self.assertEqual(report["identities"], self.identities)
        self.assertEqual(report["matrix_source_sha256"], conformance.sha256_file(Path(matrix.__file__)))
        self.assertEqual(report["cells"][0]["argv"][1:], list(self.cell.args))
        self.assertEqual(report["inventory_cells"], 512)
        self.assertEqual(report["timeout_seconds"], 5)
        self.assertEqual(report["environment"]["LC_ALL"], "C")
        self.assertTrue(report["cwd"].endswith("/execution/work"))
        self.assertEqual(len(report["workload_sha256"]), 64)
        self.assertEqual(list(self.artifacts.glob("mode-ordinary-*")), [])
        explicit = self.root / "explicit.json"
        self.run_campaign([self.reference, self.reference], report_path=explicit)
        self.assertEqual(json.loads(explicit.read_text())["workload_sha256"], report["workload_sha256"])

    def test_failure_retains_exact_program_raw_bytes_and_replay(self):
        wrong = copy.deepcopy(self.reference)
        wrong.stdout = b"wrong\xff\n"
        status, _, _ = self.run_campaign([self.reference, wrong])
        self.assertEqual(status, 1)
        artifact = Path(self.report()["cells"][0]["artifact"])
        self.assertEqual((artifact / "input/Makefile").read_bytes(), self.cell.makefile)
        self.assertEqual((artifact / "input/protected-input.txt").read_bytes(), matrix.PROTECTED)
        self.assertEqual((artifact / "oracle.stdout").read_bytes(), self.reference.stdout)
        self.assertEqual((artifact / "candidate.stdout").read_bytes(), b"wrong\xff\n")
        metadata = json.loads((artifact / "matrix.json").read_text())
        self.assertEqual(metadata["replay_argv"][-2:], ["--case", self.cell.case_id])
        self.assertIn("/profile.json", metadata["replay_argv"])
        report = json.loads((artifact / "report.json").read_text())
        self.assertEqual(report["identities"], self.identities)
        self.assertIn("stdout", report["differences"])

    def test_streams_status_files_limits_and_background_mutations_all_fail(self):
        mutations = [("stderr", b"unexpected\xff"), ("returncode", 2),
                     ("filesystem", {"work/new": matrix.regular_file(b"new")}),
                     ("limit", "timeout"), ("leftover_processes", True)]
        for index, (field, value) in enumerate(mutations):
            with self.subTest(field=field):
                wrong = copy.deepcopy(self.reference)
                setattr(wrong, field, value)
                status, output, _ = self.run_campaign([self.reference, wrong], artifacts=self.artifacts / str(index))
                self.assertEqual(status, 1)
                self.assertIn(field, output)
        for field, value in (("limit", "timeout"), ("leftover_processes", True)):
            wrong = copy.deepcopy(self.reference)
            setattr(wrong, field, value)
            status, _, _ = self.run_campaign([wrong, copy.deepcopy(wrong)], artifacts=self.artifacts / field)
            self.assertEqual(status, 1)

    def test_partial_capture_error_retains_completed_oracle_observation(self):
        status, output, _ = self.run_campaign([self.reference, conformance.HarnessError("snapshot limit")])
        self.assertEqual(status, 1)
        self.assertIn("ERROR", output)
        artifact = Path(self.report()["cells"][0]["artifact"])
        report = json.loads((artifact / "report.json").read_text())
        self.assertEqual(report["errors"], ["snapshot limit"])
        self.assertEqual(report["observations"]["oracle"]["returncode"], 0)
        self.assertEqual((artifact / "oracle.stdout").read_bytes(), self.reference.stdout)
        self.assertFalse((artifact / "candidate.stdout").exists())

    def test_failure_bound_reports_unexecuted_cells_and_rejects_invalid_budgets(self):
        wrong = copy.deepcopy(self.reference)
        wrong.stderr = b"failure"
        status, output, execute = self.run_campaign([self.reference, wrong], cells=matrix.inventory()[:5], max_failures=1)
        self.assertEqual(status, 1)
        self.assertEqual(execute.call_count, 2)
        self.assertIn("4 cells unexecuted", output)
        report = self.report()
        self.assertEqual((report["executed"], report["passed"], report["failed"], report["unexecuted"]), (1, 0, 1, 4))
        for value in (0, -1, 513, True, 1.5):
            with self.assertRaises(conformance.HarnessError):
                matrix.validate_limit(value)
        for cells in ((), (self.cell, self.cell), (dataclasses.replace(self.cell, makefile=b"wrong"),)):
            with self.assertRaises(conformance.HarnessError):
                self.run_campaign([], cells=cells)

    def test_invalid_selection_and_failed_oracle_verification_cannot_run(self):
        for arguments in (["--max-failures", "0"], ["--case", "unknown"]):
            with mock.patch.object(oracle, "verify") as verify, contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(matrix.main(["--self-test", *arguments]), 2)
            verify.assert_not_called()
        with mock.patch.object(oracle, "verify", side_effect=oracle.OracleError("digest mismatch")), \
             mock.patch.object(matrix, "run_campaign") as run, contextlib.redirect_stderr(io.StringIO()) as error:
            self.assertEqual(matrix.main(["--self-test"]), 2)
        run.assert_not_called()
        self.assertIn("digest mismatch", error.getvalue())

    def test_self_test_cli_records_profile_and_same_oracle_identity(self):
        identity = {"path": "/verified/make", "sha256": "verified"}
        with mock.patch.object(oracle, "verify", return_value={"profile": "checked"}) as verify, \
             mock.patch.object(conformance, "executable_identity", return_value=identity) as identify, \
             mock.patch.object(matrix, "run_campaign", return_value=0) as run, \
             contextlib.redirect_stdout(io.StringIO()):
            status = matrix.main(["--self-test", "--case", self.cell.case_id])
        self.assertEqual(status, 0)
        verify.assert_called_once()
        identify.assert_called_once()
        call = run.call_args.kwargs
        self.assertTrue(call["self_test"])
        self.assertEqual(call["reference"], call["candidate"])
        self.assertEqual(call["identities"]["oracle"]["profile"], {"profile": "checked"})
        self.assertEqual(call["cells"], (self.cell,))
        replay = matrix.replay_arguments(self.cell, Path("/make"), Path("/make"), Path("/profile"), True)
        self.assertIn("--self-test", replay)
        self.assertNotIn("--candidate", replay)


if __name__ == "__main__":
    unittest.main()
