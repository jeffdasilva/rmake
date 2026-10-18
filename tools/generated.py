#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Reproducible, independently generated pure-expansion differential programs.

Run through the top-level Makefile once its generated gate is wired. The runner
uses trusted generated fixtures and process-group containment, not a security
sandbox. It imports no rmake implementation code and never normalizes output.
"""
from __future__ import annotations

import argparse
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
DEFAULT_SEED = 44101
DEFAULT_CASES = 200
DEFAULT_EXPRESSIONS = 32
MAX_CASES = 10000
MAX_EXPRESSIONS = 256
MAX_ASSERTIONS = 1_000_000
MAX_PROGRAM_BYTES = 64 * 1024
ATOMS = (b"a", b"beta", b"gamma", b"src/a.c", b"lib/b.c", b"dir/x.h",
         b"a%b", br"a\%b", br"a\\b", b"\x80z", b"\xff.c", b"tail/")
PATTERNS = (b"a", b"beta", b"%", b"a%", b"%.c", b"src/%.c", br"a\%b", br"a\\%b", b"")
SEPARATORS = (b" ", b"  ", b"\t", b" \t ")
UNARY = (b"strip", b"sort", b"words", b"firstword", b"lastword", b"dir",
         b"notdir", b"basename", b"suffix")


@dataclasses.dataclass(frozen=True)
class Program:
    seed: int
    index: int
    expressions: tuple[bytes, ...]
    makefile: bytes
    expected_rows: tuple[tuple[int, bytes], ...]

    @property
    def case_id(self) -> str:
        return f"generated-{self.seed}-{self.index:06d}"

    def metadata(self) -> dict[str, Any]:
        return {"generator_version": GENERATOR_VERSION, "seed": self.seed,
                "case_index": self.index, "expression_count": len(self.expressions),
                "program_sha256": hashlib.sha256(self.makefile).hexdigest(),
                "expressions_hex": [expression.hex() for expression in self.expressions],
                "original_expectations": {str(index): value.hex() for index, value in self.expected_rows}}


def validate_budget(seed: int, cases: int, expressions: int, start_index: int = 0) -> None:
    limits = (("seed", seed, 0, 2**64 - 1), ("cases", cases, 1, MAX_CASES),
              ("expressions", expressions, 8, MAX_EXPRESSIONS),
              ("start-index", start_index, 0, 1_000_000_000))
    for name, value, low, high in limits:
        if type(value) is not int or not low <= value <= high:
            raise conformance.HarnessError(f"{name} must be an integer between {low} and {high}")
    if cases * expressions > MAX_ASSERTIONS:
        raise conformance.HarnessError(f"campaign exceeds {MAX_ASSERTIONS} expression observations")
    if start_index + cases > 1_000_000_001:
        raise conformance.HarnessError("campaign case index exceeds the supported range")


def words(rng: random.Random) -> bytes:
    values = [rng.choice(ATOMS) for _ in range(rng.randrange(0, 6))]
    separator = rng.choice(SEPARATORS)
    value = separator.join(values)
    if rng.randrange(2):
        value = b" " + value
    if rng.randrange(2):
        value += b"  "
    return value


def function(name: bytes, *arguments: bytes) -> bytes:
    return b"$(" + name + b" " + b",".join(arguments) + b")"


def expression(rng: random.Random, depth: int = 0) -> bytes:
    """A bounded grammar using only currently implemented pure semantics."""
    choice = rng.randrange(18 if depth < 2 else 10)
    text = words(rng)
    if choice == 0:
        return function(rng.choice(UNARY), text)
    if choice == 1:
        return function(b"subst", rng.choice(PATTERNS), rng.choice(PATTERNS), text)
    if choice == 2:
        return function(b"patsubst", rng.choice(PATTERNS), rng.choice(PATTERNS), text)
    if choice == 3:
        patterns = rng.choice(PATTERNS) + b" " + rng.choice(PATTERNS)
        return function(rng.choice((b"filter", b"filter-out")), patterns, text)
    if choice == 4:
        return function(b"word", str(rng.randrange(1, 9)).encode(), text)
    if choice == 5:
        return function(b"wordlist", str(rng.randrange(1, 6)).encode(),
                        str(rng.randrange(0, 9)).encode(), text)
    if choice == 6:
        return function(rng.choice((b"addprefix", b"addsuffix", b"join")), words(rng), text)
    if choice == 7:
        return rng.choice((b"$(RECUR)", b"$(SIMPLE)", b"$(APPEND_R)", b"$(APPEND_S)",
                           b"$(IMM)", b"$(LITERAL)", b"$(EMPTY)", b"$(FALLBACK)",
                           b"$($(KIND)_NAME)", b"$X:$Y:$$literal", b"$(MISSING)",
                           b"$(FILES:.c=.o)", b"$(FILES:src/%.c=obj/%.o)"))
    if choice == 8:
        name = rng.choice((b"RECUR", b"SIMPLE", b"APPEND_R", b"APPEND_S", b"IMM", b"EMPTY", b"MISSING"))
        return function(rng.choice((b"origin", b"flavor", b"value")), name)
    if choice == 9:
        return function(b"findstring", rng.choice(ATOMS), text)
    if choice == 10:
        inner = expression(rng, depth + 1)
        return function(b"if", b"yes", inner, b"$(error unreachable-if)")
    if choice == 11:
        inner = expression(rng, depth + 1)
        return function(b"if", b"", b"$(error unreachable-else)", inner)
    if choice == 12:
        return function(b"or", b"", b"selected", b"$(error unreachable-or)") + b":" + expression(rng, depth + 1)
    if choice == 13:
        return function(b"and", b"", b"$(error unreachable-and)") + b":" + expression(rng, depth + 1)
    if choice == 14:
        body = b"[$(item)]/$(origin item)/$(flavor item)"
        return function(b"foreach", rng.choice((b"item", b"item ")), text, body) + b"|$(item)"
    if choice == 15:
        return function(b"call", rng.choice((b"format", b"nest")), text, words(rng))
    if choice == 16:
        body = function(b"foreach", b"inner", b"x y", b"$(outer):$(inner)")
        return function(b"foreach", b"outer", text, body)
    return function(rng.choice((b"strip", b"sort", b"words")), expression(rng, depth + 1))


def generate_program(seed: int, index: int, expressions: int) -> Program:
    validate_budget(seed, 1, expressions, index)
    digest = hashlib.sha256(f"rmake-generated-v{GENERATOR_VERSION}:{seed}:{index}".encode()).digest()
    rng = random.Random(int.from_bytes(digest, "big"))
    early = rng.choice((b"early", b"first", b"before", b"bronze"))
    late = rng.choice((b"late", b"last", b"after", b"silver"))
    first = rng.choice((b"prefix", b"start", b"initial"))
    files = b"src/red.c  src/blue.c\tinclude/shared.h"
    prelude = (
        b"# SPDX-License-Identifier: GPL-3.0-or-later\n"
        b"# Independently generated; exact seed and generator metadata accompany this file.\n"
        b".DEFAULT_GOAL := all\n"
        b"BASE = " + early + b"\n"
        b"RECUR = $(BASE)\n"
        b"SIMPLE := $(BASE)\n"
        b"APPEND_R = " + first + b"\nAPPEND_R += $(BASE)\n"
        b"APPEND_S := " + first + b"\nAPPEND_S += $(BASE)\n"
        b"BASE = " + late + b"\n"
        b"EMPTY =\nEMPTY ?= $(error unreachable-conditional)\n"
        b"FALLBACK ?= chosen\nFALLBACK ?= ignored\n"
        b"IMM :::= $$(BASE)\nIMM += $(BASE)\n"
        b"LITERAL := $$(BASE)\n"
        b"FILES = " + files + b"\n"
        b"item = outside\n"
        b"format = $(2):$(1)\n"
        b"nest = $(call format,$(1),$(2))\n"
        b"KIND = chosen\nchosen_NAME = computed\nX = short\n"
        b".PHONY: all\nall:\n"
    )
    # Independently specified anchors ensure two equally wrong executions cannot
    # satisfy the gate merely by agreeing with one another.
    anchors = (
        (b"$(strip  alpha\t beta  )", b"alpha beta"),
        (b"$(if yes,selected,$(error unreachable))", b"selected"),
        (b"$(RECUR):$(SIMPLE)", late + b":" + early),
        (b"$(APPEND_R):$(APPEND_S)", first + b" " + late + b":" + first + b" " + early),
        (b"$(EMPTY):$(FALLBACK)", b":chosen"),
        (b"$(IMM):$(LITERAL)", b"$(BASE) " + late + b":$(BASE)"),
        (b"$(call foreach,x,a b,$$(x))", b"a b"),
        (b"$($(KIND)_NAME):$$literal", b"computed:$literal"),
    )
    generated = [source for source, _expected in anchors]
    generated.extend(expression(rng) for _ in range(expressions - len(anchors)))
    # All generated raw material excludes apostrophes and line endings, so the
    # shell receives each expansion as one literal printf argument. No eval,
    # quoting normalization, or candidate implementation model is involved.
    arguments = b" ".join(b"'" + f"{number:04d}:".encode() + source + b"'"
                           for number, source in enumerate(generated))
    makefile = prelude + b"\t@printf '%s\\n' " + arguments + b" > result.bin\n\t@cat result.bin\n"
    if len(makefile) > MAX_PROGRAM_BYTES:
        raise conformance.HarnessError("generated Makefile exceeds its byte budget")
    return Program(seed, index, tuple(generated), makefile,
                   tuple((number, expected) for number, (_source, expected) in enumerate(anchors)))


def materialize(program: Program, parent: Path) -> conformance.Case:
    directory = parent / program.case_id
    directory.mkdir(parents=True)
    (directory / "Makefile").write_bytes(program.makefile)
    (directory / "generation.json").write_text(json.dumps(program.metadata(), indent=2, sort_keys=True) + "\n")
    (directory / "case.toml").write_text(
        f'id = "{program.case_id}"\nrequirement = "generated.pure-expansion"\n'
        'candidate = "required"\nargs = ["-rR", "--no-print-directory"]\n'
        'timeout_seconds = 5\n'
        f'provenance = "Original generator v{GENERATOR_VERSION}; seed {program.seed}; index {program.index}"\n'
        '[expected]\nexit_code = 0\nstderr = ""\n'
    )
    return conformance.load_case(directory)


def program_errors(program: Program, observation: conformance.Observation) -> list[str]:
    errors = []
    if observation.returncode != 0:
        errors.append("program must exit successfully")
    if observation.stderr:
        errors.append("program must not emit diagnostics")
    rows = observation.stdout.split(b"\n")
    if not rows or rows.pop() != b"":
        errors.append("output must end with a newline")
    if len(rows) != len(program.expressions):
        errors.append(f"expected {len(program.expressions)} labeled expression rows, observed {len(rows)}")
    for index, row in enumerate(rows):
        if not row.startswith(f"{index:04d}:".encode()):
            errors.append(f"expression row {index} has an invalid label")
            break
    for index, expected in program.expected_rows:
        if index >= len(rows) or rows[index] != f"{index:04d}:".encode() + expected:
            errors.append(f"original expectation failed at expression {index}")
    return errors


def save_failure(directory: Path, program: Program, case: conformance.Case, workspace: Path,
                 identities: dict[str, Any], reference: conformance.Observation,
                 candidate: conformance.Observation, errors: list[str]) -> Path:
    target = conformance.save_artifact(directory, case, workspace, identities, reference,
                                       candidate, errors, "FAIL")
    metadata = program.metadata()
    metadata["reproduce_arguments"] = ["--seed", str(program.seed), "--start-index", str(program.index),
                                       "--cases", "1", "--expressions", str(len(program.expressions))]
    (target / "generation.json").write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n")
    return target


def save_execution_failure(directory: Path, program: Program, case: conformance.Case,
                           identities: dict[str, Any], error: Exception,
                           observations: dict[str, conformance.Observation]) -> Path:
    """Retain the program even if capture or filesystem observation itself fails."""
    directory.mkdir(parents=True, exist_ok=True)
    target = Path(tempfile.mkdtemp(prefix=f"{program.case_id}-", dir=directory))
    shutil.copytree(case.path, target / "input", symlinks=True)
    (target / "generation.json").write_text(json.dumps(program.metadata(), indent=2, sort_keys=True) + "\n")
    report = {"status": "ERROR", "error": str(error), "identities": identities,
              "generation": program.metadata(), "observations": {}}
    for label, observation in observations.items():
        (target / f"{label}.stdout").write_bytes(observation.stdout)
        (target / f"{label}.stderr").write_bytes(observation.stderr)
        report["observations"][label] = conformance.observation_json(observation)
    (target / "report.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    return target


def run_campaign(*, seed: int, cases: int, expressions: int, start_index: int,
                 reference: Path, candidate: Path, identities: dict[str, Any],
                 artifacts: Path, report_path: Path | None = None) -> int:
    validate_budget(seed, cases, expressions, start_index)
    failures = 0
    workload_digest = hashlib.sha256()
    records = []
    with tempfile.TemporaryDirectory(prefix="rmake-generated-") as temporary:
        root = Path(temporary)
        inputs = root / "inputs"
        workspace = root / "execution"
        inputs.mkdir()
        workspace.mkdir()
        for index in range(start_index, start_index + cases):
            program = generate_program(seed, index, expressions)
            case = materialize(program, inputs)
            workload_digest.update(index.to_bytes(8, "big"))
            workload_digest.update(len(program.makefile).to_bytes(8, "big"))
            workload_digest.update(program.makefile)
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
                artifact = save_execution_failure(artifacts, program, case, identities, error, observations)
                record.update(status="ERROR", artifact=str(artifact), error=str(error))
                print(f"ERROR {program.case_id}: {error}\n      evidence: {artifact}")
                continue
            errors = conformance.differences(observed_reference, observed_candidate)
            errors += ["oracle: " + error for error in conformance.expectation_errors(case, observed_reference)]
            errors += ["oracle: " + error for error in program_errors(program, observed_reference)]
            errors += ["candidate: " + error for error in program_errors(program, observed_candidate)]
            if errors:
                failures += 1
                artifact = save_failure(artifacts, program, case, workspace, identities,
                                         observed_reference, observed_candidate, errors)
                record.update(status="FAIL", artifact=str(artifact), differences=errors)
                print(f"FAIL {program.case_id}: {', '.join(errors)}\n     evidence: {artifact}")
            elif (index - start_index + 1) % 25 == 0 or index + 1 == start_index + cases:
                print(f"Progress {index - start_index + 1}/{cases} programs; failures={failures}; seed={seed}")
    report = {"generator_version": GENERATOR_VERSION,
              "generator_source_sha256": conformance.sha256_file(Path(__file__)),
              "python_version": sys.version, "seed": seed, "start_index": start_index,
              "cases": cases, "expressions_per_case": expressions,
              "expression_observations": cases * expressions,
              "workload_sha256": workload_digest.hexdigest(), "identities": identities,
              "passed": cases - failures, "failed": failures, "unexecuted": 0,
              "result": "FAIL" if failures else "PASS", "programs": records,
              "limitations": ["trusted-fixture process containment, no security sandbox",
                              "timestamps not compared", "single invocation only"]}
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
    print(f"Generated results: programs={cases}, expressions={cases * expressions}, "
          f"passed={cases - failures}, failed={failures}, seed={seed}, "
          f"start_index={start_index}, generator=v{GENERATOR_VERSION}")
    print(f"Campaign report: {report_path}")
    return 1 if failures else 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    parser.add_argument("--cases", type=int, default=DEFAULT_CASES)
    parser.add_argument("--expressions", type=int, default=DEFAULT_EXPRESSIONS)
    parser.add_argument("--start-index", type=int, default=0)
    parser.add_argument("--oracle", type=Path, default=conformance.ROOT / ".tools/oracle/bin/make")
    parser.add_argument("--oracle-profile", type=Path, default=oracle.PROFILE)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--candidate", type=Path)
    mode.add_argument("--self-test", action="store_true")
    parser.add_argument("--artifacts", type=Path, default=conformance.ROOT / "artifacts/generated")
    parser.add_argument("--report", type=Path, help="optional exact campaign summary path")
    args = parser.parse_args(argv)
    try:
        validate_budget(args.seed, args.cases, args.expressions, args.start_index)
        profile = oracle.verify(args.oracle.expanduser().resolve(), args.oracle_profile)
        reference_identity = conformance.executable_identity(args.oracle, oracle=True)
        reference_identity["profile"] = profile
        candidate_identity = (reference_identity if args.self_test else
                              conformance.executable_identity(args.candidate))
        print("Generated trusted fixtures: exact byte/status/filesystem comparison; no security sandbox.")
        print("Timestamps, stateful sequences, arbitrary process events, and PTYs remain outside this gate.")
        return run_campaign(seed=args.seed, cases=args.cases, expressions=args.expressions,
                            start_index=args.start_index, reference=Path(reference_identity["path"]),
                            candidate=Path(candidate_identity["path"]),
                            identities={"oracle": reference_identity, "candidate": candidate_identity},
                            artifacts=args.artifacts, report_path=args.report)
    except (conformance.HarnessError, oracle.OracleError, OSError) as error:
        print(f"generated: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
