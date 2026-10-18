#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Bounded, original GNU Make 4.4.1 integer-comparison differential programs.

Every observation has an independent expected value. Python arbitrary integers
supply the numeric model, with the separately probed 4.4.1 negative-magnitude
ordering quirk preserved explicitly. No rmake implementation code is imported.
Generated fixtures are trusted; process containment is not a security sandbox.
"""
from __future__ import annotations

import argparse
import base64
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
DEFAULT_SEED = 441011
DEFAULT_CASES = 200
DEFAULT_EXPRESSIONS = 32
DEFAULT_MAX_FAILURES = 20
MAX_CASES = 10000
MAX_EXPRESSIONS = 64
MAX_OBSERVATIONS = 100000
MAX_PROGRAM_BYTES = 64 * 1024
PROTECTED = b"intcmp protected input\n"
LIMITATIONS = ["trusted generated inputs; no filesystem/network security sandbox",
               "timestamps not compared; single invocation",
               "valid numeric grammar; malformed inputs covered by curated fixtures",
               "up to 513 significant decimal digits; not exhaustive integer syntax",
               "two negative operands use pinned GNU 4.4.1 magnitude ordering"]


def regular_file(content: bytes) -> dict[str, Any]:
    return {"type": "file", "mode": 0o644, "size": len(content),
            "sha256": hashlib.sha256(content).hexdigest(),
            "content": base64.b64encode(content).decode("ascii")}


@dataclasses.dataclass(frozen=True)
class Program:
    seed: int
    index: int
    expressions: tuple[bytes, ...]
    values: tuple[bytes, ...]
    makefile: bytes

    @property
    def case_id(self) -> str:
        return f"intcmp-generated-v{GENERATOR_VERSION}-{self.seed}-{self.index:06d}"

    @property
    def expected_stdout(self) -> bytes:
        return b"".join(f"{index:04d}:".encode() + value + b"\n"
                        for index, value in enumerate(self.values))

    def identity(self) -> dict[str, Any]:
        return {"case_id": self.case_id, "generator_version": GENERATOR_VERSION,
                "seed": self.seed, "case_index": self.index,
                "expression_count": len(self.expressions),
                "program_sha256": hashlib.sha256(self.makefile).hexdigest(),
                "expected_stdout_sha256": hashlib.sha256(self.expected_stdout).hexdigest()}

    def metadata(self) -> dict[str, Any]:
        return {**self.identity(),
                "expressions_hex": [value.hex() for value in self.expressions],
                "expected_rows_hex": [value.hex() for value in self.values],
                "comparison_contract": "Python integers, except two negative operands compare absolute magnitudes"}


def validate_budget(seed: int, cases: int, expressions: int, start_index: int = 0,
                    max_failures: int = DEFAULT_MAX_FAILURES) -> None:
    for name, value, lower, upper in (
        ("seed", seed, 0, 2**64 - 1), ("cases", cases, 1, MAX_CASES),
        ("expressions", expressions, 1, MAX_EXPRESSIONS),
        ("start-index", start_index, 0, 1_000_000_000),
        ("max-failures", max_failures, 1, MAX_CASES),
    ):
        if type(value) is not int or not lower <= value <= upper:
            raise conformance.HarnessError(f"{name} must be an integer between {lower} and {upper}")
    if cases * expressions > MAX_OBSERVATIONS:
        raise conformance.HarnessError(f"campaign exceeds {MAX_OBSERVATIONS} observations")
    if start_index + cases > 1_000_000_001:
        raise conformance.HarnessError("campaign exceeds the supported case-index range")


def function(name: bytes, *arguments: bytes) -> bytes:
    return b"$(" + name + b" " + b",".join(arguments) + b")"


def compare(left: int, right: int) -> int:
    # Independently confirmed: intcmp -19,-7,L,E,G yields G in GNU Make 4.4.1.
    if left < 0 and right < 0:
        left, right = abs(left), abs(right)
    return (left > right) - (left < right)


def integer(rng: random.Random) -> int:
    if rng.randrange(5) == 0:
        return rng.choice((0, 1, -1, 2, -2, 2**63 - 1, -(2**63), 2**64, 10**100, -(10**100)))
    digits = rng.choice((1, 2, 4, 19, 20, 21, 64, 128, 257, 512))
    value = int("".join(str(rng.randrange(10)) for _ in range(digits)))
    return value * rng.choice((-1, 1))


def spelling(rng: random.Random, value: int) -> bytes:
    sign = b"-" if value < 0 else rng.choice((b"", b"+", b"-") if value == 0 else (b"", b"+"))
    body = sign + b"0" * rng.randrange(9) + str(abs(value)).encode()
    body = (rng.choice((b"", b" ", b"\t", b" \t")) + body
            + rng.choice((b"", b" ", b"\t", b"\t ")))
    return function(b"strip", body) if rng.randrange(4) == 0 else body


def expression(rng: random.Random) -> tuple[bytes, bytes]:
    left = integer(rng)
    right = rng.choice((left, left + 1, left - 1, integer(rng)))
    lhs, rhs = spelling(rng, left), spelling(rng, right)
    relation = compare(left, right)
    selected = relation + 1
    mode = rng.randrange(12)
    if mode == 0:
        return function(b"intcmp", lhs, rhs), str(left).encode() if relation == 0 else b""
    if mode == 1:
        return function(b"intcmp", lhs, rhs, b"lt"), b"lt" if relation < 0 else b""
    if mode == 2:
        return function(b"intcmp", lhs, rhs, b"lt", b"eq"), b"lt" if relation < 0 else b"eq"
    if mode == 3:
        return function(b"intcmp", lhs, rhs, b"lt", b"eq", b""), (b"lt", b"eq", b"")[selected]
    if mode == 4:
        parts = (b" lt ", b" eq ", b" gt ")
        return function(b"intcmp", lhs, rhs, *parts), parts[selected]
    if mode == 5:
        parts = (b"lt", b"eq", b"gt,tail,more")
        return function(b"intcmp", lhs, rhs, *parts), parts[selected]
    if mode == 6:
        parts = [function(b"word", b"0", b"unselected")] * 3
        parts[selected] = (b"less", b"equal", b"greater")[selected]
        return function(b"intcmp", lhs, rhs, *parts), parts[selected]
    if mode == 7:
        parts = [b"$$(word 0,unselected)"] * 3
        label = (b"less", b"equal", b"greater")[selected]
        parts[selected] = label + b"-$$(TEXT)"
        return function(b"call", b"intcmp", lhs, rhs, *parts), label + b"-selected"
    if mode == 8:
        return (function(b"call", b"intcmp", lhs, rhs, b"lt", b"eq", b"gt", b"ignored"),
                (b"lt", b"eq", b"gt")[selected])
    if mode == 9:
        inner = function(b"intcmp", lhs, rhs, b" lt  words ", b" eq  words ", b" gt  words ")
        return function(b"strip", inner), (b"lt words", b"eq words", b"gt words")[selected]
    if mode == 10:
        parts = (function(b"intcmp", b"-003", b"-3"),
                 function(b"intcmp", b"+004", b"4"),
                 function(b"intcmp", b"-000", b"+0"))
        return function(b"intcmp", lhs, rhs, *parts), (b"-3", b"4", b"0")[selected]
    return function(b"call", b"intcmp", lhs, rhs), str(left).encode() if relation == 0 else b""


def generate_program(seed: int, index: int, expressions: int) -> Program:
    validate_budget(seed, 1, expressions, index)
    digest = hashlib.sha256(f"rmake-intcmp-v{GENERATOR_VERSION}:{seed}:{index}".encode()).digest()
    rng = random.Random(int.from_bytes(digest, "big"))
    pairs = [expression(rng) for _ in range(expressions)]
    sources = tuple(source for source, _ in pairs)
    values = tuple(value for _, value in pairs)
    # Both the grammar and selected outputs exclude single-quote escapes and
    # physical recipe line breaks. Shell metacharacters remain literal inside
    # the fixed single-quoted printf arguments; no shell eval is used.
    for item in sources + values:
        if any(byte in item for byte in (b"'", b"\n", b"\r", b"\0")):
            raise conformance.HarnessError("unsafe bytes in generated recipe argument")
    source = b"# Original bounded intcmp generator\nTEXT := selected\n"
    source += b"".join(f"R{number:04d} := ".encode() + value + b"\n"
                       for number, value in enumerate(sources))
    arguments = b" ".join(f"'{number:04d}:$(R{number:04d})'".encode()
                          for number in range(expressions))
    source += b"all:\n\t@printf '%s\\n' " + arguments + b" > result.bin\n\t@cat result.bin\n"
    if len(source) > MAX_PROGRAM_BYTES:
        raise conformance.HarnessError("generated Makefile exceeds its byte budget")
    return Program(seed, index, sources, values, source)


def materialize(program: Program, parent: Path) -> conformance.Case:
    path = parent / program.case_id
    path.mkdir(parents=True)
    (path / "Makefile").write_bytes(program.makefile)
    (path / "protected-input.txt").write_bytes(PROTECTED)
    (path / "protected-input.txt").chmod(0o644)
    (path / "generation.json").write_text(json.dumps(program.metadata(), indent=2, sort_keys=True) + "\n")
    (path / "case.toml").write_text(
        f'id = "{program.case_id}"\nrequirement = "functions.intcmp"\n'
        'candidate = "required"\nargs = ["-rR", "--no-print-directory"]\n'
        'timeout_seconds = 5\n'
        'provenance = "Original seeded integer-comparison generator with independent expected values; no upstream code/tests copied."\n'
        '\n[expected]\nexit_code = 0\nstderr = ""\nstdout = '
        + json.dumps(program.expected_stdout.decode("ascii")) + "\n")
    return conformance.load_case(path)


def program_errors(program: Program, observed: conformance.Observation) -> list[str]:
    errors = []
    if observed.limit is not None:
        errors.append(f"execution hit {observed.limit} limit")
    if observed.leftover_processes:
        errors.append("execution left background processes")
    if observed.returncode != 0:
        errors.append("expected successful exit")
    if observed.stderr:
        errors.append("expected empty stderr")
    if observed.stdout != program.expected_stdout:
        errors.append("independent integer-comparison expectation failed")
    if observed.filesystem.get("work/result.bin") != regular_file(program.expected_stdout):
        errors.append("result.bin differs from independent expected bytes/type/mode")
    if observed.filesystem.get("work/protected-input.txt") != regular_file(PROTECTED):
        errors.append("protected input changed")
    return errors


def replay_arguments(program: Program, reference: Path, candidate: Path,
                     profile: Path, self_test: bool) -> list[str]:
    return ["python3", str(Path(__file__).resolve()), "--oracle", str(reference),
            "--oracle-profile", str(profile),
            *(["--self-test"] if self_test else ["--candidate", str(candidate)]),
            "--seed", str(program.seed), "--start-index", str(program.index),
            "--cases", "1", "--expressions", str(len(program.expressions))]


def save_failure(artifacts: Path, program: Program, case: conformance.Case, workspace: Path,
                 identities: dict[str, Any], observations: dict[str, conformance.Observation],
                 errors: list[str], replay: list[str]) -> Path:
    if set(observations) == {"oracle", "candidate"}:
        path = conformance.save_artifact(artifacts, case, workspace, identities,
                                        observations["oracle"], observations["candidate"], errors, "FAIL")
    else:
        artifacts.mkdir(parents=True, exist_ok=True)
        path = Path(tempfile.mkdtemp(prefix=program.case_id + "-", dir=artifacts))
        shutil.copytree(case.path, path / "input", symlinks=True)
        report = {"status": "ERROR", "errors": errors, "identities": identities,
                  "argv": [str(workspace / "bin/make"), *case.args],
                  "cwd": str(workspace / "work"),
                  "environment": conformance.clean_environment(workspace),
                  "timeout_seconds": case.timeout, "observations": {}}
        for label, observed in observations.items():
            (path / (label + ".stdout")).write_bytes(observed.stdout)
            (path / (label + ".stderr")).write_bytes(observed.stderr)
            report["observations"][label] = conformance.observation_json(observed)
        (path / "report.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    metadata = program.metadata()
    metadata.update(replay_argv=replay, generator_source_sha256=conformance.sha256_file(Path(__file__)))
    (path / "generation.json").write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n")
    return path


def run_campaign(*, seed: int, cases: int, expressions: int, start_index: int,
                 reference: Path, candidate: Path, identities: dict[str, Any], artifacts: Path,
                 report_path: Path | None = None, max_failures: int = DEFAULT_MAX_FAILURES,
                 profile: Path = oracle.PROFILE, self_test: bool = False,
                 invocation: list[str] | None = None) -> int:
    validate_budget(seed, cases, expressions, start_index, max_failures)
    source = Path(__file__).read_bytes()
    source_digest = hashlib.sha256(source).hexdigest()
    sources = artifacts / "sources"
    sources.mkdir(parents=True, exist_ok=True)
    source_snapshot = sources / f"intcmp-generated-{source_digest}.py"
    source_snapshot.write_bytes(source)
    workload = hashlib.sha256()
    # Generate one at a time, twice, to identify the complete requested workload
    # even when a failure budget stops execution early; memory stays bounded.
    for index in range(start_index, start_index + cases):
        program = generate_program(seed, index, expressions)
        workload.update(json.dumps(program.metadata(), sort_keys=True).encode())
        workload.update(program.makefile)
    records = []
    failures = 0
    with tempfile.TemporaryDirectory(prefix="rmake-intcmp-generated-") as temporary:
        root = Path(temporary)
        inputs, workspace = root / "inputs", root / "execution"
        inputs.mkdir()
        workspace.mkdir()
        for index in range(start_index, start_index + cases):
            program = generate_program(seed, index, expressions)
            case = materialize(program, inputs)
            replay = replay_arguments(program, reference, candidate, profile, self_test)
            record = {**program.identity(), "status": "PASS",
                      "argv": [str(workspace / "bin/make"), *case.args]}
            records.append(record)
            observations: dict[str, conformance.Observation] = {}
            errors = []
            try:
                observations["oracle"] = conformance.execute(case, workspace, reference)
                observations["candidate"] = conformance.execute(case, workspace, candidate)
            except (conformance.HarnessError, OSError) as error:
                errors.append(str(error))
                record["status"] = "ERROR"
            else:
                errors += conformance.differences(observations["oracle"], observations["candidate"])
                for label, observed in observations.items():
                    errors += [label + ": " + error for error in program_errors(program, observed)]
            if errors:
                failures += 1
                if record["status"] != "ERROR":
                    record["status"] = "FAIL"
                saved = save_failure(artifacts, program, case, workspace, identities,
                                     observations, errors, replay)
                record.update(errors=errors, artifact=str(saved), replay_argv=replay)
                print(f"{record['status']} {program.case_id}: {', '.join(errors)}\n  evidence: {saved}", flush=True)
            if len(records) % 32 == 0 or len(records) == cases:
                print(f"Progress {len(records)}/{cases} programs; failures={failures}", flush=True)
            if failures >= max_failures:
                print(f"Stopped after {failures} failures; {cases - len(records)} programs unexecuted.", flush=True)
                break
    executed = len(records)
    report = {"generator_version": GENERATOR_VERSION, "generator_source_sha256": source_digest,
              "generator_source_snapshot": str(source_snapshot),
              "conformance_source_sha256": conformance.sha256_file(Path(conformance.__file__)),
              "python_version": sys.version, "invocation": invocation,
              "self_test": self_test, "oracle_profile": str(profile), "identities": identities,
              "cwd": str(workspace / "work"), "environment": conformance.clean_environment(workspace),
              "timeout_seconds": 5, "seed": seed, "start_index": start_index,
              "cases": cases, "expressions": expressions, "requested_observations": cases * expressions,
              "executed_observations": executed * expressions,
              "workload_sha256": workload.hexdigest(), "max_failures": max_failures,
              "executed": executed, "passed": executed - failures, "failed": failures,
              "unexecuted": cases - executed, "result": "FAIL" if failures else "PASS",
              "programs": records, "limitations": LIMITATIONS}
    if report_path is None:
        reports = artifacts / "campaigns"
        reports.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", suffix=".json",
                                         prefix="intcmp-generated-", dir=reports, delete=False) as destination:
            json.dump(report, destination, indent=2, sort_keys=True)
            destination.write("\n")
            report_path = Path(destination.name)
    else:
        report_path.parent.mkdir(parents=True, exist_ok=True)
        report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Intcmp generator v{GENERATOR_VERSION}: seed={seed}, start-index={start_index}, "
          f"programs={cases}, executed={executed}, passed={executed - failures}, "
          f"failed={failures}, unexecuted={cases - executed}")
    print(f"Campaign report: {report_path}")
    return int(bool(failures))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--self-test", action="store_true")
    mode.add_argument("--candidate", type=Path)
    parser.add_argument("--oracle", type=Path, default=conformance.ROOT / ".tools/oracle/bin/make")
    parser.add_argument("--oracle-profile", type=Path, default=oracle.PROFILE)
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    parser.add_argument("--cases", type=int, default=DEFAULT_CASES)
    parser.add_argument("--expressions", type=int, default=DEFAULT_EXPRESSIONS)
    parser.add_argument("--start-index", type=int, default=0)
    parser.add_argument("--max-failures", type=int, default=DEFAULT_MAX_FAILURES)
    parser.add_argument("--artifacts", type=Path, default=conformance.ROOT / "artifacts/intcmp-generated")
    parser.add_argument("--report", type=Path)
    args = parser.parse_args(argv)
    try:
        validate_budget(args.seed, args.cases, args.expressions, args.start_index, args.max_failures)
        reference = args.oracle.expanduser().resolve()
        profile_path = args.oracle_profile.expanduser().resolve()
        profile = oracle.verify(reference, profile_path)
        oracle_identity = conformance.executable_identity(reference, oracle=True)
        oracle_identity["profile"] = profile
        candidate_identity = (oracle_identity if args.self_test else
                              conformance.executable_identity(args.candidate))
        identities = {"oracle": oracle_identity, "candidate": candidate_identity}
        return run_campaign(
            seed=args.seed, cases=args.cases, expressions=args.expressions, start_index=args.start_index,
            reference=Path(oracle_identity["path"]), candidate=Path(candidate_identity["path"]),
            identities=identities, artifacts=args.artifacts, report_path=args.report,
            max_failures=args.max_failures, profile=profile_path, self_test=args.self_test,
            invocation=[sys.executable, str(Path(__file__).resolve()), *(sys.argv[1:] if argv is None else argv)])
    except (conformance.HarnessError, oracle.OracleError, OSError) as error:
        print(f"intcmp-generated: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
