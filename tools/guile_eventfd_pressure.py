# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare independent Guile wait outcomes under a late descriptor limit."""
import argparse
import json
import os
from pathlib import Path
import shutil
import tempfile

import conformance as h
import guile_frontend as g

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/guile-eventfd-pressure"


def blocking_wait(pid, reference):
    try:
        text = (Path("/proc") / str(pid) / "syscall").read_text().strip()
        fields = text.split()
        if len(fields) != 9 or not fields[0].isdigit():
            return None
        number = int(fields[0])
        if reference:
            if ((number == 61 and not (int(fields[3], 0) & 1))
                    or (number == 247 and not (int(fields[4], 0) & 1))
                    or (number == 128 and int(fields[3], 0) == 0)):
                return text
        elif ((number == 7 and int(fields[3], 0) & 0xffffffff)
              or (number == 271 and int(fields[3], 0) == 0)):
            return text
    except (OSError, ValueError):
        return None
    return None


def execute(model, directory, runtime, profile, binaries):
    fixture = directory / model["id"]
    fixture.mkdir()
    for name, content in model["files"].items():
        (fixture / name).write_bytes(content.encode())
    for name in ("recipe", "pressure.so"):
        shutil.copy2(directory / name, fixture / name)
    h.validate_fixture(fixture)
    case = h.Case(fixture, model["id"], "guile.resource.wait", "required",
                  tuple(model["args"]), 4, {}, {})
    trace = directory / "eventfd-observer"
    env = {key: profile["environment"][key] for key in g.ENVIRONMENT}
    env.update(LD_PRELOAD=str(runtime / "work" / "pressure.so"), PRESSURE_OBSERVER=str(trace))
    observations = []
    for index, binary in enumerate(binaries):
        reference = index < 2
        trace.unlink(missing_ok=True)
        h.reset_workspace(case, runtime, binary)
        roots = {name: runtime / name for name in ("work", "home", "tmp", "bin")}
        work = roots["work"]
        expected = h.snapshot(roots)
        expected.update({f"work/{name}": {"type": "file", "mode": 0o644,
                                        **g.file_fact(content.encode())}
                         for name, content in model["created"].items()})
        command = [str(roots["bin"] / "make"), *case.args]
        observed_wait = None

        def observe(pid, _stdout):
            nonlocal observed_wait
            ready = work / "ready"
            if observed_wait is not None or not ready.is_file() or ready.read_bytes() != b"recipe-executed\n":
                return
            observed_wait = blocking_wait(pid, reference)
            if observed_wait is not None:
                (work / "release.tmp").write_bytes(b"go\n")
                (work / "release.tmp").rename(work / "release")

        stdout, stderr, status, bound, left = h.capture(
            command, work, h.clean_environment(runtime, env), case.timeout,
            observe=observe if model["worker"] else None)
        filesystem = h.snapshot(roots)
        trace_bytes = trace.read_bytes() if trace.is_file() else b""
        expected_trace = model["oracle_trace" if reference else "candidate_trace"].encode()
        errors = [name for name, differs in (
            ("stdout", stdout != model["stdout"].encode()),
            ("stderr", stderr != model["stderr"].encode()),
            ("status", status != model["status"]),
            ("bounds", bool(bound or left)),
            ("actual recipe wait", model["worker"] and observed_wait is None),
            ("kernel allocation and recipe-exec observer", trace_bytes != expected_trace),
            ("complete filesystem", filesystem != expected),
        ) if differs]
        observations.append({"command": command, "stdout_hex": stdout.hex(),
                             "stderr_hex": stderr.hex(), "status": status, "bound": bound,
                             "leftovers": left, "raw_blocking_wait": observed_wait,
                             "trace_hex": trace_bytes.hex(), "expected_trace_hex": expected_trace.hex(),
                             "filesystem": filesystem, "expected_filesystem": expected, "errors": errors})
    return {"model": model, "observations": observations,
            "gnu2_qualified": not observations[0]["errors"] and not observations[1]["errors"],
            "candidate_matches": not observations[2]["errors"] if len(observations) == 3 else None}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", type=Path, required=True)
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--case", action="append", default=[])
    args = parser.parse_args()
    if args.report.exists():
        parser.error("refusing to replace earlier evidence")
    if os.uname().machine != "x86_64":
        parser.error("the causal wait observer requires Linux x86-64")
    profile = g.read_profile(args.profile)
    reference = Path(profile["binary"]["path"])
    binaries = [reference, reference]
    if args.candidate is not None:
        binaries.append(args.candidate.resolve(strict=True))
    models_path = FIXTURES / "models.json"
    available = json.loads(models_path.read_bytes())["models"]
    unknown = set(args.case) - {row["id"] for row in available}
    if unknown:
        parser.error(f"unknown cases: {sorted(unknown)}")
    models = [row for row in available if not args.case or row["id"] in args.case]
    if not models:
        parser.error("no Guile pressure cases selected")
    old = os.umask(0o022)
    try:
        with tempfile.TemporaryDirectory(prefix="rmake-guile-eventfd-") as temp:
            directory = Path(temp)
            runtime = directory / "runtime"
            runtime.mkdir()
            compilations = []
            for name, source, flags in (("recipe", "recipe.c", []),
                                         ("pressure.so", "pressure.c", ["-fPIC", "-shared"])):
                target = directory / name
                command = ["/usr/bin/cc", "-std=c11", "-Wall", "-Wextra", "-Werror", "-O2",
                           *flags, "-o", str(target), str(FIXTURES / source)]
                out, err, status, bound, left = h.capture(
                    command, directory, {"PATH": "/usr/bin:/bin", "LC_ALL": "C"}, 10)
                if status or bound or left or out or err:
                    raise h.HarnessError(f"witness compilation failed: {status} {out!r} {err!r}")
                compilations.append({"command": command, "sha256": h.sha256_file(target)})
            rows = [execute(row, directory, runtime, profile, binaries) for row in models]
    finally:
        os.umask(old)
    report = {"profile_sha256": h.sha256_file(args.profile),
              "binary_sha256": [h.sha256_file(binary) for binary in binaries],
              "functional_cpu_affinity": sorted(os.sched_getaffinity(0)),
              "source_hashes": {path.name: h.sha256_file(path) for path in
                                (Path(__file__), models_path, FIXTURES / "pressure.c", FIXTURES / "recipe.c")},
              "compilations": compilations, "results": rows}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    with args.report.open("x") as stream:
        json.dump(report, stream, indent=2)
        stream.write("\n")
    for row in rows:
        print(row["model"]["id"], row["gnu2_qualified"], row["candidate_matches"],
              [item["errors"] for item in row["observations"]], flush=True)
    return int(any(not row["gnu2_qualified"] or
                   row["candidate_matches"] is False
                   for row in rows))


if __name__ == "__main__":
    raise SystemExit(main())
