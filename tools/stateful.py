#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Compare trusted stateful GNU Make fixtures, including relational file mtimes.

This reuses conformance process-group containment, NOT a security sandbox.
Only reviewed Makefiles may be run. File content/type/mode/link topology and
regular-file/symlink mtime relations are observed at invocation boundaries;
atime, ctime, directory times and transient writes with restored state are not.
"""
from __future__ import annotations

import argparse
import base64
import dataclasses
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import sys
import tempfile
import tomllib
from typing import Any

import conformance as harness
import oracle

ROOT = Path(__file__).resolve().parent.parent
BASE_NS = 1_600_000_000_000_000_000
MAX_CASES = 128
MAX_STEPS = 16
MAX_ACTIONS = 32
MAX_MANIFEST = 65_536
MAX_TIME_FILES = 128
DEFAULT_ARGS = ("-rR", "--no-print-directory")
LIMITATIONS = [
    "trusted fixtures; process containment is not a filesystem/network security sandbox",
    "boundary observations; transient writes with restored bytes and mtime are not observed",
    "regular-file/symlink mtime relations; no atime, ctime or directory-time comparison",
    "wall-clock-created absolute timestamps are recorded but compared relationally",
]


@dataclasses.dataclass(frozen=True)
class Action:
    op: str
    path: str
    content: str | None = None
    mtime_ns: int | None = None


@dataclasses.dataclass(frozen=True)
class Step:
    name: str
    args: tuple[str, ...]
    actions: tuple[Action, ...]
    expected: dict[str, Any]
    unchanged: tuple[str, ...] = ()
    exact_mtime: dict[str, int] = dataclasses.field(default_factory=dict)
    mtime_order: tuple[tuple[str, str, str], ...] = ()


@dataclasses.dataclass(frozen=True)
class Scenario:
    case: harness.Case
    initial: tuple[Action, ...]
    steps: tuple[Step, ...]

    @property
    def anchors(self) -> tuple[int, ...]:
        values = {BASE_NS}
        for action in self.initial + tuple(a for step in self.steps for a in step.actions):
            if action.mtime_ns is not None:
                values.add(action.mtime_ns)
        for step in self.steps:
            values.update(step.exact_mtime.values())
        return tuple(sorted(values))


@dataclasses.dataclass
class StepObservation:
    observation: harness.Observation
    before_filesystem: dict[str, Any]
    before_mtime_ns: dict[str, int]
    after_mtime_ns: dict[str, int]
    relations: dict[str, Any]


class ExecutionFailure(harness.HarnessError):
    """Preserve completed and partial observations when a boundary fails."""

    def __init__(self, message: str, completed: list[StepObservation], partial: dict[str, Any]):
        super().__init__(message)
        self.completed = completed
        self.partial = partial


def fail(message: str) -> None:
    raise harness.HarnessError(message)


def relative_path(value: Any) -> str:
    if (not isinstance(value, str) or not value or "\0" in value
            or "\\" in value or len(value) > 512):
        fail("action/expectation path must be a bounded relative POSIX path")
    path = PurePosixPath(value)
    if path.is_absolute() or any(part in ("", ".", "..") for part in value.split("/")):
        fail(f"invalid action/expectation path: {value!r}")
    return value


def timestamp(value: Any) -> int:
    # Keep deterministic dates well in the past, and within ordinary Linux filesystems.
    if type(value) is not int or not 1 <= value <= BASE_NS + 1_000_000_000_000:
        fail("mtime_ns must be an integer between 1 and the deterministic upper bound")
    return value


def actions(value: Any) -> tuple[Action, ...]:
    if not isinstance(value, list) or len(value) > MAX_ACTIONS:
        fail(f"actions must be a list of at most {MAX_ACTIONS} operations")
    result = []
    for item in value:
        if not isinstance(item, dict) or set(item) - {"op", "path", "content", "mtime_ns"}:
            fail("invalid action fields")
        op = item.get("op")
        required = {"op", "path"} | ({"mtime_ns"} if op in ("write", "utime") else set())
        if op == "write":
            required.add("content")
        if op not in ("write", "utime", "remove") or set(item) != required:
            fail("action must be write(path,content,mtime_ns), utime(path,mtime_ns), or remove(path)")
        path = relative_path(item["path"])
        content = item.get("content")
        if op == "write" and (not isinstance(content, str) or len(content.encode()) > MAX_MANIFEST):
            fail("write content must be a bounded string")
        result.append(Action(op, path, content,
                             timestamp(item["mtime_ns"]) if "mtime_ns" in item else None))
    return tuple(result)


def load_scenario(path: Path) -> Scenario:
    harness.validate_fixture(path)
    manifest = path / "case.toml"
    if manifest.is_symlink() or not manifest.is_file() or manifest.stat().st_size > MAX_MANIFEST:
        fail(f"missing, symlinked or oversized manifest: {manifest}")
    try:
        data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    except (ValueError, UnicodeError) as error:
        raise harness.HarnessError(f"invalid manifest {manifest}: {error}") from error
    if set(data) - {"id", "description", "provenance", "initial", "steps"}:
        fail(f"unknown scenario fields: {manifest}")
    ident = data.get("id")
    if not isinstance(ident, str) or not re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,79}", ident):
        fail(f"invalid scenario id: {manifest}")
    if ident != path.name:
        fail(f"scenario id must match directory: {manifest}")
    for key in ("description", "provenance"):
        if not isinstance(data.get(key), str) or not data[key].strip():
            fail(f"missing {key}: {manifest}")
    rows = data.get("steps")
    if not isinstance(rows, list) or not 1 <= len(rows) <= MAX_STEPS:
        fail(f"scenario must contain 1..{MAX_STEPS} steps: {manifest}")
    steps = []
    names = set()
    for row in rows:
        if not isinstance(row, dict) or set(row) - {"name", "args", "actions", "expected", "unchanged", "exact_mtime", "mtime_order"}:
            fail(f"invalid step fields: {manifest}")
        name = row.get("name")
        if not isinstance(name, str) or not re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,79}", name) or name in names:
            fail(f"invalid or duplicate step name: {manifest}")
        names.add(name)
        args = row.get("args", list(DEFAULT_ARGS))
        if (not isinstance(args, list) or len(args) > 32
                or any(not isinstance(a, str) or "\0" in a or len(a) > 4096 for a in args)):
            fail(f"step args must be at most 32 bounded strings: {manifest}")
        expected = row.get("expected")
        if not isinstance(expected, dict) or set(expected) != {"exit_code", "stdout", "stderr"}:
            fail(f"each step requires exact exit_code/stdout/stderr expectations: {manifest}")
        if type(expected["exit_code"]) is not int or not -127 <= expected["exit_code"] <= 255:
            fail(f"invalid expected exit_code: {manifest}")
        if any(not isinstance(expected[k], str) for k in ("stdout", "stderr")):
            fail(f"expected output must be strings: {manifest}")
        unchanged = row.get("unchanged", [])
        if not isinstance(unchanged, list) or len(unchanged) > MAX_TIME_FILES:
            fail(f"invalid unchanged paths: {manifest}")
        exact = row.get("exact_mtime", {})
        if not isinstance(exact, dict) or len(exact) > MAX_TIME_FILES:
            fail(f"invalid exact_mtime: {manifest}")
        orders = row.get("mtime_order", [])
        if not isinstance(orders, list) or len(orders) > MAX_TIME_FILES:
            fail(f"invalid mtime_order: {manifest}")
        pairs = []
        for pair in orders:
            if (not isinstance(pair, list) or len(pair) != 3
                    or pair[1] not in ("<", "<=", "=", ">=", ">")):
                fail(f"mtime_order must contain [left, relation, right] triples: {manifest}")
            pairs.append((relative_path(pair[0]), pair[1], relative_path(pair[2])))
        steps.append(Step(name, tuple(args), actions(row.get("actions", [])), expected,
                          tuple(relative_path(p) for p in unchanged),
                          {relative_path(p): timestamp(v) for p, v in exact.items()}, tuple(pairs)))
    case = harness.Case(path.resolve(), ident, "stateful.explicit-serial", "required", (), 5, {}, {})
    return Scenario(case, actions(data.get("initial", [])), tuple(steps))


def work_path(work: Path, relative: str) -> Path:
    path = work
    for part in PurePosixPath(relative).parts:
        path /= part
        if path.is_symlink():
            fail(f"action cannot traverse or replace a symlink: {relative}")
    return path


def set_time(path: Path, ns: int) -> None:
    os.utime(path, ns=(ns, ns), follow_symlinks=False)
    if path.lstat().st_mtime_ns != ns:
        fail(f"filesystem did not preserve requested nanoseconds for {path}: {ns}")


def apply_actions(work: Path, operations: tuple[Action, ...]) -> None:
    for action in operations:
        path = work_path(work, action.path)
        if action.op == "write":
            if path.exists() and not path.is_file():
                fail(f"write requires a regular file or absent path: {action.path}")
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(action.content.encode())
            set_time(path, action.mtime_ns)
        elif action.op == "utime":
            if not path.is_file():
                fail(f"utime requires existing regular file: {action.path}")
            set_time(path, action.mtime_ns)
        else:
            if not path.is_file():
                fail(f"remove requires existing regular file: {action.path}")
            path.unlink()


def mtimes(roots: dict[str, Path], filesystem: dict[str, Any]) -> dict[str, int]:
    result = {}
    for key, item in filesystem.items():
        if item["type"] not in ("file", "symlink"):
            continue
        if len(result) >= MAX_TIME_FILES:
            fail(f"timestamp observation exceeded {MAX_TIME_FILES} files")
        label, _, relative = key.partition("/")
        result[key] = (roots[label] / relative).lstat().st_mtime_ns
    return result


def compare_time(left: int, right: int) -> int:
    return (left > right) - (left < right)


def order_holds(after: dict[str, int], pair: tuple[str, str, str]) -> bool:
    left, operator, right = pair
    if f"work/{left}" not in after or f"work/{right}" not in after:
        return False
    comparison = compare_time(after[f"work/{left}"], after[f"work/{right}"])
    return comparison in {"<": (-1,), "<=": (-1, 0), "=": (0,), ">=": (0, 1), ">": (1,)}[operator]


def time_relations(before: dict[str, int], after: dict[str, int], anchors: tuple[int, ...],
                   orders: tuple[tuple[str, str, str], ...] = ()) -> dict[str, Any]:
    changes = {path: ("created" if path not in before else "removed" if path not in after
                      else compare_time(after[path], before[path])) for path in sorted(before.keys() | after.keys())}
    # Do not infer exact relative ordering for unrelated wall-clock writes: two
    # valid runs can fall on different timestamp ticks. Declared <=/>= predicates
    # can express the required order without requiring distinct clock ticks.
    return {"change": changes,
            "anchors": {path: [compare_time(value, anchor) for anchor in anchors]
                        for path, value in sorted(after.items())},
            "ordering": [[*pair, order_holds(after, pair)] for pair in orders]}


def execute(scenario: Scenario, root: Path, executable: Path) -> list[StepObservation]:
    results: list[StepObservation] = []
    partial: dict[str, Any] = {"phase": "reset"}
    try:
        harness.reset_workspace(scenario.case, root, executable)
        roots = {name: root / name for name in ("work", "home", "tmp")}
        partial["phase"] = "initial-timestamps"
        initial = harness.snapshot(roots)
        # Check 1ns resolution explicitly, then make every fixture file deterministic.
        for key in mtimes(roots, initial):
            label, _, relative = key.partition("/")
            set_time(roots[label] / relative, BASE_NS + 1)
            set_time(roots[label] / relative, BASE_NS)
        apply_actions(root / "work", scenario.initial)
        for step in scenario.steps:
            partial = {"step": step.name, "phase": "actions"}
            apply_actions(root / "work", step.actions)
            partial["phase"] = "observe-before"
            before_files = harness.snapshot(roots)
            before = mtimes(roots, before_files)
            partial.update(before_filesystem=before_files, before_mtime_ns=before, phase="capture")
            out, err, code, limit, leftover = harness.capture(
                [str(root / "bin/make"), *step.args], root / "work",
                harness.clean_environment(root), scenario.case.timeout)
            partial.update(phase="observe-after", stdout_base64=base64.b64encode(out).decode("ascii"),
                           stderr_base64=base64.b64encode(err).decode("ascii"), returncode=code,
                           limit=limit, leftover_processes=leftover)
            after_files = harness.snapshot(roots)
            after = mtimes(roots, after_files)
            result = harness.Observation(out, err, code, limit, after_files, leftover)
            results.append(StepObservation(result, before_files, before, after,
                                           time_relations(before, after, scenario.anchors, step.mtime_order)))
            if limit or leftover:
                break
        return results
    except (harness.HarnessError, OSError) as error:
        raise ExecutionFailure(f"{partial['phase']}: {error}", results, partial) from error


def expectation_errors(scenario: Scenario, results: list[StepObservation]) -> list[str]:
    errors = []
    if len(results) != len(scenario.steps):
        errors.append("oracle did not complete every step")
    for step, result in zip(scenario.steps, results):
        case = dataclasses.replace(scenario.case, expected=step.expected)
        errors.extend(f"{step.name}: {error}" for error in harness.expectation_errors(case, result.observation))
        for path in step.unchanged:
            key = f"work/{path}"
            if (key not in result.before_filesystem or key not in result.before_mtime_ns
                    or result.before_filesystem.get(key) != result.observation.filesystem.get(key)
                    or result.before_mtime_ns[key] != result.after_mtime_ns.get(key)):
                errors.append(f"{step.name}: oracle changed expected unchanged file {path}")
        for pair in step.mtime_order:
            if not order_holds(result.after_mtime_ns, pair):
                errors.append(f"{step.name}: oracle disagrees with mtime order {pair}")
        for path, value in step.exact_mtime.items():
            if result.after_mtime_ns.get(f"work/{path}") != value:
                errors.append(f"{step.name}: oracle disagrees with expected mtime_ns for {path}")
    return errors


def differences(reference: list[StepObservation], candidate: list[StepObservation]) -> list[str]:
    result = []
    if len(reference) != len(candidate):
        result.append("step-count")
    for index, (left, right) in enumerate(zip(reference, candidate), 1):
        result.extend(f"step-{index}.{field}" for field in harness.differences(left.observation, right.observation))
        if left.relations != right.relations:
            result.append(f"step-{index}.mtime-relations")
        # Inputs produced by the preceding invocation must stay equivalent too.
        if left.before_filesystem != right.before_filesystem:
            result.append(f"step-{index}.before-filesystem")
    return result


def artifact(directory: Path, scenario: Scenario, root: Path, identities: dict[str, Any],
             reference: list[StepObservation], candidate: list[StepObservation], errors: list[str],
             execution_failure: dict[str, Any] | None = None) -> Path:
    directory.mkdir(parents=True, exist_ok=True)
    target = Path(tempfile.mkdtemp(prefix=f"{scenario.case.id}-", dir=directory))
    shutil.copytree(scenario.case.path, target / "input" / scenario.case.id, symlinks=True)
    inputs = harness.snapshot({"input": scenario.case.path})
    report = {"schema_version": 1, "case": scenario.case.id, "differences": errors,
              "identities": identities, "environment": harness.clean_environment(root),
              "cwd": str(root / "work"), "alias": str(root / "bin/make"),
              "timeout_seconds": scenario.case.timeout, "baseline_mtime_ns": BASE_NS,
              "anchors_ns": scenario.anchors, "initial": [dataclasses.asdict(a) for a in scenario.initial],
              "steps": [dataclasses.asdict(step) for step in scenario.steps],
              "input_snapshot": inputs,
              "input_sha256": hashlib.sha256(json.dumps(inputs, sort_keys=True).encode()).hexdigest(),
              "platform": list(os.uname()), "limitations": LIMITATIONS,
              "execution_failure": execution_failure,
              "replay": [sys.executable, str(Path(__file__).resolve()), "--cases", str(target / "input"),
                         "--case", scenario.case.id, "--oracle", identities["oracle"]["path"],
                         "--oracle-profile", identities["oracle"]["profile_path"],
                         "--candidate", identities["candidate"]["path"]]}
    for label, results in (("oracle", reference), ("candidate", candidate)):
        rows = []
        for index, observed in enumerate(results, 1):
            prefix = f"{label}-{index:02d}"
            (target / f"{prefix}.stdout").write_bytes(observed.observation.stdout)
            (target / f"{prefix}.stderr").write_bytes(observed.observation.stderr)
            rows.append({**harness.observation_json(observed.observation),
                         "stdout_file": f"{prefix}.stdout", "stderr_file": f"{prefix}.stderr",
                         "before_filesystem": observed.before_filesystem,
                         "before_mtime_ns": observed.before_mtime_ns,
                         "after_mtime_ns": observed.after_mtime_ns, "relations": observed.relations})
        report[label] = rows
    (target / "report.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    return target


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle", type=Path, default=ROOT / ".tools/oracle/bin/make")
    parser.add_argument("--oracle-profile", type=Path, default=oracle.PROFILE)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--self-test", action="store_true")
    mode.add_argument("--candidate", type=Path)
    parser.add_argument("--cases", type=Path, default=ROOT / "tests/stateful")
    parser.add_argument("--case", action="append", default=[])
    parser.add_argument("--artifacts", type=Path, default=ROOT / "artifacts/stateful")
    parser.add_argument("--report", type=Path, help="optional exact campaign summary path")
    args = parser.parse_args(argv)
    try:
        if len(args.case) > MAX_CASES:
            fail("too many case selectors")
        if not args.cases.is_dir() or args.cases.is_symlink():
            fail("cases must be a real directory")
        paths = sorted(path for path in args.cases.iterdir() if path.is_dir())
        if not 1 <= len(paths) <= MAX_CASES:
            fail(f"expected 1..{MAX_CASES} scenarios")
        scenarios = [load_scenario(path) for path in paths]
        unknown = set(args.case) - {s.case.id for s in scenarios}
        if unknown:
            fail(f"unknown case IDs: {sorted(unknown)}")
        scenarios = [s for s in scenarios if not args.case or s.case.id in args.case]
        profile = oracle.verify(args.oracle.expanduser().resolve(), args.oracle_profile)
        reference_identity = harness.executable_identity(args.oracle, oracle=True)
        reference_identity.update(profile=profile, profile_path=str(args.oracle_profile.resolve()))
        candidate_identity = reference_identity if args.self_test else harness.executable_identity(args.candidate)
        identities = {"oracle": reference_identity, "candidate": candidate_identity}
        failures = 0
        records = []
        print("Stateful: trusted fixtures only; process containment is NOT a security sandbox.")
        print("Exact bytes/status/files and relational file mtimes; every case is required.")
        with tempfile.TemporaryDirectory(prefix="rmake-stateful-") as temporary:
            root = Path(temporary)
            for scenario in scenarios:
                input_files = harness.snapshot({"input": scenario.case.path})
                record = {"id": scenario.case.id, "status": "PASS", "steps": len(scenario.steps),
                          "input_sha256": hashlib.sha256(json.dumps(input_files, sort_keys=True).encode()).hexdigest(),
                          "file_sha256": {name: value["sha256"] for name, value in input_files.items()
                                          if value["type"] == "file"},
                          "initial": [dataclasses.asdict(action) for action in scenario.initial],
                          "step_configuration": [dataclasses.asdict(step) for step in scenario.steps],
                          "anchors_ns": scenario.anchors}
                records.append(record)
                reference, candidate = [], []
                engine = "oracle"
                execution_failure = None
                try:
                    reference = execute(scenario, root, Path(reference_identity["path"]))
                    engine = "candidate"
                    candidate = execute(scenario, root, Path(candidate_identity["path"]))
                    errors = expectation_errors(scenario, reference) + differences(reference, candidate)
                except ExecutionFailure as error:
                    if engine == "oracle":
                        reference = error.completed
                    else:
                        candidate = error.completed
                    errors = [f"{engine} execution failure: {error}"]
                    execution_failure = {"engine": engine, "message": str(error), "partial": error.partial}
                record.update(oracle_steps=len(reference), candidate_steps=len(candidate))
                if errors:
                    failures += 1
                    evidence = artifact(args.artifacts, scenario, root, identities, reference, candidate, errors, execution_failure)
                    record.update(status="ERROR" if execution_failure else "FAIL", differences=errors, artifact=str(evidence))
                    print(f"FAIL {scenario.case.id}: {', '.join(errors)}\n     evidence: {evidence}")
                else:
                    print(f"PASS {scenario.case.id} ({len(scenario.steps)} steps)")
            environment = harness.clean_environment(root)
            cwd = str(root / "work")
        report = {"schema_version": 1, "runner_sha256": harness.sha256_file(Path(__file__)),
                  "python_version": sys.version, "identities": identities,
                  "environment": environment, "cwd": cwd, "baseline_mtime_ns": BASE_NS,
                  "timeout_seconds": 5, "self_test": args.self_test,
                  "cases_directory": str(args.cases.resolve()), "cases": records,
                  "passed": len(scenarios) - failures, "failed": failures,
                  "steps_per_engine": sum(len(s.steps) for s in scenarios),
                  "result": "FAIL" if failures else "PASS", "limitations": LIMITATIONS}
        if args.report:
            args.report.parent.mkdir(parents=True, exist_ok=True)
            args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
            report_path = args.report
        else:
            reports = args.artifacts / "campaigns"
            reports.mkdir(parents=True, exist_ok=True)
            with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", suffix=".json",
                                             prefix="stateful-", dir=reports, delete=False) as stream:
                json.dump(report, stream, indent=2, sort_keys=True)
                stream.write("\n")
                report_path = Path(stream.name)
        print(f"Results: pass={len(scenarios) - failures}, fail={failures}")
        print(f"Campaign report: {report_path}")
        return int(bool(failures))
    except (harness.HarnessError, oracle.OracleError, OSError) as error:
        print(f"stateful: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
