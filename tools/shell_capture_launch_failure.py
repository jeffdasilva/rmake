#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare the live capture pipe lifetime after two ordinary launch failures."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

import conformance as h
import oracle
from shell_capture_lifetime import file_entry


ROOT = Path(h.__file__).resolve().parents[1]
FIXTURE = ROOT / "tests/shell-capture-launch-failure"
ENTRY_SOURCE = ROOT / "tests/process-restarts/entry.c"
PHASES = ("before", "after-one", "after-two", "restarted")
EXPECTED = {
    "stdout": "restart=1\n",
    "stderr": "make: ./absent: No such file or directory\nmake: ./denied: Permission denied\n",
    "exit_code": 0,
}
FILE_UPDATES = {
    "work/generated.mk": b"REBUILT := yes\n",
    "work/statuses": b"[]:127 []:127 [witness-ok]:0\n",
    "work/seen": b"anchor\nENTRY\nENTRY\n",
}


def descriptors(pid):
    proc = Path("/proc") / str(pid)
    entries = []
    for fd in sorted((proc / "fd").iterdir(), key=lambda path: int(path.name)):
        try:
            target = os.readlink(fd)
            info = dict(line.split(":", 1) for line in
                        (proc / "fdinfo" / fd.name).read_text().splitlines())
        except FileNotFoundError:
            continue
        entries.append({"fd": int(fd.name), "target": target, "info": info})
    return entries


def process_identity(pid):
    proc = Path("/proc") / str(pid)
    fields = (proc / "stat").read_text().rsplit(")", 1)[1].split()
    return {"parent": int(fields[1]), "group": int(fields[2]),
            "start": int(fields[19]), "image": os.readlink(proc / "exe")}


def pipe_id(entry):
    match = re.fullmatch(r"pipe:\[(\d+)\]", entry["target"])
    return int(match.group(1)) if match else None


def flags(entry):
    return int(entry["info"]["flags"], 8)


def pipe_pair(inode, entries):
    modes = sorted(flags(entry) & os.O_ACCMODE for entry in entries)
    return modes == [os.O_RDONLY, os.O_WRONLY] and all(
        flags(entry) & os.O_CLOEXEC and int(entry["info"]["ino"]) == inode
        for entry in entries
    )


def expected_filesystem(runtime):
    roots = {name: runtime / name for name in ("work", "home", "tmp", "bin")}
    snapshot = h.snapshot(roots)
    snapshot.update((path, file_entry(content)) for path, content in FILE_UPDATES.items())
    return roots, snapshot


def observe_run(case, runtime, image, is_oracle):
    h.reset_workspace(case, runtime, image)
    work = runtime / "work"
    roots, expected_files = expected_filesystem(runtime)
    phases = {}
    owner = None

    def observe(pid, _stdout):
        nonlocal owner
        owner = pid
        if len(phases) == len(PHASES):
            return
        phase = PHASES[len(phases)]
        ready = work / (phase + ".ready")
        if not ready.is_file():
            return
        witness = json.loads(ready.read_text())
        child_pid = witness["pid"]
        try:
            child_fds = descriptors(child_pid)
            parent_fds = descriptors(pid)
            child, parent = process_identity(child_pid), process_identity(pid)
        except FileNotFoundError:
            # The bounded capture will report a missing or failed witness.
            return
        stdout_fd = next((entry for entry in child_fds if entry["fd"] == 1), None)
        capture_id = None if stdout_fd is None else pipe_id(stdout_fd)
        capture_ends = [entry for entry in parent_fds if pipe_id(entry) == capture_id
                        and capture_id is not None]
        # Child startup can publish the marker just before the parent closes
        # its temporary copy of the writer. Sample at the stable read boundary.
        if len(capture_ends) == 2:
            return
        phases[phase] = {
            "witness": witness, "child": child, "parent": parent, "child_fds": child_fds,
            "parent_fds": parent_fds, "capture_id": capture_id,
            "entry_journal": (work / "seen").read_bytes().hex(),
        }
        (work / (phase + ".go")).touch(exist_ok=False)

    environment = h.clean_environment(runtime, {"LD_PRELOAD": str(work / "entry.so")})
    stdout, stderr, status, limit, leftovers = h.capture(
        [str(runtime / "bin/make"), *case.args], work, environment, case.timeout,
        observe=observe,
    )
    observed = h.Observation(stdout, stderr, status, limit, h.snapshot(roots), leftovers)
    errors = h.expectation_errors(case, observed)
    if observed.filesystem != expected_files:
        errors.append("complete work/home/tmp/bin filesystem")
    if owner is None or (Path("/proc") / str(owner)).exists():
        errors.append("interpreter was not reaped")
    if tuple(phases) != PHASES:
        errors.append("four ordered live witnesses")
    else:
        errors.extend(check_phases(phases, owner, image, is_oracle))
    return {"image": str(image), "sha256": h.sha256_file(image),
            "expected_filesystem": expected_files, "observed": h.observation_json(observed),
            "owner": owner, "phases": phases, "errors": errors}


def check_phases(phases, owner, image, is_oracle):
    errors = []
    first = phases["before"]["parent"]
    seen_captures = set()
    prior_pipes = {}
    baseline = {}
    for name, phase in phases.items():
        parent, child = phase["parent"], phase["child"]
        if (parent["image"] != str(image) or parent["start"] != first["start"]
                or parent["group"] != owner):
            errors.append(f"{name}: stable interpreter identity")
        if child["parent"] != owner or child["group"] != owner or phase["witness"]["parent"] != owner:
            errors.append(f"{name}: observer is a direct child")
        journal = b"anchor\nENTRY\n" if name != "restarted" else FILE_UPDATES["work/seen"]
        if phase["entry_journal"] != journal.hex():
            errors.append(f"{name}: actual executable entry count")

        current = phase["capture_id"]
        stdout = next((entry for entry in phase["child_fds"] if entry["fd"] == 1), None)
        if (stdout is None or current is None or current in seen_captures
                or flags(stdout) & os.O_ACCMODE != os.O_WRONLY):
            errors.append(f"{name}: independent current capture writer")
        seen_captures.add(current)
        if any(entry["fd"] > 2 and pipe_id(entry) is not None for entry in phase["child_fds"]):
            errors.append(f"{name}: observer inherited an unrelated pipe")

        pipes = {}
        for entry in phase["parent_fds"]:
            inode = pipe_id(entry)
            if inode is not None and entry["fd"] > 2:
                pipes.setdefault(inode, []).append(entry)
        live = pipes.pop(current, [])
        if (len(live) != 1 or flags(live[0]) & os.O_ACCMODE != os.O_RDONLY
                or not flags(live[0]) & os.O_CLOEXEC or int(live[0]["info"]["ino"]) != current):
            errors.append(f"{name}: live parent capture reader")
        if seen_captures.intersection(pipes):
            errors.append(f"{name}: earlier successful capture is still held")

        if name == "before":
            baseline = pipes
            if len(baseline) > (0 if is_oracle else 1) or not all(
                pipe_pair(inode, ends) and all(flags(end) & os.O_NONBLOCK for end in ends)
                for inode, ends in baseline.items()
            ):
                errors.append("before: baseline may contain only the candidate's private wake pipe")
        elif name == "restarted":
            if pipes.keys() & prior_pipes.keys() or len(pipes) != len(baseline) or not all(
                pipe_pair(inode, ends) and all(flags(end) & os.O_NONBLOCK for end in ends)
                for inode, ends in pipes.items()
            ):
                errors.append("restarted: old pairs absent and private baseline restored")
        else:
            failed = {inode: ends for inode, ends in pipes.items() if inode not in baseline}
            expected = 1 if name == "after-one" else 2
            if len(failed) != expected or not all(
                pipe_pair(inode, ends) and all(not flags(end) & os.O_NONBLOCK for end in ends)
                for inode, ends in failed.items()
            ):
                errors.append(f"{name}: exactly {expected} complete CLOEXEC failed-capture pairs")
            if any(pipes.get(inode) != ends for inode, ends in prior_pipes.items()):
                errors.append(f"{name}: unchanged ownership of earlier pipes")
        prior_pipes = pipes
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle", type=Path, default=ROOT / ".tools/oracle/bin/make")
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--cc", default="cc")
    args = parser.parse_args()
    reference = args.oracle.resolve(strict=True)
    profile = oracle.verify(reference)
    paths = [(reference, True), (reference, True)]
    if args.candidate is not None:
        paths.append((args.candidate.resolve(strict=True), False))
    with tempfile.TemporaryDirectory(prefix="rmake-shell-launch-failure-") as directory:
        root = Path(directory)
        fixture = root / "input"
        fixture.mkdir()
        for name in ("Makefile", "denied", "seen", "witness.py"):
            shutil.copyfile(FIXTURE / name, fixture / name)
            (fixture / name).chmod(0o644)
        subprocess.run(
            [args.cc, "-std=c11", "-Wall", "-Wextra", "-Werror", "-O2", "-shared", "-fPIC",
             str(ENTRY_SOURCE), "-o", str(fixture / "entry.so")],
            timeout=30, check=True,
        )
        (fixture / "entry.so").chmod(0o755)
        h.validate_fixture(fixture)
        case = h.Case(fixture, "shell-capture-launch-failure", "shell.capture.launch", "required",
                      ("-rR",), 5.0, {}, EXPECTED)
        runtime = root / "runtime"
        runtime.mkdir()
        observations = [observe_run(case, runtime, image, is_oracle) for image, is_oracle in paths]
        errors = [row["errors"] for row in observations]
        report = {"case": case.id, "oracle_profile": profile["profile_id"], "args": case.args,
                  "expected": EXPECTED, "source_sha256": h.sha256_file(Path(__file__)),
                  "entry_source_sha256": h.sha256_file(ENTRY_SOURCE),
                  "fixture_sha256": {name.name: h.sha256_file(name) for name in sorted(fixture.iterdir())},
                  "observations": observations, "passed": not any(errors)}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print("PASS" if report["passed"] else "FAIL", case.id, errors)
    return int(not report["passed"])


if __name__ == "__main__":
    raise SystemExit(main())
