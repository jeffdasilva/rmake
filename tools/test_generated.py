#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Independent tests for deterministic generation and strict differential gates."""
import contextlib
import copy
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import conformance
import generated
import oracle


def observation(program):
    expected = dict(program.expected_rows)
    output = b"".join(f"{index:04d}:".encode() + expected.get(index, b"some value") + b"\n"
                      for index in range(len(program.expressions)))
    return conformance.Observation(output, b"", 0, None,
                                   {"work/result.bin": {"content": output.hex()}})


class GenerationTests(unittest.TestCase):
    def test_same_seed_and_index_produce_identical_program_and_metadata(self):
        left = generated.generate_program(44101, 17, 32)
        right = generated.generate_program(44101, 17, 32)
        self.assertEqual(left, right)
        self.assertEqual(left.metadata(), right.metadata())

    def test_case_generation_is_independent_of_preceding_cases(self):
        wanted = generated.generate_program(9, 5, 32)
        for index in range(5):
            generated.generate_program(9, index, 32)
        self.assertEqual(wanted, generated.generate_program(9, 5, 32))

    def test_seed_and_index_select_different_programs(self):
        first = generated.generate_program(44101, 0, 64)
        self.assertNotEqual(first.makefile, generated.generate_program(44102, 0, 64).makefile)
        self.assertNotEqual(first.makefile, generated.generate_program(44101, 1, 64).makefile)

    def test_program_contains_binary_inputs_and_has_safe_shell_arguments(self):
        program = generated.generate_program(44101, 0, 256)
        self.assertTrue(any(byte >= 128 for byte in program.makefile))
        for expression in program.expressions:
            self.assertNotIn(b"'", expression)
            self.assertNotIn(b"\n", expression)
            self.assertNotIn(b"\r", expression)
            self.assertNotIn(b"\0", expression)
        self.assertIn(b" > result.bin\n", program.makefile)
        self.assertIn(b"\t@cat result.bin\n", program.makefile)

    def test_metadata_retains_exact_expressions_and_seed(self):
        program = generated.generate_program(1234, 7, 64)
        metadata = program.metadata()
        self.assertEqual(metadata["seed"], 1234)
        self.assertEqual(metadata["case_index"], 7)
        self.assertEqual([bytes.fromhex(value) for value in metadata["expressions_hex"]],
                         list(program.expressions))

    def test_budget_validation_rejects_invalid_and_excessive_requests(self):
        for parameters in ((-1, 1, 32, 0), (2**64, 1, 32, 0), (True, 1, 32, 0),
                           (1, 0, 32, 0), (1, 10001, 32, 0), (1, 1, 7, 0),
                           (1, 1, 257, 0), (1, 10000, 256, 0), (1, 2, 32, 1_000_000_000),
                           (1, 1, 32, -1), (1, 1.0, 32, 0)):
            with self.subTest(parameters=parameters):
                with self.assertRaises(conformance.HarnessError):
                    generated.validate_budget(*parameters)
        generated.validate_budget(0, 1, 8, 0)
        generated.validate_budget(2**64 - 1, 1, 256, 1_000_000_000)

    def test_program_byte_budget_is_enforced(self):
        with mock.patch.object(generated, "MAX_PROGRAM_BYTES", 20):
            with self.assertRaises(conformance.HarnessError):
                generated.generate_program(1, 0, 8)

    def test_materialized_binary_makefile_and_manifest_are_reusable(self):
        program = generated.generate_program(44101, 0, 128)
        with tempfile.TemporaryDirectory() as temporary:
            case = generated.materialize(program, Path(temporary))
            self.assertEqual((case.path / "Makefile").read_bytes(), program.makefile)
            self.assertEqual(case.candidate, "required")
            self.assertEqual(case.expected, {"exit_code": 0, "stderr": ""})
            metadata = json.loads((case.path / "generation.json").read_text())
            self.assertEqual(metadata["seed"], program.seed)


class ExpectationTests(unittest.TestCase):
    def setUp(self):
        self.program = generated.generate_program(44101, 0, 8)
        self.reference = observation(self.program)

    def test_independently_specified_anchor_values_pass(self):
        self.assertEqual(generated.program_errors(self.program, self.reference), [])

    def test_equal_wrong_outputs_fail_original_expectations(self):
        self.reference.stdout = self.reference.stdout.replace(b"0000:alpha beta", b"0000:wrong")
        self.assertIn("original expectation failed at expression 0",
                      generated.program_errors(self.program, self.reference))

    def test_missing_rows_bad_labels_and_missing_newline_fail(self):
        variants = [self.reference.stdout[:-1], self.reference.stdout.split(b"\n", 1)[1],
                    self.reference.stdout.replace(b"0000:", b"9999:")]
        for output in variants:
            with self.subTest(output=output):
                altered = copy.deepcopy(self.reference)
                altered.stdout = output
                self.assertTrue(generated.program_errors(self.program, altered))

    def test_errors_cannot_masquerade_as_valid_expressions(self):
        self.reference.returncode = 2
        self.reference.stderr = b"failure"
        errors = generated.program_errors(self.program, self.reference)
        self.assertIn("program must exit successfully", errors)
        self.assertIn("program must not emit diagnostics", errors)


class CampaignTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.artifacts = Path(self.temporary.name) / "artifacts"
        self.program = generated.generate_program(44101, 5, 8)
        self.reference = observation(self.program)
        self.identities = {"oracle": {"path": "/oracle", "sha256": "oracle-id"},
                           "candidate": {"path": "/candidate", "sha256": "candidate-id"}}

    def run_campaign(self, side_effect):
        with mock.patch.object(conformance, "execute", side_effect=side_effect), \
             contextlib.redirect_stdout(io.StringIO()) as output:
            result = generated.run_campaign(seed=44101, cases=1, expressions=8, start_index=5,
                                            reference=Path("/oracle"), candidate=Path("/candidate"),
                                            identities=self.identities, artifacts=self.artifacts)
        return result, output.getvalue()

    def evidence(self):
        return next(self.artifacts.glob("generated-*"))

    def test_matching_case_passes_without_failure_artifact(self):
        status, output = self.run_campaign([self.reference, copy.deepcopy(self.reference)])
        self.assertEqual(status, 0)
        self.assertIn("passed=1, failed=0", output)
        self.assertEqual(list(self.artifacts.glob("generated-*")), [])
        reports = list((self.artifacts / "campaigns").glob("*.json"))
        self.assertEqual(len(reports), 1)
        report = json.loads(reports[0].read_text())
        self.assertEqual(report["result"], "PASS")
        self.assertEqual(report["seed"], 44101)
        self.assertEqual(report["start_index"], 5)
        self.assertEqual(report["expressions_per_case"], 8)
        self.assertEqual(report["identities"], self.identities)
        self.assertEqual(report["unexecuted"], 0)
        self.assertEqual(report["generator_version"], generated.GENERATOR_VERSION)
        self.assertEqual(len(report["workload_sha256"]), 64)
        self.assertEqual(report["generator_source_sha256"], conformance.sha256_file(Path(generated.__file__)))

    def test_mismatch_retains_exact_program_raw_streams_and_reproducer(self):
        candidate = copy.deepcopy(self.reference)
        candidate.stdout += b"unexpected\xff\n"
        status, output = self.run_campaign([self.reference, candidate])
        self.assertEqual(status, 1)
        self.assertIn("FAIL", output)
        saved = self.evidence()
        self.assertEqual((saved / "input/Makefile").read_bytes(), self.program.makefile)
        self.assertEqual((saved / "candidate.stdout").read_bytes(), candidate.stdout)
        self.assertEqual((saved / "oracle.stdout").read_bytes(), self.reference.stdout)
        metadata = json.loads((saved / "generation.json").read_text())
        self.assertEqual(metadata["seed"], 44101)
        self.assertEqual(metadata["case_index"], 5)
        self.assertEqual(metadata["reproduce_arguments"],
                         ["--seed", "44101", "--start-index", "5", "--cases", "1", "--expressions", "8"])
        report = json.loads((saved / "report.json").read_text())
        self.assertEqual(report["identities"], self.identities)

    def test_filesystem_only_mutation_fails(self):
        candidate = copy.deepcopy(self.reference)
        candidate.filesystem["work/result.bin"]["content"] = "mutant"
        status, output = self.run_campaign([self.reference, candidate])
        self.assertEqual(status, 1)
        self.assertIn("filesystem", output)

    def test_identical_but_wrong_engines_do_not_pass(self):
        wrong = copy.deepcopy(self.reference)
        wrong.stdout = wrong.stdout.replace(b"0000:alpha beta", b"0000:wrong")
        status, _output = self.run_campaign([wrong, copy.deepcopy(wrong)])
        self.assertEqual(status, 1)

    def test_capture_error_keeps_input_and_completed_oracle_observation(self):
        status, output = self.run_campaign([self.reference, conformance.HarnessError("snapshot limit")])
        self.assertEqual(status, 1)
        self.assertIn("ERROR", output)
        saved = self.evidence()
        self.assertEqual((saved / "input/Makefile").read_bytes(), self.program.makefile)
        self.assertEqual((saved / "oracle.stdout").read_bytes(), self.reference.stdout)
        report = json.loads((saved / "report.json").read_text())
        self.assertEqual(report["error"], "snapshot limit")
        self.assertEqual(report["generation"]["seed"], 44101)

    def test_same_absolute_execution_workspace_is_reused(self):
        with mock.patch.object(conformance, "execute", side_effect=[self.reference, self.reference]) as execute, \
             contextlib.redirect_stdout(io.StringIO()):
            generated.run_campaign(seed=44101, cases=1, expressions=8, start_index=5,
                                   reference=Path("/oracle"), candidate=Path("/candidate"),
                                   identities=self.identities, artifacts=self.artifacts)
        self.assertEqual(execute.call_args_list[0].args[1], execute.call_args_list[1].args[1])

    def test_invalid_budget_is_rejected_before_oracle_execution(self):
        with mock.patch.object(oracle, "verify") as verify, \
             contextlib.redirect_stderr(io.StringIO()) as error:
            status = generated.main(["--self-test", "--cases", "0"])
        self.assertEqual(status, 2)
        verify.assert_not_called()
        self.assertIn("cases must be", error.getvalue())

    def test_unverified_oracle_cannot_start_campaign(self):
        with mock.patch.object(oracle, "verify", side_effect=oracle.OracleError("digest mismatch")), \
             mock.patch.object(generated, "run_campaign") as run, \
             contextlib.redirect_stderr(io.StringIO()) as error:
            status = generated.main(["--self-test"])
        self.assertEqual(status, 2)
        run.assert_not_called()
        self.assertIn("digest mismatch", error.getvalue())


if __name__ == "__main__":
    unittest.main()
