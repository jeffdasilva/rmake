#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Generation safety, independent expectations, and strict campaign evidence."""
import contextlib
import copy
import io
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest import mock

import conformance
import intcmp_generated as generated
import oracle


def observation(program):
    return conformance.Observation(
        program.expected_stdout, b"", 0, None,
        {"work/result.bin": generated.regular_file(program.expected_stdout),
         "work/protected-input.txt": generated.regular_file(generated.PROTECTED)})


def expected_execution(case, *_args):
    output = case.expected["stdout"].encode("ascii")
    return conformance.Observation(
        output, b"", 0, None,
        {"work/result.bin": generated.regular_file(output),
         "work/protected-input.txt": generated.regular_file(generated.PROTECTED)})


class GenerationTests(unittest.TestCase):
    def test_seed_and_index_reproduce_independently_of_previous_cases(self):
        wanted = generated.generate_program(1234, 17, 32)
        for index in range(17):
            generated.generate_program(1234, index, 32)
        self.assertEqual(wanted, generated.generate_program(1234, 17, 32))
        self.assertNotEqual(wanted.makefile, generated.generate_program(1235, 17, 32).makefile)
        self.assertNotEqual(wanted.makefile, generated.generate_program(1234, 18, 32).makefile)

    def test_pinned_negative_magnitude_order_and_signed_zero(self):
        for left, right, expected in ((-19, -7, 1), (-7, -19, -1), (-19, 7, -1),
                                      (0, -1, 1), (-0, +0, 0), (-(10**100), -9, 1)):
            with self.subTest(left=left, right=right):
                self.assertEqual(generated.compare(left, right), expected)

    def test_numeric_spelling_preserves_value_with_huge_values(self):
        rng = generated.random.Random(1)
        for value in (0, -1, 2**64, -(10**512 - 1)):
            for _ in range(30):
                spelled = generated.spelling(rng, value)
                if spelled.startswith(b"$(strip "):
                    spelled = spelled[len(b"$(strip "):-1]
                self.assertEqual(int(spelled), value)

    def test_generated_grammar_and_outputs_stay_inside_recipe_quotes(self):
        for index in range(20):
            program = generated.generate_program(9, index, 64)
            self.assertLessEqual(len(program.makefile), generated.MAX_PROGRAM_BYTES)
            for value in program.expressions + program.values:
                for unsafe in (b"'", b"\n", b"\r", b"\0"):
                    self.assertNotIn(unsafe, value)
            self.assertIn(b" > result.bin\n", program.makefile)
            self.assertIn(b"\t@cat result.bin\n", program.makefile)

    def test_unsafe_grammar_or_model_outputs_are_rejected(self):
        for pair in ((b"$(intcmp 1,1)'", b"1"), (b"$(intcmp 1,1)", b"bad\nvalue")):
            with mock.patch.object(generated, "expression", return_value=pair):
                with self.assertRaises(conformance.HarnessError):
                    generated.generate_program(1, 0, 1)

    def test_metadata_reconstructs_every_expression_and_expected_row(self):
        program = generated.generate_program(1, 2, 8)
        metadata = program.metadata()
        self.assertEqual(tuple(bytes.fromhex(value) for value in metadata["expressions_hex"]),
                         program.expressions)
        self.assertEqual(tuple(bytes.fromhex(value) for value in metadata["expected_rows_hex"]),
                         program.values)
        self.assertEqual(len(program.expected_stdout.splitlines()), 8)
        self.assertEqual(metadata["seed"], 1)
        self.assertEqual(metadata["case_index"], 2)
        self.assertEqual(metadata["program_sha256"], generated.hashlib.sha256(program.makefile).hexdigest())

    def test_materialized_case_is_directly_replayable_with_typed_expectations(self):
        program = generated.generate_program(7, 3, 8)
        with tempfile.TemporaryDirectory() as temporary:
            case = generated.materialize(program, Path(temporary))
            self.assertEqual(conformance.load_case(case.path), case)
            self.assertEqual((case.path / "Makefile").read_bytes(), program.makefile)
            self.assertEqual(case.expected, {"exit_code": 0, "stderr": "",
                                             "stdout": program.expected_stdout.decode("ascii")})
            self.assertEqual((case.path / "protected-input.txt").read_bytes(), generated.PROTECTED)
            metadata = json.loads((case.path / "generation.json").read_text())
            self.assertEqual(metadata, program.metadata())

    def test_budgets_reject_invalid_types_and_excess_total_work(self):
        for parameters in ((True, 1, 1, 0, 1), (-1, 1, 1, 0, 1), (2**64, 1, 1, 0, 1),
                           (1, 0, 1, 0, 1), (1, 10001, 1, 0, 1), (1, 1, 0, 0, 1),
                           (1, 1, 65, 0, 1), (1, 10000, 64, 0, 1),
                           (1, 2, 1, 1000000000, 1), (1, 1, 1, 0, 0),
                           (1, 1, 1, 0, True), (1, 1.5, 1, 0, 1)):
            with self.subTest(parameters=parameters), self.assertRaises(conformance.HarnessError):
                generated.validate_budget(*parameters)
        generated.validate_budget(0, 1, 1, 0, 1)
        generated.validate_budget(2**64 - 1, 1, 64, 1000000000, 10000)

    def test_source_size_budget_is_enforced(self):
        with mock.patch.object(generated, "MAX_PROGRAM_BYTES", 1):
            with self.assertRaises(conformance.HarnessError):
                generated.generate_program(1, 0, 1)


class ExpectationTests(unittest.TestCase):
    def setUp(self):
        self.program = generated.generate_program(3, 0, 4)

    def test_success_requires_exact_expected_output_and_files(self):
        self.assertEqual(generated.program_errors(self.program, observation(self.program)), [])

    def test_status_stderr_and_output_are_independently_checked(self):
        for attribute, value in (("returncode", 2), ("stderr", b"unexpected\xff"), ("stdout", b"wrong\n")):
            altered = observation(self.program)
            setattr(altered, attribute, value)
            with self.subTest(attribute=attribute):
                self.assertTrue(generated.program_errors(self.program, altered))

    def test_matching_resource_limits_and_background_processes_are_failures(self):
        for attribute, value in (("limit", "output"), ("limit", "time"), ("leftover_processes", True)):
            altered = observation(self.program)
            setattr(altered, attribute, value)
            with self.subTest(attribute=attribute):
                self.assertTrue(generated.program_errors(self.program, altered))
                self.assertTrue(conformance.differences(altered, copy.deepcopy(altered)))

    def test_result_file_type_mode_bytes_and_protected_input_are_checked(self):
        for path, field, value in (("work/result.bin", "type", "symlink"),
                                  ("work/result.bin", "mode", 0o600),
                                  ("work/result.bin", "content", "wrong"),
                                  ("work/protected-input.txt", "size", 0)):
            altered = observation(self.program)
            altered.filesystem[path][field] = value
            with self.subTest(path=path, field=field):
                self.assertTrue(generated.program_errors(self.program, altered))
        altered = observation(self.program)
        del altered.filesystem["work/result.bin"]
        self.assertTrue(generated.program_errors(self.program, altered))


class CampaignTests(unittest.TestCase):
    def run_campaign(self, root, **overrides):
        parameters = dict(seed=42, cases=3, expressions=4, start_index=9,
                          reference=Path("/oracle"), candidate=Path("/candidate"),
                          identities={"oracle": {"sha256": "oracle"}, "candidate": {"sha256": "candidate"}},
                          artifacts=root / "evidence", report_path=root / "report.json",
                          profile=Path("/custom-profile.json"), max_failures=2)
        parameters.update(overrides)
        with contextlib.redirect_stdout(io.StringIO()):
            code = generated.run_campaign(**parameters)
        return code, json.loads((root / "report.json").read_text())

    def test_success_report_retains_complete_replay_identity_and_workload(self):
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(
                conformance, "execute", side_effect=expected_execution):
            code, report = self.run_campaign(Path(temporary))
            self.assertEqual(code, 0)
            self.assertEqual((report["executed"], report["passed"], report["failed"], report["unexecuted"]),
                             (3, 3, 0, 0))
            self.assertEqual((report["seed"], report["start_index"], report["expressions"]), (42, 9, 4))
            self.assertEqual(report["executed_observations"], 12)
            self.assertEqual(report["oracle_profile"], "/custom-profile.json")
            self.assertEqual(len(report["workload_sha256"]), 64)
            snapshot = Path(report["generator_source_snapshot"])
            self.assertEqual(conformance.sha256_file(snapshot), report["generator_source_sha256"])
            self.assertEqual(snapshot.read_bytes(), Path(generated.__file__).read_bytes())

    def test_same_wrong_result_from_both_engines_fails_independent_model(self):
        def wrong(case, *args):
            result = expected_execution(case, *args)
            result.stdout = b"wrong\n"
            result.filesystem["work/result.bin"] = generated.regular_file(result.stdout)
            return result
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(conformance, "execute", side_effect=wrong):
            code, report = self.run_campaign(Path(temporary), max_failures=1)
            self.assertEqual(code, 1)
            self.assertEqual((report["executed"], report["failed"], report["unexecuted"]), (1, 1, 2))
            record = report["programs"][0]
            self.assertEqual(record["status"], "FAIL")
            artifact = Path(record["artifact"])
            self.assertTrue((artifact / "input/case.toml").is_file())
            replay_input = Path(temporary) / "replay" / record["case_id"]
            replay_input.parent.mkdir()
            shutil.copytree(artifact / "input", replay_input)
            self.assertEqual(conformance.load_case(replay_input).id, record["case_id"])
            self.assertTrue((artifact / "oracle.stdout").is_file())
            self.assertTrue((artifact / "candidate.stdout").is_file())
            metadata = json.loads((artifact / "generation.json").read_text())
            self.assertEqual(metadata["case_index"], 9)
            self.assertIn("/custom-profile.json", metadata["replay_argv"])
            self.assertEqual(metadata["replay_argv"], record["replay_argv"])
            self.assertEqual(report["requested_observations"], 12)
            self.assertEqual(report["executed_observations"], 4)

    def test_identical_limits_never_pass_campaign(self):
        def limited(case, *args):
            result = expected_execution(case, *args)
            result.limit = "output"
            return result
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(conformance, "execute", side_effect=limited):
            code, report = self.run_campaign(Path(temporary), max_failures=1)
            self.assertEqual(code, 1)
            self.assertIn("execution-limit", report["programs"][0]["errors"])

    def test_exception_retains_completed_oracle_capture_and_final_report(self):
        def capture(case, _root, executable):
            if executable == Path("/candidate"):
                raise conformance.HarnessError("snapshot failed")
            return expected_execution(case)
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(conformance, "execute", side_effect=capture):
            code, report = self.run_campaign(Path(temporary), max_failures=1)
            self.assertEqual(code, 1)
            record = report["programs"][0]
            self.assertEqual(record["status"], "ERROR")
            artifact = Path(record["artifact"])
            self.assertTrue((artifact / "input/Makefile").is_file())
            self.assertTrue((artifact / "oracle.stdout").is_file())
            detail = json.loads((artifact / "report.json").read_text())
            self.assertEqual(set(detail["observations"]), {"oracle"})
            self.assertEqual(detail["errors"], ["snapshot failed"])
            self.assertEqual(report["unexecuted"], 2)

    def test_first_capture_exception_still_retains_input_and_empty_observations(self):
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(
                conformance, "execute", side_effect=OSError("cannot launch")):
            code, report = self.run_campaign(Path(temporary), max_failures=1)
            self.assertEqual(code, 1)
            artifact = Path(report["programs"][0]["artifact"])
            self.assertTrue((artifact / "input/case.toml").is_file())
            self.assertEqual(json.loads((artifact / "report.json").read_text())["observations"], {})

    def test_report_is_saved_when_no_custom_path_is_given(self):
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(
                conformance, "execute", side_effect=expected_execution), contextlib.redirect_stdout(io.StringIO()):
            root = Path(temporary)
            result = generated.run_campaign(seed=1, cases=1, expressions=1, start_index=0,
                                             reference=Path("/oracle"), candidate=Path("/candidate"),
                                             identities={}, artifacts=root)
            self.assertEqual(result, 0)
            reports = list((root / "campaigns").glob("*.json"))
            self.assertEqual(len(reports), 1)
            self.assertEqual(json.loads(reports[0].read_text())["result"], "PASS")


class CliTests(unittest.TestCase):
    def test_oracle_validation_failure_prevents_execution(self):
        with mock.patch.object(oracle, "verify", side_effect=oracle.OracleError("profile mismatch")), \
                mock.patch.object(generated, "run_campaign") as run, contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(generated.main(["--self-test"]), 2)
            run.assert_not_called()

    def test_invalid_budget_is_rejected_before_oracle_or_capture(self):
        with mock.patch.object(oracle, "verify") as verify, contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(generated.main(["--self-test", "--cases", "0"]), 2)
            verify.assert_not_called()

    def test_profile_and_candidate_identity_are_forwarded_to_campaign(self):
        def identity(path, **_kwargs):
            return {"path": str(Path(path).resolve()), "sha256": "identity"}
        with mock.patch.object(oracle, "verify", return_value={"profile_id": "verified"}) as verify, \
                mock.patch.object(conformance, "executable_identity", side_effect=identity), \
                mock.patch.object(generated, "run_campaign", return_value=0) as run:
            result = generated.main(["--candidate", "/candidate", "--oracle", "/oracle",
                                     "--oracle-profile", "/custom-profile", "--seed", "17",
                                     "--start-index", "9", "--cases", "2", "--expressions", "8"])
            self.assertEqual(result, 0)
            verify.assert_called_once_with(Path("/oracle"), Path("/custom-profile"))
            arguments = run.call_args.kwargs
            self.assertEqual(arguments["candidate"], Path("/candidate"))
            self.assertEqual(arguments["identities"]["oracle"]["profile"]["profile_id"], "verified")
            self.assertEqual((arguments["seed"], arguments["start_index"], arguments["cases"]), (17, 9, 2))


if __name__ == "__main__":
    unittest.main()
