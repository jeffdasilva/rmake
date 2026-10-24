#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Independent graph-generator and strict differential-gate regression tests."""
import base64
from collections import Counter
import contextlib
import copy
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import conformance
import graph_generated as graphs
import oracle


def observation(program):
    # Only the explicit anchors are modeled here. Other node automatic fields
    # are deliberately arbitrary; exact field values belong to the oracle.
    rows = [graphs.ANCHOR, graphs.EMPTY_NODE]
    rows += [f"node|{name}|first|unique|plus|newer|order".encode()
             for name in program.node_names[1:]]
    rows.append(graphs.FINISH)
    files = {f"work/{name}": {"type": "file", "mode": 0o644,
                              "content": base64.b64encode(graphs.node_content(name)).decode()}
             for name in program.node_names}
    return conformance.Observation(b"\n".join(rows) + b"\n", b"", 0, None, files)


class GenerationTests(unittest.TestCase):
    def test_secondary_profile_is_replayable_and_keeps_the_authored_dag(self):
        headers = set()
        for index in range(20):
            plain = graphs.generate_program(44109, index, 8, patterns=True)
            program = graphs.generate_program(44109, index, 8, patterns=True, secondary=True)
            self.assertEqual(plain.declarations, program.declarations)
            self.assertEqual(plain.goals, program.goals)
            self.assertEqual(program, graphs.generate_program(44109, index, 8,
                                                             patterns=True, secondary=True))
            self.assertNotEqual(plain.case_id, program.case_id)
            self.assertEqual(graphs.reproduction(program)[-2:], ["--patterns", "--secondary"])
            self.assertTrue(program.metadata()["secondary_expansion"])
            for target, position in program.secondary_clauses:
                self.assertEqual(program.declarations[position].target, target)
                trace = f"$$(info secondary|{target}|{position}|".encode()
                self.assertEqual(program.makefile.count(trace), 1)
            headers.update(line.split(b": ", 1)[1].split(b":", 1)[0]
                           for line in program.makefile.splitlines()
                           if line.startswith(b"n") and b": " in line and line.count(b":") == 2)
        self.assertIn(b"n%", headers)
        self.assertIn(b"%", headers)
        self.assertTrue(any(header.startswith(b"n") and header.endswith(b"%")
                            and header != b"n%" for header in headers))

    def test_secondary_traces_cannot_be_missing_duplicated_or_malformed(self):
        program = graphs.generate_program(44109, 0, 8, secondary=True)
        observed = observation(program)
        traces = [f"secondary|{name}|{index}|stem|first|unique|plus|newer|order".encode()
                  for name, index in program.secondary_clauses]
        observed.stdout = observed.stdout.replace(graphs.FINISH, b"\n".join(traces + [graphs.FINISH]))
        self.assertEqual(graphs.program_errors(program, observed), [])
        for replacement in (b"", traces[0] + b"\n" + traces[0] + b"\n",
                            b"secondary|malformed\n"):
            mutant = copy.deepcopy(observed)
            mutant.stdout = mutant.stdout.replace(traces[0] + b"\n", replacement, 1)
            self.assertTrue(graphs.program_errors(program, mutant))

    def test_patterns_combine_with_explicit_scopes_without_changing_graphs(self):
        repeated_patterns = False
        for index in range(20):
            plain = graphs.generate_program(44109, index, 8)
            program = graphs.generate_program(44109, index, 8, patterns=True)
            self.assertTrue(program.scopes)
            self.assertTrue(program.patterns)
            self.assertEqual(plain.declarations, program.declarations)
            self.assertEqual(plain.goals, program.goals)
            self.assertEqual(program, graphs.generate_program(44109, index, 8, patterns=True))
            self.assertEqual(graphs.reproduction(program)[-1], "--patterns")
            self.assertTrue(program.metadata()["pattern_scopes"])
            patterns = [line.split(":", 1)[0] for line in program.scope_declarations
                        if "%" in line.split(":", 1)[0]]
            self.assertTrue(patterns)
            repeated_patterns |= len(patterns) != len(set(patterns))
        self.assertTrue(repeated_patterns)

    def test_scope_programs_preserve_graphs_and_replay_scope_declarations(self):
        modifiers = set()
        operators = set()
        for index in range(30):
            plain = graphs.generate_program(44109, index, 8)
            scoped = graphs.generate_program(44109, index, 8, scopes=True)
            self.assertEqual(plain.declarations, scoped.declarations)
            self.assertEqual(plain.goals, scoped.goals)
            self.assertNotEqual(plain.case_id, scoped.case_id)
            self.assertEqual(scoped, graphs.generate_program(44109, index, 8, scopes=True))
            self.assertTrue(scoped.metadata()["target_scopes"])
            self.assertEqual(scoped.metadata()["scope_declarations"], list(scoped.scope_declarations))
            for line in scoped.scope_declarations:
                self.assertIn(line.encode() + b"\n", scoped.makefile)
                modifiers.update(word for word in line.split() if word in
                                 ("private", "override", "export", "unexport"))
                operators.update(word for word in line.split() if word in
                                 ("=", ":=", "::=", ":::=", "+=", "?="))
            self.assertIn("--scopes", graphs.reproduction(scoped))
        self.assertEqual(modifiers, {"private", "override", "export", "unexport"})
        self.assertEqual(operators, {"=", ":=", "::=", ":::=", "+=", "?="})

    def test_same_seed_index_and_size_produce_identical_program(self):
        left = graphs.generate_program(44109, 7, 8)
        right = graphs.generate_program(44109, 7, 8)
        self.assertEqual(left, right)
        self.assertEqual(left.metadata(), right.metadata())

    def test_resuming_does_not_depend_on_preceding_generation(self):
        expected = graphs.generate_program(9, 20, 8)
        for index in range(20):
            graphs.generate_program(9, index, 8)
        self.assertEqual(expected, graphs.generate_program(9, 20, 8))

    def test_seed_and_index_select_different_programs(self):
        first = graphs.generate_program(44109, 0, 8)
        self.assertNotEqual(first.makefile, graphs.generate_program(44110, 0, 8).makefile)
        self.assertNotEqual(first.makefile, graphs.generate_program(44109, 1, 8).makefile)

    def test_graph_is_bounded_acyclic_and_has_exactly_one_recipe_per_node(self):
        for index in range(30):
            program = graphs.generate_program(44109, index, 12)
            declarations = Counter(item.target for item in program.declarations)
            recipes = Counter(item.target for item in program.declarations if item.recipe)
            self.assertEqual(set(declarations), set(program.node_names))
            self.assertTrue(all(1 <= count <= 4 for count in declarations.values()))
            self.assertEqual(recipes, Counter(program.node_names))
            self.assertEqual(set(program.goals), set(program.node_names))
            self.assertLessEqual(set(program.phony), set(program.node_names))
            for item in program.declarations:
                self.assertLessEqual(len(item.normal), 5)
                self.assertLessEqual(len(item.order_only), 4)
                for dependency in item.normal + item.order_only:
                    self.assertLess(int(dependency[1:]), int(item.target[1:]))
            self.assertLessEqual(len(program.makefile), graphs.MAX_PROGRAM_BYTES)

    def test_seeded_sample_contains_duplicates_role_collisions_and_recipe_positions(self):
        duplicated = collision = recipe_not_first = recipe_not_last = False
        for index in range(20):
            program = graphs.generate_program(44109, index, 8)
            for name in program.node_names:
                values = [item for item in program.declarations if item.target == name]
                normal = [dep for item in values for dep in item.normal]
                order = [dep for item in values for dep in item.order_only]
                duplicated |= len(normal) > len(set(normal))
                collision |= bool(set(normal) & set(order))
                recipe_not_first |= not values[0].recipe
                recipe_not_last |= not values[-1].recipe
        self.assertTrue(all((duplicated, collision, recipe_not_first, recipe_not_last)))

    def test_metadata_preserves_original_graph_and_output_expectations(self):
        program = graphs.generate_program(19, 22, 8)
        metadata = program.metadata()
        self.assertEqual(metadata["seed"], 19)
        self.assertEqual(metadata["case_index"], 22)
        self.assertEqual(metadata["node_count"], 8)
        self.assertEqual(len(metadata["declarations"]), len(program.declarations))
        self.assertEqual(metadata["original_expectations"]["node_files"]["n0"],
                         graphs.node_content("n0").hex())

    def test_budget_rejects_invalid_types_and_excessive_work(self):
        invalid = [(-1, 1, 8, 0, 20), (2**64, 1, 8, 0, 20), (True, 1, 8, 0, 20),
                   (1, 0, 8, 0, 20), (1, 10001, 8, 0, 20), (1, 1, 1, 0, 20),
                   (1, 1, 33, 0, 20), (1, 10000, 32, 0, 20), (1, 1, 8, -1, 20),
                   (1, 2, 8, 1_000_000_000, 20), (1, 1, 8, 0, 0),
                   (1, 1, 8, 0, 1001), (1, 1.0, 8, 0, 20)]
        for values in invalid:
            with self.subTest(values=values):
                with self.assertRaises(conformance.HarnessError):
                    graphs.validate_budget(*values)
        graphs.validate_budget(0, 1, 2)
        graphs.validate_budget(2**64 - 1, 1, 32, 1_000_000_000, 1)

    def test_source_byte_budget_is_enforced(self):
        with mock.patch.object(graphs, "MAX_PROGRAM_BYTES", 10):
            with self.assertRaises(conformance.HarnessError):
                graphs.generate_program(1, 0, 8)

    def test_materialized_program_retains_exact_source_and_required_manifest(self):
        program = graphs.generate_program(44109, 5, 8)
        with tempfile.TemporaryDirectory() as temporary:
            case = graphs.materialize(program, Path(temporary))
            self.assertEqual((case.path / "Makefile").read_bytes(), program.makefile)
            self.assertEqual(case.candidate, "required")
            self.assertEqual(case.args, ("-rR", "--no-print-directory"))
            self.assertEqual(case.expected, {"exit_code": 0, "stderr": ""})
            saved = json.loads((case.path / "generation.json").read_text())
            self.assertEqual(saved["program_sha256"], program.metadata()["program_sha256"])


class ExpectationTests(unittest.TestCase):
    def test_scope_anchors_reject_missing_duplicate_and_malformed_observations(self):
        program = graphs.generate_program(44109, 5, 8, scopes=True)
        observed = observation(program)
        rows = [f"scope|{name}|value|raw|file|recursive|other|export|export".encode()
                for name in program.node_names]
        observed.stdout = observed.stdout.replace(graphs.FINISH,
                                                  b"\n".join(rows + [graphs.FINISH]))
        self.assertEqual(graphs.program_errors(program, observed), [])
        for wrong in (rows[1:], rows + rows[:1], [b"scope|n0"] + rows[1:]):
            mutant = copy.deepcopy(observed)
            mutant.stdout = mutant.stdout.replace(b"\n".join(rows), b"\n".join(wrong))
            self.assertTrue(graphs.program_errors(program, mutant))

    def setUp(self):
        self.program = graphs.generate_program(44109, 5, 8)
        self.reference = observation(self.program)

    def test_independent_markers_and_output_files_pass(self):
        self.assertEqual(graphs.program_errors(self.program, self.reference), [])

    def test_first_last_and_empty_node_anchors_detect_equal_wrong_engines(self):
        for original in (graphs.ANCHOR, graphs.FINISH, graphs.EMPTY_NODE):
            with self.subTest(original=original):
                altered = copy.deepcopy(self.reference)
                altered.stdout = altered.stdout.replace(original, b"wrong", 1)
                self.assertTrue(graphs.program_errors(self.program, altered))

    def test_duplicate_missing_and_malformed_trace_rows_fail(self):
        rows = self.reference.stdout.splitlines(keepends=True)
        for output in (b"".join(rows[:2] + rows[1:]), b"".join(rows[:1] + rows[2:]),
                       self.reference.stdout.replace(b"node|n1|", b"invalid|n1|"),
                       self.reference.stdout[:-1]):
            altered = copy.deepcopy(self.reference)
            altered.stdout = output
            self.assertTrue(graphs.program_errors(self.program, altered))

    def test_missing_file_wrong_type_and_wrong_contents_fail(self):
        for entry in (None, {"type": "symlink", "target": "n1"},
                      {"type": "file", "content": base64.b64encode(b"wrong").decode()}):
            altered = copy.deepcopy(self.reference)
            if entry is None:
                del altered.filesystem["work/n0"]
            else:
                altered.filesystem["work/n0"] = entry
            self.assertIn("original output-file expectation failed for n0",
                          graphs.program_errors(self.program, altered))


class CampaignTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.artifacts = Path(temporary.name) / "artifacts"
        self.program = graphs.generate_program(44109, 5, 8)
        self.reference = observation(self.program)
        self.identities = {"oracle": {"path": "/oracle", "sha256": "oracle-digest"},
                           "candidate": {"path": "/candidate", "sha256": "candidate-digest"}}

    def run_campaign(self, effects, **options):
        arguments = {"seed": 44109, "cases": 1, "nodes": 8, "start_index": 5,
                     "reference": Path("/oracle"), "candidate": Path("/candidate"),
                     "identities": self.identities, "artifacts": self.artifacts}
        arguments.update(options)
        with mock.patch.object(conformance, "execute", side_effect=effects) as execute, \
             contextlib.redirect_stdout(io.StringIO()) as output:
            status = graphs.run_campaign(**arguments)
        return status, output.getvalue(), execute

    def report(self):
        return json.loads(next((self.artifacts / "campaigns").glob("*.json")).read_text())

    def evidence(self):
        return next(self.artifacts.glob("graph-generated-*"))

    def test_success_retains_complete_campaign_metadata_without_failure_artifact(self):
        status, output, _execute = self.run_campaign([self.reference, copy.deepcopy(self.reference)])
        self.assertEqual(status, 0)
        self.assertIn("passed=1, failed=0", output)
        self.assertIn("start_index=5, generator=v1", output)
        self.assertEqual(list(self.artifacts.glob("graph-generated-*")), [])
        report = self.report()
        for key, value in {"seed": 44109, "start_index": 5, "cases": 1,
                           "nodes_per_case": 8, "executed": 1, "passed": 1,
                           "unexecuted": 0, "result": "PASS"}.items():
            self.assertEqual(report[key], value)
        self.assertEqual(report["identities"], self.identities)
        self.assertEqual(len(report["workload_sha256"]), 64)
        self.assertEqual(report["generator_source_sha256"],
                         conformance.sha256_file(Path(graphs.__file__)))

    def test_raw_stdout_mutation_retains_exact_source_streams_and_reproducer(self):
        mutant = copy.deepcopy(self.reference)
        mutant.stdout = mutant.stdout.replace(b"first", b"changed\xff", 1)
        status, _output, _execute = self.run_campaign([self.reference, mutant])
        self.assertEqual(status, 1)
        evidence = self.evidence()
        self.assertEqual((evidence / "input/Makefile").read_bytes(), self.program.makefile)
        self.assertEqual((evidence / "oracle.stdout").read_bytes(), self.reference.stdout)
        self.assertEqual((evidence / "candidate.stdout").read_bytes(), mutant.stdout)
        metadata = json.loads((evidence / "generation.json").read_text())
        self.assertEqual(metadata["reproduce_arguments"],
                         ["--seed", "44109", "--start-index", "5", "--cases", "1", "--nodes", "8"])
        report = json.loads((evidence / "report.json").read_text())
        self.assertEqual(report["identities"], self.identities)
        self.assertIn("stdout", report["differences"])

    def test_stderr_status_and_filesystem_mutations_each_fail(self):
        mutations = (("stderr", lambda item: setattr(item, "stderr", b"unexpected\xff\n")),
                     ("returncode", lambda item: setattr(item, "returncode", 2)),
                     ("filesystem", lambda item: item.filesystem["work/n0"].update(mode=0o755)),
                     ("filesystem", lambda item: item.filesystem.update({"work/extra": {"type": "file"}})))
        for index, (expected, mutate) in enumerate(mutations):
            with self.subTest(expected=expected, index=index):
                mutant = copy.deepcopy(self.reference)
                mutate(mutant)
                status, output, _execute = self.run_campaign(
                    [self.reference, mutant], artifacts=self.artifacts / str(index))
                self.assertEqual(status, 1)
                self.assertIn(expected, output)

    def test_identical_wrong_stdout_and_files_do_not_pass(self):
        wrong = copy.deepcopy(self.reference)
        wrong.stdout = wrong.stdout.replace(graphs.ANCHOR, b"wrong")
        wrong.filesystem["work/n0"]["content"] = "wrong"
        status, output, _execute = self.run_campaign([wrong, copy.deepcopy(wrong)])
        self.assertEqual(status, 1)
        self.assertIn("first-target anchor", output)
        self.assertIn("output-file expectation", output)

    def test_capture_failure_preserves_program_and_completed_oracle(self):
        status, output, _execute = self.run_campaign(
            [self.reference, conformance.HarnessError("snapshot exceeded bound")])
        self.assertEqual(status, 1)
        self.assertIn("ERROR", output)
        evidence = self.evidence()
        self.assertEqual((evidence / "input/Makefile").read_bytes(), self.program.makefile)
        self.assertEqual((evidence / "oracle.stdout").read_bytes(), self.reference.stdout)
        report = json.loads((evidence / "report.json").read_text())
        self.assertEqual(report["generation"]["case_index"], 5)
        self.assertEqual(report["error"], "snapshot exceeded bound")

    def test_both_sides_reuse_one_absolute_workspace(self):
        _status, _output, execute = self.run_campaign([self.reference, self.reference])
        self.assertEqual(execute.call_args_list[0].args[1], execute.call_args_list[1].args[1])
        self.assertTrue(execute.call_args_list[0].args[1].is_absolute())

    def test_failure_limit_reports_unexecuted_graphs_without_counting_them_as_passes(self):
        wrong = copy.deepcopy(self.reference)
        wrong.stderr = b"failure"
        status, output, execute = self.run_campaign([self.reference, wrong], cases=5, max_failures=1)
        self.assertEqual(status, 1)
        self.assertEqual(execute.call_count, 2)
        self.assertIn("4 graphs unexecuted", output)
        report = self.report()
        self.assertEqual((report["executed"], report["passed"], report["failed"], report["unexecuted"]),
                         (1, 0, 1, 4))

    def test_explicit_report_path_is_honored(self):
        path = self.artifacts / "summary.json"
        status, _output, _execute = self.run_campaign([self.reference, self.reference], report_path=path)
        self.assertEqual(status, 0)
        self.assertEqual(json.loads(path.read_text())["result"], "PASS")

    def test_invalid_bounds_reject_before_oracle_verification(self):
        with mock.patch.object(oracle, "verify") as verify, \
             contextlib.redirect_stderr(io.StringIO()) as error:
            status = graphs.main(["--self-test", "--nodes", "0"])
        self.assertEqual(status, 2)
        verify.assert_not_called()
        self.assertIn("nodes must be", error.getvalue())

    def test_unverified_oracle_cannot_start_campaign(self):
        with mock.patch.object(oracle, "verify", side_effect=oracle.OracleError("digest mismatch")), \
             mock.patch.object(graphs, "run_campaign") as run, \
             contextlib.redirect_stderr(io.StringIO()) as error:
            status = graphs.main(["--self-test"])
        self.assertEqual(status, 2)
        run.assert_not_called()
        self.assertIn("digest mismatch", error.getvalue())


if __name__ == "__main__":
    unittest.main()
