#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare initial output mutex completion across inherited and explicit flags."""
import argparse
from dataclasses import replace
import json
import os
from pathlib import Path
import tempfile

import conformance as h
import oracle
from debug_output import calibrate_version, models as debug_models
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


VERSION_IDS = ("explicit-missing-with-version", "explicit-missing-with-spooled-version")
VERSION_SLOT = b"@EXPLICIT_VERSION@"


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


def observe(model, case, runtime, image, version):
    if case.id in VERSION_IDS:
        expected = dict(case.expected)
        template = expected.pop("stdout").encode()
        if (version is None or template.count(VERSION_SLOT) != 1
                or VERSION_SLOT in expected["stderr"].encode() or "--version" not in case.args):
            raise h.HarnessError("invalid explicit version slot in startup mutex model")
        expected["stdout_hex"] = template.replace(VERSION_SLOT, version).hex()
        case = replace(case, expected=expected)
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
            "stdout_hex": out.hex(), "stderr_hex": err.hex(),
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
    models = []
    for filename, ids in (("models.json", IDS), ("version-models.json", VERSION_IDS)):
        config = json.loads((FIXTURE / filename).read_bytes())
        if config["schema"] != 1 or tuple(model["id"] for model in config["models"]) != ids:
            raise h.HarnessError("unexpected startup mutex profile inventory")
        models.extend(config["models"])
    reference_version, = (model.explicit_version_reference for model in debug_models()
                          if model.id == "debug-invalid-before-version")
    if reference_version is None:
        raise h.HarnessError("missing pinned GNU version banner")
    with tempfile.TemporaryDirectory(prefix="rmake-startup-output-mutex-") as directory:
        root = Path(directory)
        runtime = root / "runtime"
        runtime.mkdir()
        oracle_version, oracle_calibration = calibrate_version(reference, root / "oracle-version", reference_version)
        versions = [oracle_version, oracle_version]
        candidate_calibration = []
        if args.candidate is not None:
            candidate_version, candidate_calibration = calibrate_version(images[-1], root / "candidate-version")
            if b"eager\n" in candidate_version or b"cannot open output sync mutex" in candidate_version:
                raise h.HarnessError("version banner contains a fixture effect")
            versions.append(candidate_version)
        records = []
        for model in models:
            case = prepare(model, root / model["id"])
            observations = [observe(model, case, runtime, image, version)
                            for image, version in zip(images, versions)]
            errors = [row["errors"] for row in observations]
            records.append({"model": model, "observations": observations, "passed": not any(errors)})
            print("FAIL" if any(errors) else "PASS", case.id, errors)
    report = {"oracle_profile": profile["profile_id"], "records": records,
              "source_sha256": h.sha256_file(Path(__file__)),
              "model_sha256": h.sha256_file(FIXTURE / "models.json"),
              "version_model_sha256": h.sha256_file(FIXTURE / "version-models.json"),
              "explicit_version_calibration": {"oracle": oracle_calibration, "candidate": candidate_calibration},
              "witness_sha256": h.sha256_file(FIXTURE / "witness.py"),
              "affinity": sorted(os.sched_getaffinity(0)),
              "passed": all(row["passed"] for row in records)}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    return int(not report["passed"])


if __name__ == "__main__":
    raise SystemExit(main())
