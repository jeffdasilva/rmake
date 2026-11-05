#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare authentication at the private wake slots in the roomy glibc profile."""
import argparse
import array
import fcntl
import json
import os
from pathlib import Path
import shutil
import stat
import sys
import tempfile
import termios

import conformance as h
import oracle
from shell_capture_launch_failure import process_identity, require_roomy_descriptor_limit


ROOT = Path(h.__file__).resolve().parents[1]
FIXTURE = ROOT / "tests/jobserver-wake-slots"
SOURCES = ("Makefile", "launch.py", "witness.py")
WIRE = (16, 17, 18, 19)
MODEL_IDS = ("closed-high-slots", "closed-high-reversed", "valid-high-aliases")


def descriptor(pid, fd):
    proc = Path("/proc") / str(pid)
    path = proc / "fd" / str(fd)
    try:
        target = os.readlink(path)
        metadata = path.stat()
        info = dict(line.split(":", 1) for line in
                    (proc / "fdinfo" / str(fd)).read_text().splitlines())
    except FileNotFoundError:
        return None
    return {"target": target, "dev": metadata.st_dev, "ino": metadata.st_ino,
            "fifo": stat.S_ISFIFO(metadata.st_mode), "flags": int(info["flags"], 8)}


def endpoint_matches(endpoint, identity, fd, cloexec):
    direction = os.O_RDONLY if fd % 2 == 0 else os.O_WRONLY
    return (endpoint is not None and endpoint["fifo"]
            and (endpoint["dev"], endpoint["ino"]) == (identity["dev"], identity["ino"])
            and endpoint["flags"] & os.O_ACCMODE == direction
            and bool(endpoint["flags"] & os.O_CLOEXEC) == cloexec)


def check_live(live, image, inherited, is_oracle, expected):
    if not live:
        return ["bounded recipe witness missing"]
    owner, child = live["pid"], live["child"]
    errors = []
    if (live["reported_parent"] != owner or live["child_identity"]["parent"] != owner
            or live["child_identity"]["group"] != owner or live["parent_identity"]["group"] != owner
            or live["parent_identity"]["image"] != str(image)):
        errors.append("direct live recipe and executable identity")
    if any((Path("/proc") / str(pid)).exists() for pid in (owner, child)):
        errors.append("process remained after reap")

    host, recipe = live["host_slots"], live["child_slots"]
    if inherited:
        wire = live["prepared_identity"]
        if live["pending_token_bytes"] != expected["preseeded_bytes"]:
            errors.append("original live pool token count changed")
        for fd in WIRE:
            if not endpoint_matches(host[fd], wire, fd, cloexec=fd in (18, 19)):
                errors.append(f"retained genuine high alias {fd}")
        for fd in (16, 17):
            if not endpoint_matches(recipe[fd], wire, fd, cloexec=False):
                errors.append(f"unadvertised inherited alias {fd}")
        for kind, table, fds in (("host", host, WIRE), ("recipe", recipe, (16, 17))):
            for fd in fds:
                entry = table[fd]
                if entry is not None and bool(entry["flags"] & os.O_NONBLOCK) != (fd in (16, 18)):
                    errors.append(f"{kind} inherited alias {fd} open-file status")
        if any(recipe[fd] is not None for fd in (18, 19)):
            errors.append("advertised descriptors leaked into a nonrecursive recipe")

    if is_oracle:
        if not inherited and any(host[fd] is not None for fd in (16, 17)):
            errors.append("GNU closed high slots unexpectedly open")
    else:
        high = (20, 21) if inherited else (16, 17)
        reader, writer = (host[fd] for fd in high)
        if (reader is None or writer is None
                or not endpoint_matches(reader, reader, high[0], cloexec=True)
                or not endpoint_matches(writer, reader, high[1], cloexec=True)
                or not all(end["flags"] & os.O_NONBLOCK for end in (reader, writer))):
            errors.append("actual private high wake endpoints")
        if any(recipe[fd] is not None for fd in high):
            errors.append("private high wake pipe leaked into a recipe")
    return errors


def execute(model, config, image, is_oracle, fixture, runtime):
    expected = config["expected"]
    inherited = model["inherited"]
    prefix = "inherited" if inherited else "closed"
    anchors = {"exit_code": expected["exit_code"], "stdout": expected[prefix + "_stdout"],
               "stderr": expected[prefix + "_stderr"]}
    case = h.Case(fixture, model["id"], "parallel.jobserver", "required",
                  tuple(config["args"]), 4.0, {"MAKEFLAGS": "-j2 --jobserver-auth=" + model["auth"]},
                  anchors)
    h.reset_workspace(case, runtime, image)
    work = runtime / "work"
    roots = {name: runtime / name for name in ("work", "home", "tmp", "bin")}
    expected_files = h.snapshot(roots)
    command = [str(runtime / "bin/make"), *case.args]
    if inherited:
        command = [sys.executable, "-B", "launch.py", *command]
    live = {}

    def observe(pid, _stdout):
        ready = work / "ready"
        if live or not ready.is_file():
            return
        try:
            child, reported_parent = map(int, ready.read_text().split())
            parent_identity, child_identity = process_identity(pid), process_identity(child)
        except FileNotFoundError:
            return
        live.update(pid=pid, child=child, reported_parent=reported_parent,
                    parent_identity=parent_identity, child_identity=child_identity,
                    host_slots={fd: descriptor(pid, fd) for fd in range(16, 22)},
                    child_slots={fd: descriptor(child, fd) for fd in range(16, 22)})
        if inherited:
            wire = json.loads((work / "wire.ready").read_bytes())
            live["prepared_identity"] = wire
            endpoint = live["host_slots"][16]
            live["pending_token_bytes"] = None
            if endpoint_matches(endpoint, wire, 16, cloexec=False):
                with os.fdopen(os.open(f"/proc/{pid}/fd/16",
                                       os.O_RDONLY | os.O_NONBLOCK | os.O_CLOEXEC), "rb") as reader:
                    count = array.array("i", [0])
                    fcntl.ioctl(reader.fileno(), termios.FIONREAD, count, True)
                    live["pending_token_bytes"] = count[0]
        (work / "release").touch(exist_ok=False)

    out, err, code, limit, leftovers = h.capture(
        command, work, h.clean_environment(runtime, case.environment), case.timeout, observe=observe,
    )
    observed = h.Observation(out, err, code, limit, h.snapshot(roots), leftovers)
    errors = h.expectation_errors(case, observed)
    if observed.filesystem != expected_files:
        errors.append("complete work/home/tmp/bin filesystem")
    errors.extend(check_live(live, image, inherited, is_oracle, expected))
    return {"model": model, "image": str(image), "sha256": h.sha256_file(image),
            "live": live, "expected_filesystem": expected_files,
            "observed": h.observation_json(observed), "errors": errors}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle", type=Path, default=ROOT / ".tools/oracle/bin/make")
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    require_roomy_descriptor_limit()
    reference = args.oracle.resolve(strict=True)
    profile = oracle.verify(reference)
    paths = [(reference, True), (reference, True)]
    if args.candidate is not None:
        paths.append((args.candidate.resolve(strict=True), False))
    config = json.loads((FIXTURE / "models.json").read_bytes())
    if tuple(model["id"] for model in config["models"]) != MODEL_IDS:
        raise h.HarnessError("unexpected roomy jobserver wake-slot inventory")
    with tempfile.TemporaryDirectory(prefix="rmake-jobserver-wake-slots-") as directory:
        fixture = Path(directory) / "input"
        runtime = Path(directory) / "runtime"
        fixture.mkdir()
        runtime.mkdir()
        for name in SOURCES:
            shutil.copyfile(FIXTURE / name, fixture / name)
            (fixture / name).chmod(0o644)
        h.validate_fixture(fixture)
        cases = []
        for model in config["models"]:
            observations = [execute(model, config, image, is_oracle, fixture, runtime)
                            for image, is_oracle in paths]
            errors = [row["errors"] for row in observations]
            cases.append({"id": model["id"], "observations": observations, "passed": not any(errors)})
            print("FAIL" if any(errors) else "PASS", model["id"], errors)
    report = {"oracle_profile": profile["profile_id"], "model": config, "cases": cases,
              "source_sha256": h.sha256_file(Path(__file__)),
              "fixture_sha256": {name: h.sha256_file(FIXTURE / name)
                                 for name in (*SOURCES, "models.json")},
              "passed": all(case["passed"] for case in cases)}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    return int(not report["passed"])


if __name__ == "__main__":
    raise SystemExit(main())
