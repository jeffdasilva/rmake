#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Strict GNU2 comparison of the bounded owned-catalog NLS profiles."""
import argparse
import base64
import hashlib
import json
import os
from dataclasses import replace
from pathlib import Path
import sys
import tempfile

import catalogs
import conformance as h
import locale_data
import native_modules as native
import oracle as reference

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "tests/nls"
LOADER_WITNESS = """
import ctypes
import locale
import os
import sys
locale.setlocale(locale.LC_ALL, "")
library = ctypes.CDLL(None)
library.dlopen.argtypes = [ctypes.c_char_p, ctypes.c_int]
library.dlopen.restype = ctypes.c_void_p
library.dlsym.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
library.dlsym.restype = ctypes.c_void_p
library.dlerror.argtypes = []
library.dlerror.restype = ctypes.c_char_p
flags = os.RTLD_LAZY | os.RTLD_GLOBAL
kind = sys.argv[1]
if kind == "open":
    assert not library.dlopen(b"absent_nls.so", flags)
else:
    assert kind == "symbol"
    handle = library.dlopen(b"./loader_probe.so", flags)
    assert handle
    assert not library.dlsym(handle, b"nls_missing_setup")
message = library.dlerror()
assert message
os.write(1, message)
"""


OS_WITNESS = """
import ctypes
import errno
import locale
import os
import sys
locale.setlocale(locale.LC_ALL, "")
function = ctypes.CDLL(None).strerror
function.argtypes = [ctypes.c_int]
function.restype = ctypes.c_char_p
os.write(1, function(getattr(errno, sys.argv[1])))
"""


def errno_witness(case, runtime, fact):
    name, locale = fact
    assert name in {"ENOENT", "ENOTDIR", "EISDIR"}
    assert locale in {"fr_FR.UTF-8", "C.UTF-8"}
    environment = h.clean_environment(runtime, {**case.environment, "LC_ALL": locale, "LANG": locale})
    out, err, status, limit, leftovers = h.capture(
        [sys.executable, "-c", OS_WITNESS, name], runtime / "work", environment, 5,
    )
    if status or err or limit or leftovers or not out or b"\n" in out:
        raise h.HarnessError("could not witness public libc errno text")
    expected = dict(case.expected)
    stderr = bytes.fromhex(expected["stderr_hex"])
    if stderr.count(b"@OS_DETAIL@") != 1:
        raise h.HarnessError("expected one exact independent errno slot")
    expected["stderr_hex"] = stderr.replace(b"@OS_DETAIL@", out).hex()
    return replace(case, expected=expected), out


def cases(selection):
    data = json.loads((DATA / "models.json").read_bytes())
    structure = json.loads((DATA / "structure.json").read_bytes())
    producers = json.loads((DATA / "producers.json").read_bytes())
    loader = json.loads((DATA / "loader.json").read_bytes())
    include = json.loads((DATA / "include.json").read_bytes())
    remake = json.loads((DATA / "remake.json").read_bytes())
    repeat = json.loads((DATA / "repeat.json").read_bytes())
    replay = json.loads((DATA / "optional-replay.json").read_bytes())
    suites = (data, structure, producers, loader, include, remake, repeat, replay)
    if tuple(len(suite["rows"]) for suite in suites) != (30, 16, 18, 15, 20, 14, 13, 18):
        raise h.HarnessError("unexpected NLS model inventory")
    rows = [row for suite in suites for row in suite["rows"]]
    if len({row["id"] for row in rows}) != len(rows):
        raise h.HarnessError("duplicate NLS model ID")
    if any(suite["reference_catalog_sha256"] != data["reference_catalog_sha256"]
           for suite in suites[1:]):
        raise h.HarnessError("NLS catalog identity differs across suites")
    unknown = set(selection) - {row["id"] for row in rows}
    if unknown:
        raise h.HarnessError("unknown exact NLS IDs: " + ", ".join(sorted(unknown)))
    fixtures = {**data["fixture_sha256"]}
    new_fixtures = producers["fixture_sha256"]
    if set(fixtures) != {"nls_probe.c", "nls_control.c"} or set(new_fixtures) != {"nls_control.c", "producer_native.c"}:
        raise h.HarnessError("unexpected owned NLS source inventory")
    if new_fixtures["nls_control.c"] != fixtures["nls_control.c"]:
        raise h.HarnessError("shared NLS fixture identity differs")
    fixtures["producer_native.c"] = new_fixtures["producer_native.c"]
    loader_fixtures = loader["fixture_sha256"]
    if set(loader_fixtures) != {"nls_control.c", "loader_probe.c"} or loader_fixtures["nls_control.c"] != fixtures["nls_control.c"]:
        raise h.HarnessError("loader fixture inventory or shared source identity differs")
    fixtures["loader_probe.c"] = loader_fixtures["loader_probe.c"]
    include_fixtures = include["fixture_sha256"]
    if set(include_fixtures) != {"nls_control.c", "include_errno.c"} or include_fixtures["nls_control.c"] != fixtures["nls_control.c"]:
        raise h.HarnessError("include fixture inventory or shared source identity differs")
    fixtures["include_errno.c"] = include_fixtures["include_errno.c"]
    if remake["fixture_sha256"] != {"nls_control.c": fixtures["nls_control.c"]}:
        raise h.HarnessError("remake fixture inventory or shared source identity differs")
    if repeat["fixture_sha256"] or replay["fixture_sha256"]:
        raise h.HarnessError("repeat and replay cases should not compile any additional fixtures")
    for name, expected in fixtures.items():
        if h.sha256_file(DATA / name) != expected:
            raise h.HarnessError("NLS fixture source hash differs: " + name)
    if any(module["source"] not in fixtures for row in rows for module in row["modules"]):
        raise h.HarnessError("NLS model refers to unpinned source")
    return data, [row for row in rows if not selection or row["id"] in selection]


def prepare(row, environment):
    expected = row["expected"]
    modules = tuple(native.Module(module["source"], module["filename"], tuple(module["defines"])) for module in row["modules"])
    def expected_bytes(name):
        literal, hex_bytes = expected.get(name), expected.get(name + "_hex")
        if (literal is None) == (hex_bytes is None):
            raise h.HarnessError("expected exactly one NLS byte representation: " + name)
        return literal.encode() if literal is not None else bytes.fromhex(hex_bytes)
    return native.Model(row["id"], {name: bytes.fromhex(value) for name, value in row["files"].items()},
                        {"exit_code": expected["exit_code"], "stdout_hex": expected_bytes("stdout").hex(),
                         "stderr_hex": expected_bytes("stderr").hex()}, expected_bytes("seen"), modules,
                        tuple(row["args"]), {**row["environment"], **environment},
                        symlinks=row.get("symlinks", {}), mtimes_ns=row.get("mtimes_ns", {}))


def loader_witness(case, runtime, kind):
    if kind not in ("open", "symbol"):
        raise h.HarnessError("unknown raw loader witness")
    out, err, status, limit, leftovers = h.capture(
        [sys.executable, "-c", LOADER_WITNESS, kind], runtime / "work",
        h.clean_environment(runtime, case.environment), 5,
    )
    if status or err or limit or leftovers or not out or b"\n" in out:
        raise h.HarnessError("could not witness the raw loader error")
    expected = dict(case.expected)
    text = bytes.fromhex(expected["stderr_hex"])
    if text.count(b"@LOADER_DETAIL@") != 1:
        raise h.HarnessError("loader model must declare exactly one detail argument")
    expected["stderr_hex"] = text.replace(b"@LOADER_DETAIL@", out).hex()
    return replace(case, expected=expected), out


def prepare_reference(oracle, catalog_sha256, locale_root):
    catalogs.verify(ROOT / "catalogs")
    oracle = oracle.resolve()
    oracle_profile = reference.verify(oracle)
    prefixes = [arg.removeprefix("--prefix=") for arg in oracle_profile.get("build", {}).get("configure_args", []) if arg.startswith("--prefix=")]
    if len(prefixes) != 1 or not Path(prefixes[0]).is_absolute():
        raise h.HarnessError("reference profile must identify its configured catalog prefix")
    catalog = Path(prefixes[0]) / "share/locale/fr/LC_MESSAGES/make.mo"
    if h.sha256_file(catalog) != catalog_sha256:
        raise h.HarnessError("reference French message catalog identity differs")
    profile = locale_data.provision(locale_root.resolve())
    return oracle, oracle_profile, catalog, profile


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle", type=Path, default=ROOT / ".tools/oracle/bin/make")
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--locale-root", type=Path, default=ROOT / ".tools/locale-data")
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--case", action="append", default=[])
    args = parser.parse_args(argv)
    data, rows = cases(args.case)
    oracle, oracle_profile, catalog, profile = prepare_reference(
        args.oracle, data["reference_catalog_sha256"], args.locale_root,
    )
    native.SOURCES = DATA
    report = {"classification": data["classification"], "models_sha256": h.sha256_file(DATA / "models.json"),
              "structure_sha256": h.sha256_file(DATA / "structure.json"),
              "producer_sha256": h.sha256_file(DATA / "producers.json"),
              "loader_sha256": h.sha256_file(DATA / "loader.json"),
              "include_sha256": h.sha256_file(DATA / "include.json"),
              "remake_sha256": h.sha256_file(DATA / "remake.json"),
              "repeat_sha256": h.sha256_file(DATA / "repeat.json"),
              "optional_replay_sha256": h.sha256_file(DATA / "optional-replay.json"),
              "tool_sha256": h.sha256_file(Path(__file__)), "locale_profile": profile,
              "oracle": h.executable_identity(oracle, oracle=True), "oracle_profile_id": oracle_profile["profile_id"], "candidate": h.executable_identity(args.candidate) if args.candidate else None,
              "reference_catalog": {"path": str(catalog), "sha256": h.sha256_file(catalog)},
              "owned_catalog_manifest_sha256": h.sha256_file(ROOT / "catalogs/provenance.json"),
              "functional_cpu_affinity": sorted(os.sched_getaffinity(0)), "cases": [], "passed": False}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    def publish():
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    with tempfile.TemporaryDirectory(prefix="rmake-nls-") as temporary:
        root = Path(temporary)
        reset = root / "runtime"
        reset.mkdir()
        for row in rows:
            model = prepare(row, profile["environment"])
            record = {"id": model.id, "model": row, "observations": [], "passed": False}
            report["cases"].append(record)
            try:
                case, compilation = native.prepare(model, root / row["id"], "/usr/bin/cc")
            except h.HarnessError as error:
                record["preparation_error"] = str(error)
                publish()
                raise
            h.reset_workspace(case, reset, oracle)
            if (kind := row.get("loader_fact")) is not None:
                case, detail = loader_witness(case, reset, kind)
                record["raw_loader_detail_hex"] = detail.hex()
            if (fact := row.get("errno_fact")) is not None:
                case, detail = errno_witness(case, reset, fact)
                record["raw_errno_detail_hex"] = detail.hex()
            expected_files = h.snapshot({name: reset / name for name in ("work", "home", "tmp")})
            expected_files["work/seen"].update(content=base64.b64encode(model.seen).decode(), size=len(model.seen),
                                               sha256=hashlib.sha256(model.seen).hexdigest())
            for name, contents in row.get("updates", {}).items():
                path = "work/" + str(native.safe_relative(name))
                value = bytes.fromhex(contents)
                expected_files[path] = dict(
                    expected_files.get(path, {"type": "file", "mode": 0o644}),
                    content=base64.b64encode(value).decode(), size=len(value),
                    sha256=hashlib.sha256(value).hexdigest(),
                )
            record.update(compilation=compilation, expected_filesystem=expected_files, environment=case.environment)
            expected_mtimes = row.get("expected_mtimes_ns", {})
            timestamp_paths = {name: reset / "work" / native.safe_relative(name) for name in expected_mtimes}
            observed = []
            for executable in [oracle, oracle, *([args.candidate] if args.candidate else [])]:
                value = h.execute(case, reset, executable)
                differences = h.differences(observed[0], value) if observed else []
                observed.append(value)
                timestamps = {name: path.stat().st_mtime_ns if path.exists() else None
                              for name, path in timestamp_paths.items()}
                record["observations"].append(dict(h.observation_json(value), stdout_hex=value.stdout.hex(), stderr_hex=value.stderr.hex(),
                                                   observed_mtimes_ns=timestamps, timestamps_match=timestamps == expected_mtimes,
                                                   errors=h.expectation_errors(case, value), filesystem_matches=value.filesystem == expected_files,
                                                   differences=differences))
                if value.limit or value.leftover_processes or value.returncode < 0:
                    record["zero_credit"] = True
                    publish()
                    raise h.HarnessError("new NLS bound/signal retained; stop without replay")
            record["passed"] = all(not item["errors"] and item["filesystem_matches"] and item["timestamps_match"] and not item["differences"] for item in record["observations"])
            print(("PASS " if record["passed"] else "FAIL ") + row["id"], flush=True)
            publish()
    report["passed"] = all(record["passed"] for record in report["cases"])
    report["totals"] = {"cases": len(report["cases"]), "pass": sum(record["passed"] for record in report["cases"])}
    publish()
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, h.HarnessError, locale_data.ProfileError, reference.OracleError) as error:
        raise SystemExit(str(error)) from error
