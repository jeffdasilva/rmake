# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Verify logical job count can exceed the physical descriptor budget."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import shutil
import tempfile

import conformance as h
import oracle

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/resource-fd-parallel"


def file_record(value):
    raw = value.encode()
    return {"type": "file", "mode": 0o644, "size": len(raw),
            "sha256": hashlib.sha256(raw).hexdigest(), "content": base64.b64encode(raw).decode()}


def execute(model, directory, runtime, witness, binaries):
    fixture = directory / model["id"]
    fixture.mkdir()
    for name, content in model["files"].items():
        (fixture / name).write_bytes(content.encode())
    shutil.copy2(witness, fixture / "barrier")
    h.validate_fixture(fixture)
    case = h.Case(fixture, model["id"], "resource.parallel-fd-budget", "required",
                  tuple(model["args"]), 3, {}, {})
    observations = []
    for binary in binaries:
        h.reset_workspace(case, runtime, binary)
        roots = {name: runtime / name for name in ("work", "home", "tmp", "bin")}
        expected = h.snapshot(roots)
        expected.update({f"work/{name}": file_record(value)
                         for name, value in model["created"].items()})
        limit = model["nofile"]
        command = ["/usr/bin/prlimit", f"--nofile={limit}:{limit}", "--",
                   str(roots["bin"] / "make"), *case.args]
        stdout, stderr, status, bound, left = h.capture(
            command, roots["work"], h.clean_environment(runtime), case.timeout)
        filesystem = h.snapshot(roots)
        errors = [name for name, differs in (
            ("stdout", stdout != model["stdout"].encode()),
            ("stderr", stderr != model["stderr"].encode()),
            ("status", status != model["status"]),
            ("bounds", bool(bound or left)),
            ("complete filesystem", filesystem != expected),
        ) if differs]
        observations.append({"command": command, "stdout_hex": stdout.hex(),
                             "stderr_hex": stderr.hex(), "status": status, "bound": bound,
                             "leftovers": left, "filesystem": filesystem,
                             "expected_filesystem": expected, "errors": errors})
    return {"model": model, "observations": observations,
            "gnu2_qualified": observations[0] == observations[1] and not observations[0]["errors"],
            "candidate_matches": not observations[2]["errors"] if len(observations) == 3 else None}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle", type=Path, default=ROOT / ".tools/oracle/bin/make")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--candidate", type=Path)
    mode.add_argument("--oracle-only", action="store_true")
    parser.add_argument("--report", type=Path, default=ROOT / "artifacts/resource-fd-parallel/campaign.json")
    parser.add_argument("--case", action="append", default=[])
    parser.add_argument("--cc", default="cc")
    args = parser.parse_args()
    reference = args.oracle.resolve(strict=True)
    candidate = args.candidate.resolve(strict=True) if args.candidate is not None else None
    profile = oracle.verify(reference)
    binaries = (reference, reference) if candidate is None else (reference, reference, candidate)
    models_path = FIXTURES / "models.json"
    source = FIXTURES / "barrier.c"
    available = json.loads(models_path.read_bytes())["models"]
    unknown = set(args.case) - {row["id"] for row in available}
    if unknown:
        parser.error(f"unknown cases: {sorted(unknown)}")
    if not available:
        parser.error("no descriptor-budget cases are available")
    models = [row for row in available if not args.case or row["id"] in args.case]
    compiler_path = shutil.which(args.cc)
    if compiler_path is None:
        parser.error("the independent barrier witness requires a C compiler")
    old = os.umask(0o022)
    try:
        with tempfile.TemporaryDirectory(prefix="rmake-compact-poll-") as temp:
            directory = Path(temp)
            runtime = directory / "runtime"
            runtime.mkdir()
            witness = directory / "barrier"
            command = [compiler_path, "-std=c11", "-Wall", "-Wextra", "-Werror", "-O2",
                       "-o", str(witness), str(source)]
            out, err, status, bound, left = h.capture(
                command, directory, {"PATH": "/usr/bin:/bin", "LC_ALL": "C"}, 10)
            if status or bound or left:
                raise h.HarnessError(f"barrier compilation failed: {status} {out!r} {err!r}")
            compiler = {"command": command, "sha256": h.sha256_file(witness)}
            rows = [execute(row, directory, runtime, witness, binaries)
                    for row in models]
    finally:
        os.umask(old)
    report = {"oracle_profile": profile["profile_id"], "oracle_sha256": h.sha256_file(reference),
              "candidate_sha256": h.sha256_file(candidate) if candidate is not None else None,
              "functional_cpu_affinity": sorted(os.sched_getaffinity(0)),
              "source_hashes": {path.name: h.sha256_file(path) for path in
                                (Path(__file__), models_path, source)},
              "compilation": compiler, "results": rows}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    with args.report.open("w") as stream:
        json.dump(report, stream, indent=2)
        stream.write("\n")
    for row in rows:
        last = row["observations"][-1]
        print(row["model"]["id"], row["gnu2_qualified"], row["candidate_matches"],
              [item["errors"] for item in row["observations"]],
              last["status"], last["bound"], last["leftovers"], flush=True)
    return int(any(not row["gnu2_qualified"] or row["candidate_matches"] is False for row in rows))


if __name__ == "__main__":
    raise SystemExit(main())
