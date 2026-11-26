#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Exact GNU2 comparison of localized directory framing and cwd failures."""
import argparse
import base64
import errno
from dataclasses import replace
import json
import os
from pathlib import Path
import re
import sys
import tempfile

import conformance as h
import directory_reporting as directory
import locale_data
import nls_diagnostics as nls
import oracle as reference

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "tests/nls"
COPIED_STDIN = "@COPIED_STDIN@"
STDIN_CREATE_RECORD = "home/stdin-create-attempt"
STDIN_CREATE_PATH = b"@STDIN_CREATE_PATH@"


def observe(case, runtime, binary, launcher):
    if launcher is None:
        return h.execute(case, runtime, binary)
    h.reset_workspace(case, runtime, binary)
    executable = str(runtime / "bin/make")
    cwd = runtime / "work"
    if launcher in ("entry", "entry-stdin"):
        # Keep the disposable entry inside the observed workspace while -C uses its sibling.
        cwd = cwd / "entry"
        cwd.mkdir(mode=0o755)
        command = [executable, *case.args]
        if launcher == "entry-stdin":
            command = ["/bin/sh", "-c", 'exec "$@" < ../build/stdin-input.mk', "stdin-launcher", *command]
    elif launcher == "stdin-create":
        command = ["/bin/sh", "-c", 'exec "$@" < stdin-input.mk', "stdin-launcher", executable, *case.args]
    else:
        command = [sys.executable, str(DATA / "deleted_cwd.py"), launcher,
                   str(cwd), executable, *case.args]
    out, err, status, limit, leftovers = h.capture(
        command, cwd, h.clean_environment(runtime, case.environment), case.timeout,
    )
    files = h.snapshot({name: runtime / name for name in ("work", "home", "tmp")})
    return h.Observation(out, err, status, limit, files, leftovers)


def errno_witness(root, environment, name):
    # Query the OS diagnostic independently, with the same locale inputs.
    # The Make template, prefix, spacing, newline and stream remain literal.
    witness_environment = {key: value for key, value in environment.items() if key != "LD_PRELOAD"}
    out, err, status, limit, leftovers = h.capture(
        [sys.executable, "-c", nls.OS_WITNESS, name], root,
        h.clean_environment(root, witness_environment), 5,
    )
    if status or err or limit or leftovers or not out or b"\n" in out:
        raise h.HarnessError("could not witness host libc filesystem diagnostic")
    return out


def load_rows(selection):
    data = json.loads((DATA / "directory.json").read_bytes())
    rows = data["rows"]
    names = {row["id"] for row in rows}
    if len(rows) != 50 or len(names) != 50:
        raise h.HarnessError("unexpected NLS directory inventory")
    if set(selection) - names:
        raise h.HarnessError("unknown exact NLS directory ID")
    fixtures = data["fixture_sha256"]
    if set(fixtures) != {"nls_control.c", "deleted_cwd.py", "native/fatal_scope.c",
                         "include_errno.c", "stdin_create_observer.c"}:
        raise h.HarnessError("unexpected NLS directory fixture inventory")
    for name, expected in fixtures.items():
        source = DATA.parent / name if name == "native/fatal_scope.c" else DATA / name
        if h.sha256_file(source) != expected:
            raise h.HarnessError("NLS directory fixture identity differs: " + name)
    module_sources = fixtures.keys() - {"deleted_cwd.py"}
    for row in rows:
        if any(module["source"] not in module_sources for module in row["modules"]):
            raise h.HarnessError("NLS directory model uses unpinned source")
        if row["launch"] not in (None, "initial", "after-C", "entry", "entry-stdin", "stdin-create"):
            raise h.HarnessError("unknown NLS directory launch")
    return data, [row for row in rows if not selection or row["id"] in selection]


def make_model(row, root, locale):
    environment = {**row["environment"], **locale["environment"]}
    expected = dict(row["expected"])
    errors = {}
    stderr = bytes.fromhex(expected["stderr_template_hex"])
    # The loaded callback witnesses the exact errno bytes in the ordered seen file.
    callback_errno = any(item["source"] == "include_errno.c" for item in row["modules"])
    cwd_failure = row["launch"] == "initial" or (row["launch"] == "after-C" and not callback_errno)
    for name in ("ENOENT", "EISDIR", "ENOTDIR"):
        slot = f"@{name}@".encode()
        count = stderr.count(slot)
        if count != 0 or (name == "ENOENT" and cwd_failure):
            if count != 1:
                raise h.HarnessError("filesystem failure must declare exactly one slot per errno")
            errors[name] = errno_witness(root, environment, name)
            stderr = stderr.replace(slot, errors[name])
    expected["stderr_template_hex"] = stderr.hex()
    model = directory.Model(
        row["id"], {name: bytes.fromhex(value) for name, value in row["files"].items()},
        tuple(row["args"]), environment, expected, bytes.fromhex(expected["seen_template_hex"]),
        modules=tuple(directory.Module(
            item["source"] if "/" in item["source"] else "nls/" + item["source"],
            item["filename"], tuple(item.get("defines", ())),
        ) for item in row["modules"]),
        updates={name: bytes.fromhex(value) for name, value in row.get("expected_file_updates", {}).items()},
    )
    return model, errors


def expected_restart_source(filesystem, path, kind):
    if path == COPIED_STDIN:
        if kind != "directory":
            raise h.HarnessError("copied stdin witness must retain its replacement directory")
        key = "tmp/" + COPIED_STDIN
    else:
        key = "work/" + str(directory.safe_relative(path))
        if filesystem.get(key, {}).get("type") != "file":
            raise h.HarnessError("restart source must name an input file")
    if kind == "directory":
        filesystem[key] = {"type": "directory", "mode": 0o755}
    elif kind == "absent":
        del filesystem[key]
    else:
        raise h.HarnessError("unknown restart source effect")


def identify_copied_stdin(value, runtime):
    # Read the single retained directory name from the filesystem, never from stderr.
    sampled = directory.evidence(value)
    sources = [key for key, item in value.filesystem.items()
               if re.fullmatch(r"tmp/Gm[A-Za-z0-9]{6}", key) and item["type"] == "directory"]
    if len(sources) != 1:
        return value, sampled
    key = sources[0]
    path = os.fsencode(runtime / key)
    sampled["source_path_hex"] = path.hex()
    filesystem = dict(value.filesystem)
    filesystem["tmp/" + COPIED_STDIN] = filesystem.pop(key)
    return replace(value, filesystem=filesystem, stderr=value.stderr.replace(path, COPIED_STDIN.encode())), sampled


def observe_stdin_create(value, row):
    sample = value.filesystem.get(STDIN_CREATE_RECORD)
    expected = row["stdin_create"]
    if expected is None:
        if sample is not None:
            raise h.HarnessError("ordinary input unexpectedly attempted named stdin storage")
        return value, None
    if sample is None or sample.get("type") != "file" or sample.get("mode") != 0o600:
        raise h.HarnessError("missing named stdin creation witness")
    prefix = os.fsencode(row["environment"]["MAKE_TMPDIR"])
    error = getattr(errno, expected) if expected else 0
    pattern = b"(" + re.escape(prefix) + rb"Gm[A-Za-z0-9]{6})\t" + str(error).encode() + b"\n"
    matched = re.fullmatch(pattern, base64.b64decode(sample["content"], validate=True))
    if matched is None:
        raise h.HarnessError("named stdin attempt path or errno differs")
    path = matched.group(1)
    filesystem = {key: entry for key, entry in value.filesystem.items() if key != STDIN_CREATE_RECORD}
    stderr = value.stderr.replace(path, STDIN_CREATE_PATH)
    return replace(value, stderr=stderr, filesystem=filesystem), {"path_hex": path.hex(), "errno": error}


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
    directory.SOURCES = DATA.parent
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
            model, errors = make_model(row, root, locale)
            case, compilation, filesystem = directory.prepare(model, root / model.id, runtime, "/usr/bin/cc")
            source_effect = row.get("source_after_restart")
            if source_effect:
                path, kind = source_effect
                expected_restart_source(filesystem, path, kind)
            record = {"id": model.id, "model": row, "compilation": compilation, "expected_filesystem": filesystem,
                      "enoent_hex": errors["ENOENT"].hex() if "ENOENT" in errors else None, "observations": []}
            for error in ("EISDIR", "ENOTDIR"):
                if error in errors:
                    record[error.lower() + "_hex"] = errors[error].hex()
            if "stdin_create" in row:
                record["stdin_create_observations"] = []
            if source_effect and path == COPIED_STDIN:
                record["copied_stdin_observations"] = []
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
                if source_effect and path == COPIED_STDIN:
                    value, sampled = identify_copied_stdin(value, runtime)
                    record["copied_stdin_observations"].append(sampled)
                    observed[-1] = value
                if "stdin_create" in row:
                    value, sampled = observe_stdin_create(value, row)
                    record["stdin_create_observations"].append(sampled)
                    observed[-1] = value
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
