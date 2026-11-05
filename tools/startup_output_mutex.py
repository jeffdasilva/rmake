#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare initial output mutex completion across inherited and explicit flags."""
import argparse
import json
import os
from pathlib import Path
import tempfile

import conformance as h
import oracle
from shell_capture_lifetime import file_entry


ROOT = Path(h.__file__).resolve().parents[1]
FIXTURE = ROOT / "tests/startup-output-mutex"
IDS = (
    "inherited-gnu-missing",
    "inherited-make-missing",
    "nonempty-make-reopens-retained-lock",
    "empty-make-defers-retained-reopen",
    "invalid-repeated-before-explicit-replacement",
    "invalid-empty-explicit-completion",
    "valid-visible-before-later-effects",
    "explicit-open-before-directory-change",
    "explicit-missing-before-directory-change",
    "explicit-missing-in-inherited-reader-buffer",
    "explicit-missing-before-help",
)


def prepare(model, fixture):
    fixture.mkdir()
    for name, content in model["inputs"].items():
        path = fixture / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content.encode())
    if model["id"] == "valid-visible-before-later-effects":
        (fixture / "witness.py").write_bytes((FIXTURE / "witness.py").read_bytes())
    h.validate_fixture(fixture)
    expected = {"stdout": model["stdout"], "stderr": model["stderr"], "exit_code": model["status"]}
    return h.Case(fixture, model["id"], "output.sync", "required",
                  tuple(model["args"]), 4.0, model["environment"], expected,
                  "merged" if model["merged"] else "separate")


def observe(model, case, runtime, image):
    h.reset_workspace(case, runtime, image)
    roots = {name: runtime / name for name in ("work", "home", "tmp", "bin")}
    expected_files = h.snapshot(roots)
    for name in model["remove"]:
        expected_files.pop("work/" + name)
    if model["seen"] is not None:
        expected_files["work/seen"] = file_entry(model["seen"].encode())
    out, err, code, limit, leftovers = h.capture(
        [str(runtime / "bin/make"), *case.args], runtime / "work",
        h.clean_environment(runtime, case.environment), case.timeout, case.output == "merged",
    )
    value = h.Observation(out, err, code, limit, h.snapshot(roots), leftovers)
    errors = h.expectation_errors(case, value)
    if value.filesystem != expected_files:
        errors.append("complete work/home/tmp/bin filesystem")
    return {"image": str(image), "sha256": h.sha256_file(image),
            "expected_filesystem": expected_files, "observed": h.observation_json(value),
            "errors": errors}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle", type=Path, default=ROOT / ".tools/oracle/bin/make")
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    reference = args.oracle.resolve(strict=True)
    profile = oracle.verify(reference)
    images = [reference, reference]
    if args.candidate is not None:
        images.append(args.candidate.resolve(strict=True))
    config = json.loads((FIXTURE / "models.json").read_bytes())
    if config["schema"] != 1 or tuple(model["id"] for model in config["models"]) != IDS:
        raise h.HarnessError("unexpected startup mutex profile inventory")
    with tempfile.TemporaryDirectory(prefix="rmake-startup-output-mutex-") as directory:
        root = Path(directory)
        runtime = root / "runtime"
        runtime.mkdir()
        records = []
        for model in config["models"]:
            case = prepare(model, root / model["id"])
            observations = [observe(model, case, runtime, image) for image in images]
            errors = [row["errors"] for row in observations]
            records.append({"model": model, "observations": observations, "passed": not any(errors)})
            print("FAIL" if any(errors) else "PASS", case.id, errors)
    report = {"oracle_profile": profile["profile_id"], "records": records,
              "source_sha256": h.sha256_file(Path(__file__)),
              "model_sha256": h.sha256_file(FIXTURE / "models.json"),
              "witness_sha256": h.sha256_file(FIXTURE / "witness.py"),
              "affinity": sorted(os.sched_getaffinity(0)),
              "passed": all(row["passed"] for row in records)}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    return int(not report["passed"])


if __name__ == "__main__":
    raise SystemExit(main())
