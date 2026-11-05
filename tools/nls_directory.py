#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Exact GNU2 comparison of localized directory framing and cwd failures."""
import argparse
import json
import os
from pathlib import Path
import sys
import tempfile

import conformance as h
import directory_reporting as directory
import locale_data
import nls_diagnostics as nls
import oracle as reference

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "tests/nls"
ENOENT_WITNESS = """
import ctypes
import errno
import locale
import os
locale.setlocale(locale.LC_ALL, "")
function = ctypes.CDLL(None).strerror
function.argtypes = [ctypes.c_int]
function.restype = ctypes.c_char_p
os.write(1, function(errno.ENOENT))
"""


def observe(case, runtime, binary, launcher):
    if launcher is None:
        return h.execute(case, runtime, binary)
    h.reset_workspace(case, runtime, binary)
    executable = str(runtime / "bin/make")
    command = [sys.executable, str(DATA / "deleted_cwd.py"), launcher,
               str(runtime / "work"), executable, *case.args]
    out, err, status, limit, leftovers = h.capture(
        command, runtime / "work", h.clean_environment(runtime, case.environment), case.timeout,
    )
    files = h.snapshot({name: runtime / name for name in ("work", "home", "tmp")})
    return h.Observation(out, err, status, limit, files, leftovers)


def errno_witness(root, environment):
    # Query the OS diagnostic independently, with the same locale inputs.
    # The Make template, prefix, spacing, newline and stream remain literal.
    out, err, status, limit, leftovers = h.capture(
        [sys.executable, "-c", ENOENT_WITNESS], root,
        h.clean_environment(root, environment), 5,
    )
    if status or err or limit or leftovers or not out or b"\n" in out:
        raise h.HarnessError("could not witness host libc ENOENT diagnostic")
    return out


def load_rows(selection):
    data = json.loads((DATA / "directory.json").read_bytes())
    rows = data["rows"]
    names = {row["id"] for row in rows}
    if len(rows) != 17 or len(names) != 17:
        raise h.HarnessError("unexpected NLS directory inventory")
    if set(selection) - names:
        raise h.HarnessError("unknown exact NLS directory ID")
    fixtures = data["fixture_sha256"]
    if set(fixtures) != {"nls_control.c", "deleted_cwd.py"}:
        raise h.HarnessError("unexpected NLS directory fixture inventory")
    for name, expected in fixtures.items():
        if h.sha256_file(DATA / name) != expected:
            raise h.HarnessError("NLS directory fixture identity differs: " + name)
    for row in rows:
        if any(module["source"] != "nls_control.c" for module in row["modules"]):
            raise h.HarnessError("NLS directory model uses unpinned source")
        if row["launch"] not in (None, "initial", "after-C"):
            raise h.HarnessError("unknown NLS directory launch")
    return data, [row for row in rows if not selection or row["id"] in selection]


def make_model(row, root, locale):
    environment = {**row["environment"], **locale["environment"]}
    expected = dict(row["expected"])
    errno_bytes = None
    if row["launch"] is not None:
        stderr = bytes.fromhex(expected["stderr_template_hex"])
        if stderr.count(b"@ENOENT@") != 1:
            raise h.HarnessError("deleted-cwd model must declare exactly one errno slot")
        errno_bytes = errno_witness(root, environment)
        expected["stderr_template_hex"] = stderr.replace(b"@ENOENT@", errno_bytes).hex()
    model = directory.Model(
        row["id"], {name: bytes.fromhex(value) for name, value in row["files"].items()},
        tuple(row["args"]), environment, expected, bytes.fromhex(expected["seen_template_hex"]),
        modules=tuple(directory.Module(**item) for item in row["modules"]),
    )
    return model, errno_bytes


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle", type=Path, default=ROOT / ".tools/oracle/bin/make")
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--locale-root", type=Path, default=ROOT / ".tools/locale-data")
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--case", action="append", default=[])
    args = parser.parse_args(argv)
    data, rows = load_rows(args.case)
    oracle, oracle_profile, catalog, locale = nls.prepare_reference(
        args.oracle, data["reference_catalog_sha256"], args.locale_root,
    )
    directory.SOURCES = DATA
    candidate = args.candidate.resolve() if args.candidate else None
    report = {"classification": data["classification"], "models_sha256": h.sha256_file(DATA / "directory.json"),
              "driver_sha256": h.sha256_file(Path(__file__)), "locale_profile": locale,
              "oracle": h.executable_identity(oracle, oracle=True), "oracle_profile_id": oracle_profile["profile_id"],
              "candidate": h.executable_identity(candidate) if candidate else None,
              "reference_catalog_sha256": h.sha256_file(catalog), "functional_cpu_affinity": sorted(os.sched_getaffinity(0)),
              "records": [], "passed": False}
    args.report.parent.mkdir(parents=True, exist_ok=True)

    def publish():
        args.report.write_text(json.dumps(report, indent=2) + "\n")

    with tempfile.TemporaryDirectory(prefix="rmake-nls-directory-") as temporary:
        root = Path(temporary)
        runtime = root / "runtime"
        runtime.mkdir()
        report["runtime_path"] = str(runtime)
        for row in rows:
            model, errno_bytes = make_model(row, root, locale)
            case, compilation, filesystem = directory.prepare(model, root / model.id, runtime, "/usr/bin/cc")
            record = {"id": model.id, "model": row, "compilation": compilation, "expected_filesystem": filesystem,
                      "enoent_hex": None if errno_bytes is None else errno_bytes.hex(), "observations": []}
            report["records"].append(record)
            binaries = [oracle, oracle]
            if candidate:
                binaries.append(candidate)
            observed = []
            for binary in binaries:
                value = observe(case, runtime, binary, row["launch"])
                observed.append(value)
                record["observations"].append(directory.evidence(value))
                publish()
                if value.limit or value.leftover_processes or value.returncode < 0:
                    raise h.HarnessError("new NLS directory bound or signal retained; stop without replay")
            record.update(directory.assess(model, case, runtime, filesystem, observed, [{} for _ in observed]))
            print(("PASS " if record["passed"] else "FAIL ") + model.id, flush=True)
            publish()
    report["totals"] = {"cases": len(report["records"]), "pass": sum(row["passed"] for row in report["records"])}
    report["passed"] = report["totals"]["pass"] == len(rows)
    publish()
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, h.HarnessError, locale_data.ProfileError, reference.OracleError) as error:
        raise SystemExit(str(error)) from error
