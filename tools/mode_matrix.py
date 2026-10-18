#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Strict, bounded Cartesian recipe-mode comparisons against GNU Make 4.4.1.

These 512 original programs predate GNU source inspection. They combine eight
small recipe shapes, ordinary/.ONESHELL execution, and n/q/t/i/s flags without
modeling GNU's execution algorithm. Run only these reviewed trusted fixtures;
process-group containment is not a filesystem or network security sandbox.
"""
from __future__ import annotations

import argparse
import base64
import dataclasses
import hashlib
import itertools
import json
from pathlib import Path
import shutil
import sys
import tempfile
from typing import Any

import conformance
import oracle

MATRIX_VERSION = 1
DEFAULT_MAX_FAILURES = 20
FLAGS = "nqtis"
PROTECTED = b"mode-matrix protected input\n"
SHAPES = {
    "plain": ("", "\t@printf '%s\\n' first > first.txt\n\t@printf '%s\\n' second\n"),
    "first-force": ("", "\t+@printf '%s\\n' first > first.txt\n\t@printf '%s\\n' second\n"),
    "internal-force": ("", "\t@printf '%s\\n' first > first.txt\n\t+@printf '%s\\n' second\n"),
    "ignored-first-failure": ("", "\t-@exit 3\n\t@printf '%s\\n' survived\n"),
    "first-failure": ("", "\t@exit 3\n\t@printf '%s\\n' survived\n"),
    "modifier-only": ("", "\t@-\n\t+@\n"),
    "errexit": (".SHELLFLAGS = -ec\n", "\t@false\n\t@printf '%s\\n' survived\n"),
    "empty-first": ("", "\t\n\t@printf '%s\\n' body\n"),
}
LIMITATIONS = ["trusted-fixture process containment, no security sandbox",
               "timestamps not compared", "single invocation only",
               "fixed serial recipe-mode matrix, not exhaustive Make compatibility"]


def regular_file(content: bytes) -> dict[str, Any]:
    return {"type": "file", "mode": 0o644, "size": len(content),
            "sha256": hashlib.sha256(content).hexdigest(),
            "content": base64.b64encode(content).decode("ascii")}


# Explicit simple examples, not a model of how combined flags are resolved.
# Values: status, stdout, expected first.txt bytes (None = absent), all bytes.
ANCHORS = {
    ("plain", ()): (0, b"second\n", b"first\n", None),
    ("plain", ("n",)): (0, b"printf '%s\\n' first > first.txt\nprintf '%s\\n' second\n", None, None),
    ("plain", ("q",)): (1, b"", None, None),
    ("plain", ("t",)): (0, b"touch all\n", None, b""),
    ("modifier-only", ()): (0, b"make: 'all' is up to date.\n", None, None),
}


@dataclasses.dataclass(frozen=True)
class Cell:
    kind: str
    shape: str
    flags: tuple[str, ...]
    makefile: bytes

    @property
    def case_id(self) -> str:
        return f"mode-{self.kind}-{self.shape}-{''.join(self.flags) or 'default'}"

    @property
    def args(self) -> tuple[str, ...]:
        return ("-rR", "--no-print-directory", *("-" + flag for flag in self.flags))

    def metadata(self) -> dict[str, Any]:
        return {"case_id": self.case_id, "matrix_version": MATRIX_VERSION,
                "kind": self.kind, "shape": self.shape, "flags": list(self.flags),
                "argv_after_alias": list(self.args),
                "makefile_sha256": hashlib.sha256(self.makefile).hexdigest(),
                "protected_input_sha256": hashlib.sha256(PROTECTED).hexdigest(),
                "has_fixed_anchor": (self.shape, self.flags) in ANCHORS}


def inventory() -> tuple[Cell, ...]:
    result = []
    for kind in ("ordinary", "oneshell"):
        for enabled in itertools.product((False, True), repeat=len(FLAGS)):
            flags = tuple(flag for flag, used in zip(FLAGS, enabled) if used)
            for shape, (config, body) in SHAPES.items():
                source = ("# Original independent mode-combination probe\n"
                          + (".ONESHELL:\n" if kind == "oneshell" else "")
                          + config + "all:\n" + body)
                result.append(Cell(kind, shape, flags, source.encode("ascii")))
    return tuple(result)


def select_cells(case_ids: list[str]) -> tuple[Cell, ...]:
    cells = inventory()
    requested = set(case_ids)
    unknown = requested - {cell.case_id for cell in cells}
    if unknown:
        raise conformance.HarnessError(f"unknown case IDs: {sorted(unknown)}")
    return tuple(cell for cell in cells if not requested or cell.case_id in requested)


def validate_limit(max_failures: int) -> None:
    if type(max_failures) is not int or not 1 <= max_failures <= 512:
        raise conformance.HarnessError("max-failures must be an integer between 1 and 512")


def materialize(cell: Cell, parent: Path) -> conformance.Case:
    path = parent / cell.case_id
    path.mkdir(parents=True)
    (path / "Makefile").write_bytes(cell.makefile)
    (path / "protected-input.txt").write_bytes(PROTECTED)
    (path / "protected-input.txt").chmod(0o644)
    (path / "matrix.json").write_text(json.dumps(cell.metadata(), indent=2, sort_keys=True) + "\n")
    (path / "case.toml").write_text(
        f'id = "{cell.case_id}"\nrequirement = "matrix.cli-modes"\n'
        'candidate = "required"\nargs = ' + json.dumps(cell.args) + '\n'
        'timeout_seconds = 5\n'
        'provenance = "Original Cartesian recipe-mode probes authored before GNU source inspection; no upstream code or tests copied."\n')
    return conformance.load_case(path)


def anchor_errors(cell: Cell, observation: conformance.Observation) -> list[str]:
    errors = []
    if observation.filesystem.get("work/protected-input.txt") != regular_file(PROTECTED):
        errors.append("protected input changed")
    anchor = ANCHORS.get((cell.shape, cell.flags))
    if anchor is not None:
        status, stdout, first, target = anchor
        if (observation.returncode, observation.stdout, observation.stderr) != (status, stdout, b""):
            errors.append("fixed mode output/status anchor failed")
        for name, content in (("first.txt", first), ("all", target)):
            expected = None if content is None else regular_file(content)
            if observation.filesystem.get("work/" + name) != expected:
                errors.append(f"fixed mode file anchor failed for {name}")
    return errors


def replay_arguments(cell: Cell, reference: Path, candidate: Path,
                     profile: Path, self_test: bool) -> list[str]:
    return ["python3", str(Path(__file__).resolve()), "--oracle", str(reference),
            "--oracle-profile", str(profile),
            *(["--self-test"] if self_test else ["--candidate", str(candidate)]),
            "--case", cell.case_id]


def save_failure(artifacts: Path, cell: Cell, case: conformance.Case, workspace: Path,
                 identities: dict[str, Any], observations: dict[str, conformance.Observation],
                 errors: list[str], replay: list[str]) -> Path:
    if set(observations) == {"oracle", "candidate"}:
        path = conformance.save_artifact(artifacts, case, workspace, identities,
                                        observations["oracle"], observations["candidate"], errors, "FAIL")
    else:
        artifacts.mkdir(parents=True, exist_ok=True)
        path = Path(tempfile.mkdtemp(prefix=cell.case_id + "-", dir=artifacts))
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
    metadata = cell.metadata()
    metadata.update(replay_argv=replay, matrix_source_sha256=conformance.sha256_file(Path(__file__)))
    (path / "matrix.json").write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n")
    return path


def run_campaign(*, cells: tuple[Cell, ...], reference: Path, candidate: Path,
                 identities: dict[str, Any], artifacts: Path, report_path: Path | None = None,
                 max_failures: int = DEFAULT_MAX_FAILURES, profile: Path = oracle.PROFILE,
                 self_test: bool = False, invocation: list[str] | None = None) -> int:
    validate_limit(max_failures)
    known = {cell.case_id: cell for cell in inventory()}
    if (not cells or len(cells) > 512 or len({cell.case_id for cell in cells}) != len(cells)
            or any(known.get(cell.case_id) != cell for cell in cells)):
        raise conformance.HarnessError("campaign must select distinct original matrix cells")
    records = []
    failures = 0
    workload = hashlib.sha256()
    for cell in cells:
        workload.update(json.dumps(cell.metadata(), sort_keys=True).encode())
        workload.update(cell.makefile)
    with tempfile.TemporaryDirectory(prefix="rmake-mode-matrix-") as temporary:
        root = Path(temporary)
        inputs, workspace = root / "inputs", root / "execution"
        inputs.mkdir()
        workspace.mkdir()
        for cell in cells:
            case = materialize(cell, inputs)
            replay = replay_arguments(cell, reference, candidate, profile, self_test)
            record = cell.metadata()
            record.update(status="PASS", argv=[str(workspace / "bin/make"), *case.args])
            records.append(record)
            observations = {}
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
                    errors += [label + ": " + error for error in anchor_errors(cell, observed)]
            if errors:
                failures += 1
                if record["status"] != "ERROR":
                    record["status"] = "FAIL"
                saved = save_failure(artifacts, cell, case, workspace, identities,
                                     observations, errors, replay)
                record.update(errors=errors, artifact=str(saved), replay_argv=replay)
                print(f"{record['status']} {cell.case_id}: {', '.join(errors)}\n  evidence: {saved}", flush=True)
            if len(records) % 32 == 0 or len(records) == len(cells):
                print(f"Progress {len(records)}/{len(cells)} cells; failures={failures}", flush=True)
            if failures >= max_failures:
                print(f"Stopped after {failures} failures; {len(cells) - len(records)} cells unexecuted.", flush=True)
                break
    executed = len(records)
    report = {"matrix_version": MATRIX_VERSION,
              "matrix_source_sha256": conformance.sha256_file(Path(__file__)),
              "conformance_source_sha256": conformance.sha256_file(Path(conformance.__file__)),
              "python_version": sys.version, "invocation": invocation,
              "self_test": self_test, "oracle_profile": str(profile),
              "identities": identities, "inventory_cells": len(known),
              "cwd": str(workspace / "work"),
              "environment": conformance.clean_environment(workspace),
              "timeout_seconds": 5,
              "selected_cases": [cell.case_id for cell in cells],
              "workload_sha256": workload.hexdigest(), "max_failures": max_failures,
              "executed": executed, "passed": executed - failures,
              "failed": failures, "unexecuted": len(cells) - executed,
              "result": "FAIL" if failures else "PASS", "cells": records,
              "limitations": LIMITATIONS}
    if report_path is None:
        reports = artifacts / "campaigns"
        reports.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", suffix=".json",
                                         prefix="mode-matrix-", dir=reports, delete=False) as destination:
            json.dump(report, destination, indent=2, sort_keys=True)
            destination.write("\n")
            report_path = Path(destination.name)
    else:
        report_path.parent.mkdir(parents=True, exist_ok=True)
        report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Mode matrix v{MATRIX_VERSION}: cells={len(cells)}, executed={executed}, "
          f"passed={executed - failures}, failed={failures}, unexecuted={len(cells) - executed}")
    print(f"Campaign report: {report_path}")
    return int(bool(failures))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--self-test", action="store_true")
    mode.add_argument("--candidate", type=Path)
    parser.add_argument("--oracle", type=Path, default=conformance.ROOT / ".tools/oracle/bin/make")
    parser.add_argument("--oracle-profile", type=Path, default=oracle.PROFILE)
    parser.add_argument("--case", action="append", default=[], help="exact replay case ID; repeatable")
    parser.add_argument("--max-failures", type=int, default=DEFAULT_MAX_FAILURES)
    parser.add_argument("--artifacts", type=Path, default=conformance.ROOT / "artifacts/mode-matrix")
    parser.add_argument("--report", type=Path)
    args = parser.parse_args(argv)
    try:
        validate_limit(args.max_failures)
        cells = select_cells(args.case)
        profile_path = args.oracle_profile.expanduser().resolve()
        profile = oracle.verify(args.oracle.expanduser().resolve(), profile_path)
        reference_identity = conformance.executable_identity(args.oracle, oracle=True)
        reference_identity["profile"] = profile
        candidate_identity = (reference_identity if args.self_test else
                              conformance.executable_identity(args.candidate))
        print("Mode matrix: strict observations of trusted fixtures; not a security sandbox.")
        print("Single invocations; timestamps are outside this gate.")
        return run_campaign(cells=cells, reference=Path(reference_identity["path"]),
                            candidate=Path(candidate_identity["path"]),
                            identities={"oracle": reference_identity, "candidate": candidate_identity},
                            artifacts=args.artifacts.expanduser().resolve(),
                            report_path=args.report.expanduser().resolve() if args.report else None,
                            max_failures=args.max_failures, profile=profile_path,
                            self_test=args.self_test,
                            invocation=["python3", str(Path(__file__).resolve()),
                                        *(sys.argv[1:] if argv is None else argv)])
    except (conformance.HarnessError, oracle.OracleError, OSError) as error:
        print(f"mode-matrix: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
