#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Reproducible differential tests of independently generated explicit DAGs.

The generator never models GNU Make's prerequisite ordering. Exact behavior is
compared with the verified oracle through conformance's bounded, same-alias
runner. These are trusted fixtures, not a security sandbox.
"""
from __future__ import annotations

import argparse
import base64
from collections import Counter
import dataclasses
import hashlib
import json
from pathlib import Path
import random
import shutil
import sys
import tempfile
from typing import Any

import conformance
import oracle

GENERATOR_VERSION = 1
DEFAULT_SEED = 44109
DEFAULT_CASES = 100
DEFAULT_NODES = 8
DEFAULT_MAX_FAILURES = 20
MAX_CASES = 10000
MAX_NODES = 32
MAX_TOTAL_NODES = 100000
MAX_PROGRAM_BYTES = 64 * 1024
MAX_INDEX = 1_000_000_000
ANCHOR = b"anchor|ready"
FINISH = b"finish|ready"
EMPTY_NODE = b"node|n0|||||"
LIMITATIONS = ["trusted-fixture process containment, no security sandbox",
               "timestamps not compared", "single invocation only",
               "explicit acyclic graphs only; no parallel execution"]


@dataclasses.dataclass(frozen=True)
class Declaration:
    target: str
    normal: tuple[str, ...]
    order_only: tuple[str, ...]
    recipe: bool


@dataclasses.dataclass(frozen=True)
class Program:
    seed: int
    index: int
    nodes: int
    declarations: tuple[Declaration, ...]
    phony: tuple[str, ...]
    goals: tuple[str, ...]
    makefile: bytes
    scopes: bool = False
    scope_declarations: tuple[str, ...] = ()

    @property
    def case_id(self) -> str:
        suffix = "-scopes" if self.scopes else ""
        return f"graph-generated-{self.seed}-{self.index:06d}{suffix}"

    @property
    def node_names(self) -> tuple[str, ...]:
        return tuple(f"n{index}" for index in range(self.nodes))

    def metadata(self) -> dict[str, Any]:
        return {"generator_version": GENERATOR_VERSION, "seed": self.seed,
                "case_index": self.index, "node_count": self.nodes,
                "program_sha256": hashlib.sha256(self.makefile).hexdigest(),
                "declarations": [dataclasses.asdict(value) for value in self.declarations],
                "target_scopes": self.scopes,
                "scope_declarations": list(self.scope_declarations),
                "phony_nodes": list(self.phony), "goal_prerequisites": list(self.goals),
                "original_expectations": {"first_row": ANCHOR.decode(),
                                          "last_row": FINISH.decode(),
                                          "empty_node_row": EMPTY_NODE.decode(),
                                          "node_files": {name: node_content(name).hex()
                                                         for name in self.node_names}}}


def node_content(name: str) -> bytes:
    return f"artifact|{name}\n".encode("ascii")


def validate_budget(seed: int, cases: int, nodes: int, start_index: int = 0,
                    max_failures: int = DEFAULT_MAX_FAILURES) -> None:
    for name, value, low, high in (("seed", seed, 0, 2**64 - 1),
                                  ("cases", cases, 1, MAX_CASES),
                                  ("nodes", nodes, 2, MAX_NODES),
                                  ("start-index", start_index, 0, MAX_INDEX),
                                  ("max-failures", max_failures, 1, 1000)):
        if type(value) is not int or not low <= value <= high:
            raise conformance.HarnessError(f"{name} must be an integer between {low} and {high}")
    if cases * nodes > MAX_TOTAL_NODES:
        raise conformance.HarnessError(f"campaign exceeds {MAX_TOTAL_NODES} generated nodes")
    if start_index + cases > MAX_INDEX + 1:
        raise conformance.HarnessError("campaign case index exceeds the supported range")


def generate_program(seed: int, index: int, nodes: int, scopes: bool = False) -> Program:
    validate_budget(seed, 1, nodes, index)
    digest = hashlib.sha256(
        f"rmake-graph-generated-v{GENERATOR_VERSION}:{seed}:{index}:{nodes}".encode()).digest()
    rng = random.Random(int.from_bytes(digest, "big"))
    pending: list[list[Declaration]] = []
    phony = []
    for node in range(nodes):
        name = f"n{node}"
        if rng.randrange(4) == 0:
            phony.append(name)
        count = rng.randrange(1, 5)
        recipe_index = rng.randrange(count)
        declarations = []
        for position in range(count):
            # Only earlier-numbered nodes are eligible. Repetitions and role
            # collisions are intentionally retained in the authored program.
            normal = tuple(f"n{rng.randrange(node)}" for _ in range(rng.randrange(6))) if node else ()
            order = tuple(f"n{rng.randrange(node)}" for _ in range(rng.randrange(5))) if node else ()
            declarations.append(Declaration(name, normal, order, position == recipe_index))
        pending.append(declarations)
    # Interleave independent targets without changing a target's declaration
    # sequence. This adds layout variety without predicting execution order.
    declarations = []
    while pending:
        selected = rng.randrange(len(pending))
        declarations.append(pending[selected].pop(0))
        if not pending[selected]:
            pending.pop(selected)
    goals = [f"n{node}" for node in range(nodes)]
    rng.shuffle(goals)
    goals.extend(rng.choice(goals) for _ in range(rng.randrange(4)))
    scope_declarations = []
    if scopes:
        scope_declarations.extend(["X = global-$(Y)", "Y = early", "export X Y"])
        for name in ["all"] + [f"n{node}" for node in range(nodes)]:
            for position in range(rng.randrange(4)):
                variable = rng.choice(("X", "Y"))
                operator = rng.choice(("=", ":=", "::=", ":::=", "+=", "?="))
                modifier = rng.choice(("", "private ", "override ", "export ", "unexport ",
                                       "private export ", "override private "))
                # X may refer to Y; Y never refers back to X. This allows
                # context-sensitive expansion without creating recursive cycles.
                value = rng.choice(("", f"{name}-{position}", "$(Y)" if variable == "X" else "$@"))
                scope_declarations.append(f"{name}: {modifier}{variable} {operator} {value}")
        scope_declarations.append("Y = late")
    lines = ["# SPDX-License-Identifier: GPL-3.0-or-later",
             "# Independently generated; generation.json preserves the seed and declarations.",
             ".DEFAULT_GOAL := all"] + scope_declarations + [".PHONY: all anchor " + " ".join(phony),
             "all: anchor " + " ".join(goals),
             "\t@printf '%s\\n' 'finish|ready'", "anchor:",
             "\t@printf '%s\\n' 'anchor|ready'"]
    for declaration in declarations:
        lines.append(declaration.target + ": " + " ".join(declaration.normal)
                     + " | " + " ".join(declaration.order_only))
        if declaration.recipe:
            lines.extend(["\t@printf '%s\\n' 'node|$@|$<|$^|$+|$?|$|'",
                          "\t@printf '%s\\n' 'artifact|$@' > '$@'"])
            if scopes:
                lines.append("\t@printf '%s|%s|%s\\n' "
                             "'scope|$@|$(X)|$(value X)|$(origin X)|$(flavor X)|$(Y)' "
                             '\"$${X-unset}\" \"$${Y-unset}\"')
    makefile = ("\n".join(lines) + "\n").encode("ascii")
    if len(makefile) > MAX_PROGRAM_BYTES:
        raise conformance.HarnessError("generated Makefile exceeds its byte budget")
    return Program(seed, index, nodes, tuple(declarations), tuple(phony), tuple(goals), makefile,
                   scopes, tuple(scope_declarations))


def materialize(program: Program, parent: Path) -> conformance.Case:
    path = parent / program.case_id
    path.mkdir(parents=True)
    (path / "Makefile").write_bytes(program.makefile)
    (path / "generation.json").write_text(json.dumps(program.metadata(), indent=2, sort_keys=True) + "\n")
    (path / "case.toml").write_text(
        f'id = "{program.case_id}"\nrequirement = "generated.explicit-graphs"\n'
        'candidate = "required"\nargs = ["-rR", "--no-print-directory"]\n'
        'timeout_seconds = 5\n'
        f'provenance = "Original graph generator v{GENERATOR_VERSION}; seed {program.seed}; index {program.index}"\n'
        '[expected]\nexit_code = 0\nstderr = ""\n')
    return conformance.load_case(path)


def program_errors(program: Program, observed: conformance.Observation) -> list[str]:
    """Check independent anchors without implementing GNU's ordering algorithm."""
    errors = []
    if observed.returncode != 0:
        errors.append("graph must exit successfully")
    if observed.stderr:
        errors.append("acyclic graph must not emit diagnostics")
    rows = observed.stdout.split(b"\n")
    if not rows or rows.pop() != b"":
        errors.append("output must end with a newline")
    if not rows or rows[0] != ANCHOR:
        errors.append("original first-target anchor failed")
    if not rows or rows[-1] != FINISH:
        errors.append("original final-target anchor failed")
    middle = rows[1:-1]
    seen = []
    scope_seen = []
    for row in middle:
        fields = row.split(b"|")
        if program.scopes and fields[0] == b"scope":
            if len(fields) != 9:
                errors.append("scope trace has an invalid shape")
            else:
                scope_seen.append(fields[1])
            continue
        if len(fields) != 7 or fields[0] != b"node":
            errors.append("node trace has an invalid shape")
            continue
        seen.append(fields[1])
    if Counter(seen) != Counter(name.encode("ascii") for name in program.node_names):
        errors.append("every generated node must execute exactly once")
    if program.scopes and Counter(scope_seen) != Counter(name.encode("ascii") for name in program.node_names):
        errors.append("every generated scope must be observed exactly once")
    if middle.count(EMPTY_NODE) != 1:
        errors.append("original empty-node automatic-variable anchor failed")
    for name in program.node_names:
        expected = node_content(name)
        entry = observed.filesystem.get(f"work/{name}", {})
        if (entry.get("type") != "file"
                or entry.get("content") != base64.b64encode(expected).decode("ascii")):
            errors.append(f"original output-file expectation failed for {name}")
    return errors


def reproduction(program: Program) -> list[str]:
    return ["--seed", str(program.seed), "--start-index", str(program.index),
            "--cases", "1", "--nodes", str(program.nodes)] + (["--scopes"] if program.scopes else [])


def save_failure(artifacts: Path, program: Program, case: conformance.Case, workspace: Path,
                 identities: dict[str, Any], reference: conformance.Observation,
                 candidate: conformance.Observation, errors: list[str]) -> Path:
    target = conformance.save_artifact(artifacts, case, workspace, identities, reference,
                                       candidate, errors, "FAIL")
    metadata = program.metadata()
    metadata["reproduce_arguments"] = reproduction(program)
    (target / "generation.json").write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n")
    return target


def save_execution_failure(artifacts: Path, program: Program, case: conformance.Case,
                           identities: dict[str, Any], error: Exception,
                           observations: dict[str, conformance.Observation]) -> Path:
    artifacts.mkdir(parents=True, exist_ok=True)
    target = Path(tempfile.mkdtemp(prefix=f"{program.case_id}-", dir=artifacts))
    shutil.copytree(case.path, target / "input", symlinks=True)
    metadata = program.metadata()
    metadata["reproduce_arguments"] = reproduction(program)
    (target / "generation.json").write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n")
    report = {"status": "ERROR", "error": str(error), "identities": identities,
              "generation": metadata, "observations": {}}
    for label, observation in observations.items():
        (target / f"{label}.stdout").write_bytes(observation.stdout)
        (target / f"{label}.stderr").write_bytes(observation.stderr)
        report["observations"][label] = conformance.observation_json(observation)
    (target / "report.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    return target


def run_campaign(*, seed: int, cases: int, nodes: int, start_index: int,
                 reference: Path, candidate: Path, identities: dict[str, Any],
                 artifacts: Path, report_path: Path | None = None,
                 max_failures: int = DEFAULT_MAX_FAILURES, scopes: bool = False) -> int:
    validate_budget(seed, cases, nodes, start_index, max_failures)
    failures = 0
    workload = hashlib.sha256()
    records: list[dict[str, Any]] = []
    with tempfile.TemporaryDirectory(prefix="rmake-graph-generated-") as temporary:
        root = Path(temporary)
        inputs, workspace = root / "inputs", root / "execution"
        inputs.mkdir()
        workspace.mkdir()
        for index in range(start_index, start_index + cases):
            program = generate_program(seed, index, nodes, scopes)
            case = materialize(program, inputs)
            workload.update(index.to_bytes(8, "big"))
            workload.update(len(program.makefile).to_bytes(8, "big"))
            workload.update(program.makefile)
            record = {"case_id": program.case_id, "index": index,
                      "program_sha256": program.metadata()["program_sha256"], "status": "PASS"}
            records.append(record)
            observations = {}
            try:
                observed_reference = conformance.execute(case, workspace, reference)
                observations["oracle"] = observed_reference
                observed_candidate = conformance.execute(case, workspace, candidate)
                observations["candidate"] = observed_candidate
            except (conformance.HarnessError, OSError) as error:
                failures += 1
                saved = save_execution_failure(artifacts, program, case, identities, error, observations)
                record.update(status="ERROR", error=str(error), artifact=str(saved))
                print(f"ERROR {program.case_id}: {error}\n      evidence: {saved}")
            else:
                errors = conformance.differences(observed_reference, observed_candidate)
                errors += ["oracle: " + error for error in conformance.expectation_errors(case, observed_reference)]
                errors += ["oracle: " + error for error in program_errors(program, observed_reference)]
                errors += ["candidate: " + error for error in program_errors(program, observed_candidate)]
                if errors:
                    failures += 1
                    saved = save_failure(artifacts, program, case, workspace, identities,
                                         observed_reference, observed_candidate, errors)
                    record.update(status="FAIL", differences=errors, artifact=str(saved))
                    print(f"FAIL {program.case_id}: {', '.join(errors)}\n     evidence: {saved}")
            if len(records) % 25 == 0 or len(records) == cases:
                print(f"Progress {len(records)}/{cases} graphs; failures={failures}; seed={seed}")
            if failures >= max_failures:
                print(f"Stopped after {failures} failures; {cases - len(records)} graphs unexecuted.")
                break
    executed = len(records)
    report = {"generator_version": GENERATOR_VERSION,
              "generator_source_sha256": conformance.sha256_file(Path(__file__)),
              "python_version": sys.version, "seed": seed, "start_index": start_index,
              "cases": cases, "nodes_per_case": nodes, "executed": executed,
              "target_scopes": scopes,
              "executed_nodes": executed * nodes, "max_failures": max_failures,
              "workload_sha256": workload.hexdigest(), "identities": identities,
              "passed": executed - failures, "failed": failures, "unexecuted": cases - executed,
              "result": "FAIL" if failures else "PASS", "programs": records,
              "limitations": LIMITATIONS}
    if report_path is None:
        reports = artifacts / "campaigns"
        reports.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", suffix=".json",
                                         prefix=f"seed-{seed}-start-{start_index}-", dir=reports,
                                         delete=False) as destination:
            json.dump(report, destination, indent=2, sort_keys=True)
            destination.write("\n")
            report_path = Path(destination.name)
    else:
        report_path.parent.mkdir(parents=True, exist_ok=True)
        report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Graph results: graphs={cases}, executed={executed}, passed={executed - failures}, "
          f"failed={failures}, unexecuted={cases - executed}, nodes={nodes}, seed={seed}, "
          f"start_index={start_index}, generator=v{GENERATOR_VERSION}")
    print(f"Campaign report: {report_path}")
    return 1 if failures else 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    parser.add_argument("--cases", type=int, default=DEFAULT_CASES)
    parser.add_argument("--nodes", type=int, default=DEFAULT_NODES)
    parser.add_argument("--scopes", action="store_true", help="add explicit target-variable scope interactions")
    parser.add_argument("--start-index", type=int, default=0)
    parser.add_argument("--max-failures", type=int, default=DEFAULT_MAX_FAILURES)
    parser.add_argument("--oracle", type=Path, default=conformance.ROOT / ".tools/oracle/bin/make")
    parser.add_argument("--oracle-profile", type=Path, default=oracle.PROFILE)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--candidate", type=Path)
    mode.add_argument("--self-test", action="store_true")
    parser.add_argument("--artifacts", type=Path, default=conformance.ROOT / "artifacts/generated-graphs")
    parser.add_argument("--report", type=Path)
    args = parser.parse_args(argv)
    try:
        validate_budget(args.seed, args.cases, args.nodes, args.start_index, args.max_failures)
        profile = oracle.verify(args.oracle.expanduser().resolve(), args.oracle_profile)
        reference_identity = conformance.executable_identity(args.oracle, oracle=True)
        reference_identity["profile"] = profile
        candidate_identity = (reference_identity if args.self_test else
                              conformance.executable_identity(args.candidate))
        print("Generated trusted DAGs: exact byte/status/filesystem comparison; no security sandbox.")
        print("Timestamps, stateful sequences, parallel jobs, and cyclic graphs are outside this gate.")
        return run_campaign(seed=args.seed, cases=args.cases, nodes=args.nodes,
                            start_index=args.start_index, reference=Path(reference_identity["path"]),
                            candidate=Path(candidate_identity["path"]),
                            identities={"oracle": reference_identity, "candidate": candidate_identity},
                            artifacts=args.artifacts, report_path=args.report,
                            max_failures=args.max_failures, scopes=args.scopes)
    except (conformance.HarnessError, oracle.OracleError, OSError) as error:
        print(f"graph-generated: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
