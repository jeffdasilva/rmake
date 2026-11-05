#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare glibc getopt diagnostics and GNU Make usage without normalizing streams."""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import tempfile

import conformance as h
import locale_data
import native_modules as native
import oracle

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "tests/startup-usage"
FIXTURE = "getopt_locale.c"


def digest(value):
    return hashlib.sha256(value).hexdigest()


def run_case(row, root, runtime, reference, candidate, locale, compiler):
    model = native.Model(
        row["id"], {name: bytes.fromhex(value) for name, value in row["files"].items()},
        {key: row["expected"][key] for key in ("exit_code", "stdout_hex")},
        bytes.fromhex(row["expected"]["seen_hex"]),
        (native.Module(FIXTURE, "getopt_locale.so", (f'PRIVATE_MESSAGES={int(row["private_messages"])}',)),),
        ("-rR", "--no-print-directory", *row["args"]),
        {**row["environment"], **locale["environment"]},
    )
    case, compilation = native.prepare(model, root / model.id, compiler)
    h.reset_workspace(case, runtime, reference)
    filesystem = h.snapshot({name: runtime / name for name in ("work", "home", "tmp")})
    filesystem["work/seen"].update(content=base64.b64encode(model.seen).decode(),
                                   size=len(model.seen), sha256=digest(model.seen))
    invocation = os.fsencode(runtime / "bin/make")
    prefix = bytes.fromhex(row["expected"]["stderr_prefix_hex"]).replace(b"@ARGV@", invocation)
    observations = []
    first = None
    for executable in (reference, reference, candidate):
        value = h.execute(case, runtime, executable)
        errors = h.expectation_errors(case, value)
        if not value.stderr.startswith(prefix):
            errors.append("getopt stderr prefix differs from the typed model")
        elif digest(value.stderr[len(prefix):]) != row["expected"]["usage_sha256"]:
            errors.append("full GNU usage transcript hash differs")
        if value.filesystem != filesystem:
            errors.append("full filesystem differs")
        differences = h.differences(first, value) if first is not None else []
        observations.append(dict(h.observation_json(value), stdout_hex=value.stdout.hex(),
                                 stderr_hex=value.stderr.hex(), errors=errors, differences=differences))
        if first is None:
            first = value
        if value.limit or value.leftover_processes or value.returncode < 0:
            break
    passed = [not row["errors"] and not row["differences"] for row in observations]
    return {"id": model.id, "model": row, "compilation": compilation, "expected_filesystem": filesystem,
            "observations": observations, "gnu2": len(passed) >= 2 and all(passed[:2]),
            "pass": len(passed) == 3 and all(passed)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle", type=Path, default=oracle.PREFIX / "bin/make")
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--locale-root", type=Path, default=ROOT / ".tools/locale-data")
    parser.add_argument("--models", type=Path, default=DATA / "getopt-locale.json")
    parser.add_argument("--cc", default="/usr/bin/cc")
    parser.add_argument("--case", action="append", default=[])
    parser.add_argument("--report", type=Path, required=True)
    opts = parser.parse_args()
    plan = json.loads(opts.models.read_bytes())
    reference = opts.oracle.resolve()
    identity = oracle.verify(reference)
    if h.sha256_file(reference) != plan["reference_sha256"]:
        raise h.HarnessError("unexpected reference executable")
    catalog = reference.parent.parent / "share/locale/fr/LC_MESSAGES/make.mo"
    if h.sha256_file(catalog) != plan["reference_make_catalog_sha256"]:
        raise h.HarnessError("unexpected reference French Make catalog")
    locale = locale_data.provision(opts.locale_root.resolve())
    if (locale["lock_sha256"], locale["generated_tree_sha256"]) != (
            plan["locale_lock_sha256"], plan["locale_generated_tree_sha256"]):
        raise h.HarnessError("unexpected locale data")
    native.SOURCES = opts.models.parent
    if h.sha256_file(native.SOURCES / FIXTURE) != plan["fixture_sha256"]:
        raise h.HarnessError("unexpected constructor fixture source")
    selected = [row for row in plan["rows"] if not opts.case or row["id"] in opts.case]
    if not selected or set(opts.case) - {row["id"] for row in selected}:
        raise h.HarnessError("unknown exact getopt locale case")
    report = {"models_sha256": h.sha256_file(opts.models),
              "tool_sha256": h.sha256_file(Path(__file__)),
              "oracle_profile": identity["profile_id"], "locale_profile": locale,
              "reference": h.executable_identity(reference, oracle=True),
              "candidate": h.executable_identity(opts.candidate),
              "functional_cpu_affinity": sorted(os.sched_getaffinity(0)), "records": []}

    def publish():
        opts.report.parent.mkdir(parents=True, exist_ok=True)
        opts.report.write_text(json.dumps(report, indent=2) + "\n")

    previous_umask = os.umask(0o022)
    try:
        with tempfile.TemporaryDirectory(prefix="rmake-getopt-locale-") as path:
            root = Path(path)
            runtime = root / "runtime"
            runtime.mkdir()
            for row in selected:
                result = run_case(row, root, runtime, reference, opts.candidate.resolve(), locale, opts.cc)
                report["records"].append(result)
                publish()
                if any(item["limit"] or item["leftover_processes"] or item["returncode"] < 0
                       for item in result["observations"]):
                    raise h.HarnessError("new signal or bound retained; stopped without replay")
                print(row["id"], result["gnu2"], result["pass"], flush=True)
    finally:
        os.umask(previous_umask)
    records = report["records"]
    report["totals"] = {"cases": len(records), "gnu2": sum(row["gnu2"] for row in records),
                        "candidate": sum(row["pass"] for row in records)}
    publish()
    print(report["totals"])
    return int(report["totals"]["candidate"] != len(records))


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (h.HarnessError, locale_data.ProfileError, oracle.OracleError, OSError) as error:
        raise SystemExit(str(error)) from error
