# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Verify worker admission and real parallel reaping when eventfd is exhausted."""
import argparse
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile

import conformance as h
import guile_detached_parallel as parallel
import guile_frontend as g

ROOT = Path(__file__).resolve().parents[1] / "tests/guile-parallel-pressure"


def observer(runtime, row, trace, is_oracle):
    work = runtime / "work"
    releases = []
    last = {}

    def observe(pid, _stdout):
        if len(releases) == 2:
            return
        stage = len(releases)
        proc = Path("/proc") / str(pid)
        try:
            auth = (work / "transport.auth").read_bytes()
            step = work / "callback.step"
            raw = (proc / "syscall").read_text().strip()
            detail = {
                "auth_hex": auth.hex(), "syscall": raw,
                "alpha_ready": (work / "alpha.ready").read_text(),
                "beta_ready": (work / "beta.ready").read_text(),
                "queued": (work / "gamma.queued").exists(),
                "started": (work / "gamma.result").exists(),
                "step": step.read_text() if step.exists() else "",
                "done": (work / "callback.done").exists(),
                "eventfd_trace": trace.read_text() if trace.is_file() else "",
            }
            last.update(detail)
            expected_trace = "" if is_oracle else row["candidate_trace"]
            if (detail["alpha_ready"] != "alpha\n" or detail["beta_ready"] != "beta\n"
                    or detail["queued"] != (row["wait"] == "reserve")
                    or detail["started"] or detail["done"]
                    or detail["step"] != ("eval returned\n" if stage else "")
                    or detail["eventfd_trace"] != expected_trace):
                return
            children = parallel.live_workers(proc, pid)
            transport = parallel.transport(proc, runtime, row["style"], auth, pid)
            if not children or not transport:
                return
            kind = parallel.wait_kind(raw, row["style"], transport["read_fds"], is_oracle)
            if not kind or (is_oracle and ((kind in ("wait4", "waitid")) != (row["wait"] == "drain"))):
                return
            if releases:
                previous = releases[0]["children"]
                if children.keys() != previous.keys() or any(
                    (state["pid"], state["start_time"]) != (previous[name]["pid"], previous[name]["start_time"])
                    for name, state in children.items()
                ):
                    return
            target = "callback.resume" if stage else "callback.release"
            temporary = work / "observer.tmp"
            temporary.write_bytes(b"go\n")
            temporary.rename(work / target)
            releases.append({**detail, "pid": pid, "children": children, "transport": transport, "kind": kind})
        except (OSError, ValueError) as error:
            last["observer_error"] = str(error)

    return observe, releases, last


def run_row(row, directory, runtime, environment, binaries):
    fixture = directory / row["id"]
    fixture.mkdir()
    for name, content in row["files"].items():
        (fixture / name).write_bytes(content.encode())
    shutil.copy2(ROOT / "worker.py", fixture / "worker.py")
    shutil.copy2(directory / "pressure.so", fixture / "pressure.so")
    h.validate_fixture(fixture)
    case = h.Case(fixture, row["id"], "guile.parallel.pressure", "required",
                  tuple(row["args"]), 5, {}, {})
    trace = directory / "eventfd-observer"
    env = {**environment, "LD_PRELOAD": str(runtime / "work" / "pressure.so"),
           "PRESSURE_OBSERVER": str(trace)}
    runs = []
    for index, engine in enumerate(binaries):
        is_oracle = index < 2
        trace.unlink(missing_ok=True)
        h.reset_workspace(case, runtime, engine)
        roots = {name: runtime / name for name in ("work", "home", "tmp", "bin")}
        expected = h.snapshot(roots)
        expected["work/seen"].update(g.file_fact(row["seen"].encode()))
        observe, releases, last = observer(runtime, row, trace, is_oracle)
        command = [str(roots["bin"] / "make"), *case.args]
        stdout, stderr, status, bound, left = h.capture(
            command, roots["work"], h.clean_environment(runtime, env), case.timeout, observe=observe)
        created = row["created"].copy()
        if releases:
            first = releases[0]
            if row["style"] == "pipe":
                pair = first["transport"]
                created["transport.auth"] = f"--jobserver-auth={pair['read_fds'][0]},{pair['write_fd']}\n"
            else:
                created["transport.auth"] = f"--jobserver-auth=fifo:{runtime / 'tmp' / ('GMfifo' + str(first['pid']))}\n"
        expected.update({f"work/{name}": {"type": "file", "mode": 0o644, **g.file_fact(content.encode())}
                         for name, content in created.items()})
        actual = h.snapshot(roots)
        raw_trace = trace.read_bytes() if trace.is_file() else b""
        expected_trace = b"" if is_oracle else row["candidate_trace"].encode()
        errors = [label for label, failed in (
            ("stdout", stdout != row["stdout"].encode()), ("stderr", stderr != row["stderr"].encode()),
            ("exit status", status != row["status"]), ("bounds", bool(bound or left)),
            ("two causal worker admissions", len(releases) != 2),
            ("post-exec eventfd witness and uninterrupted admission", raw_trace != expected_trace),
            ("complete filesystem", actual != expected),
        ) if failed]
        runs.append({"command": command, "stdout_hex": stdout.hex(), "stderr_hex": stderr.hex(),
                     "status": status, "bound": bound, "leftovers": left, "releases": releases,
                     "last_observer": last, "trace_hex": raw_trace.hex(),
                     "expected_trace_hex": expected_trace.hex(), "filesystem": actual,
                     "expected_filesystem": expected, "errors": errors})
    return {"model": row, "runs": runs,
            "gnu2_qualified": all(not run["errors"] for run in runs[:2]),
            "candidate_matches": not runs[2]["errors"] if len(runs) == 3 else None}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", type=Path, required=True)
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--case", action="append", default=[])
    args = parser.parse_args()
    if args.report.exists() or os.uname().machine != "x86_64" or not sys.platform.startswith("linux"):
        parser.error("requires a fresh report path and Linux x86-64")
    profile = g.read_profile(args.profile)
    oracle = Path(profile["binary"]["path"])
    binaries = [oracle, oracle]
    if args.candidate:
        binaries.append(args.candidate.resolve(strict=True))
    environment = {key: profile["environment"][key] for key in g.ENVIRONMENT}
    environment["GNUMAKEFLAGS"] = ""
    model_path = ROOT / "models.json"
    inventory = json.loads(model_path.read_bytes())["models"]
    models = [row for row in inventory if not args.case or row["id"] in args.case]
    if not models or set(args.case) - {row["id"] for row in models}:
        parser.error("unknown or empty case selection")
    before = os.umask(0o022)
    try:
        with tempfile.TemporaryDirectory(prefix="rmake-guile-parallel-fd-") as temp:
            directory = Path(temp)
            runtime = directory / "runtime"
            runtime.mkdir()
            library = directory / "pressure.so"
            compile_command = ["/usr/bin/cc", "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror",
                               "-shared", "-fPIC", "-o", str(library), str(ROOT / "pressure.c")]
            out, err, status, bound, left = h.capture(
                compile_command, directory, {"PATH": "/usr/bin:/bin", "LC_ALL": "C"}, 10)
            if out or err or status or bound or left:
                raise h.HarnessError(f"pressure witness compile failed: {status} {out!r} {err!r}")
            compilation = {"command": compile_command, "sha256": h.sha256_file(library)}
            rows = [run_row(row, directory, runtime, environment, binaries) for row in models]
    finally:
        os.umask(before)
    report = {
        "profile_sha256": h.sha256_file(args.profile),
        "engine_sha256": [h.sha256_file(path) for path in binaries],
        "functional_cpu_affinity": sorted(os.sched_getaffinity(0)),
        "inputs_sha256": {str(path): h.sha256_file(path) for path in
                          (Path(__file__), model_path, ROOT / "worker.py", ROOT / "pressure.c", Path(parallel.__file__))},
        "compilation": compilation, "results": rows,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    with args.report.open("x") as stream:
        json.dump(report, stream, indent=2)
        stream.write("\n")
    for row in rows:
        print(row["model"]["id"], row["gnu2_qualified"], row["candidate_matches"],
              [run["errors"] for run in row["runs"]], flush=True)
    return int(any(not row["gnu2_qualified"] or
                   row["candidate_matches"] is False for row in rows))


if __name__ == "__main__":
    raise SystemExit(main())
