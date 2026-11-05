#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare shared status flags after joining a validated borrowed Linux pipe."""
import argparse
import fcntl
import json
import os
from pathlib import Path
import resource
import shutil
import sys
import tempfile

import conformance as h
import oracle

ROOT = Path(h.__file__).resolve().parents[1]
FIXTURE = ROOT / "tests/jobserver-read-flags"
FIXTURE_SOURCES = ("Makefile", "witness.py")
CONFIG = json.loads((FIXTURE / "models.json").read_bytes())
MODEL_IDS = ("roomy-advertised-read", "compact-advertised-read", "roomy-no-auth-control")
OBSERVATION_FILES = ("recipe.json", "supervisor.json")


def identity(row):
    return row["dev"], row["ino"]


def flags_match(row, reference, reading, nonblock):
    return (row["pipe"] and identity(row) == identity(reference)
            and row["status_flags"] & os.O_ACCMODE == (os.O_RDONLY if reading else os.O_WRONLY)
            and bool(row["status_flags"] & os.O_NONBLOCK) == nonblock)


def check(model, engine, records):
    expected = CONFIG["expected"]
    case = "joined" if model["join"] else "control"
    recipe = records["recipe.json"]
    supervisor = records["supervisor.json"]
    before, after = supervisor["before"], supervisor["after"]
    errors = []
    if len(before) != 8 or len(after) != 8:
        return ["eight actual supervisor originals and aliases"]
    if (recipe["parent"] != supervisor["engine_pid"] or recipe["image"] != str(engine)
            or recipe["limit"] != [model["limit"]] * 2
            or (Path("/proc") / str(supervisor["engine_pid"])).exists()):
        errors.append("owned interpreter identity, limit and reap")
    if identity(before[0]) != identity(before[1]) or identity(before[2]) != identity(before[3]) or identity(before[0]) == identity(before[2]):
        errors.append("two distinct physical pipes")
    for index, (original, final) in enumerate(zip(before, after, strict=True)):
        reading = index % 2 == 0
        nonblock_before = expected["original_before_nonblock"][index]
        nonblock_after = expected[case + "_after_nonblock"][index]
        if not flags_match(original, before[index % 4], reading, nonblock_before):
            errors.append(f"before: actual descriptor {index}")
        if (not flags_match(final, original, reading, nonblock_after)
                or final["status_flags"] != original["status_flags"] | (os.O_NONBLOCK if nonblock_after else 0)
                or final["fd"] != original["fd"]
                or final["descriptor_flags"] != original["descriptor_flags"]
                or original["descriptor_flags"] != fcntl.FD_CLOEXEC):
            errors.append(f"after: exact flag change for descriptor {index}")
    if len({entry["fd"] for entry in before}) != len(before):
        errors.append("distinct supervisor descriptor aliases")
    if (supervisor["counts_before"] != expected["queued_token_counts"]
            or supervisor["counts_after"] != expected["queued_token_counts"]
            or supervisor["tokens"] != expected["seeded_tokens"]):
        errors.append("original bytes stay in both pipes")
    protocol = recipe["protocol"]
    desired_protocol = expected[case + "_child_nonblock"]
    if len(protocol) != len(desired_protocol) or any(
        not flags_match(entry, before[index], index == 0, nonblock)
        or entry["descriptor_flags"] & fcntl.FD_CLOEXEC
        for index, (entry, nonblock) in enumerate(zip(protocol, desired_protocol))
    ):
        errors.append("actual advertised child protocol flags and identity")
    if len(recipe["unrelated"]) != 2 or any(
        entry["fd"] != model["slots"][index + 2]
        or not flags_match(entry, before[index + 2], index == 0, False)
        or entry["descriptor_flags"] & fcntl.FD_CLOEXEC
        for index, entry in enumerate(recipe["unrelated"])
    ):
        errors.append("unrelated inherited control flags and identity")
    return errors


def run(model, image, fixture, runtime):
    expected = CONFIG["expected"]
    additions = {"WIRE_SLOTS": ",".join(map(str, model["slots"]))}
    if model["join"]:
        additions["MAKEFLAGS"] = "-j2 --jobserver-auth=" + ",".join(map(str, model["slots"][:2]))
    case = h.Case(fixture, model["id"], "parallel.jobserver", "required",
                  tuple(CONFIG["args"]), 5, additions,
                  {name: expected[name] for name in ("stdout", "stderr", "exit_code")})
    h.reset_workspace(case, runtime, image)
    work = runtime / "work"
    roots = {name: runtime / name for name in ("work", "home", "tmp", "bin")}
    expected_filesystem = h.snapshot(roots)
    command = [sys.executable, "-B", "witness.py", "launch", str(model["limit"]),
               str(runtime / "bin/make"), *case.args]
    out, err, code, limit, leftovers = h.capture(command, work, h.clean_environment(runtime, case.environment), case.timeout)
    observed = h.Observation(out, err, code, limit, h.snapshot(roots), leftovers)
    errors = h.expectation_errors(case, observed)
    other = dict(observed.filesystem)
    records = {}
    for name in OBSERVATION_FILES:
        evidence = other.pop("work/" + name, None)
        if evidence is None or evidence["type"] != "file" or evidence["mode"] != 0o644 or evidence.get("hardlinks"):
            errors.append("missing or invalid witness file " + name)
        else:
            records[name] = json.loads((work / name).read_bytes())
    if other != expected_filesystem:
        errors.append("complete fixture filesystem outside typed witness files")
    if len(records) == len(OBSERVATION_FILES):
        errors.extend(check(model, image, records))
    return {"model": model, "image": str(image), "sha256": h.sha256_file(image),
            "expected_filesystem": expected_filesystem,
            "observed": h.observation_json(observed), "records": records, "errors": errors}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle", type=Path, default=ROOT / ".tools/oracle/bin/make")
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    models = CONFIG["models"]
    if tuple(model["id"] for model in models) != MODEL_IDS:
        raise h.HarnessError("unexpected borrowed-read flag profile inventory")
    _, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
    if hard != resource.RLIM_INFINITY and hard < max(model["limit"] for model in models):
        raise h.HarnessError("borrowed-read profiles require a hard descriptor limit of at least 64")
    reference = args.oracle.resolve(strict=True)
    profile = oracle.verify(reference)
    images = [reference, reference]
    if args.candidate is not None:
        images.append(args.candidate.resolve(strict=True))
    with tempfile.TemporaryDirectory(prefix="rmake-jobserver-read-flags-") as directory:
        root = Path(directory)
        fixture, runtime = root / "input", root / "runtime"
        fixture.mkdir()
        runtime.mkdir()
        for name in FIXTURE_SOURCES:
            shutil.copyfile(FIXTURE / name, fixture / name)
            (fixture / name).chmod(0o644)
        h.validate_fixture(fixture)
        cases = []
        for model in models:
            observations = [run(model, image, fixture, runtime) for image in images]
            errors = [row["errors"] for row in observations]
            cases.append({"id": model["id"], "observations": observations, "passed": not any(errors)})
            print("FAIL" if any(errors) else "PASS", model["id"], errors)
    report = {
        "oracle_profile": profile["profile_id"],
        "model": CONFIG,
        "cases": cases,
        "affinity": sorted(os.sched_getaffinity(0)),
        "source_sha256": h.sha256_file(Path(__file__)),
        "fixture_sha256": {name: h.sha256_file(FIXTURE / name)
                           for name in (*FIXTURE_SOURCES, "models.json")},
        "passed": all(case["passed"] for case in cases),
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    return int(not report["passed"])


if __name__ == "__main__":
    raise SystemExit(main())
