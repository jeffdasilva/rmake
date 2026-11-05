#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Strict GNU2 comparison of the bounded owned-catalog NLS profiles."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import tempfile

import catalogs
import conformance as h
import locale_data
import native_modules as native
import oracle as reference

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "tests/nls"


def cases(selection):
    data = json.loads((DATA / "models.json").read_bytes())
    structure = json.loads((DATA / "structure.json").read_bytes())
    producers = json.loads((DATA / "producers.json").read_bytes())
    if (len(data["rows"]), len(structure["rows"]), len(producers["rows"])) != (30, 16, 18):
        raise h.HarnessError("unexpected NLS model inventory")
    rows = data["rows"] + structure["rows"] + producers["rows"]
    if len({row["id"] for row in rows}) != len(rows):
        raise h.HarnessError("duplicate NLS model ID")
    if any(suite["reference_catalog_sha256"] != data["reference_catalog_sha256"]
           for suite in (structure, producers)):
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
                        tuple(row["args"]), {**row["environment"], **environment})


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
            expected_files = h.snapshot({name: reset / name for name in ("work", "home", "tmp")})
            expected_files["work/seen"].update(content=base64.b64encode(model.seen).decode(), size=len(model.seen),
                                               sha256=hashlib.sha256(model.seen).hexdigest())
            record.update(compilation=compilation, expected_filesystem=expected_files, environment=case.environment)
            observed = []
            for executable in [oracle, oracle, *([args.candidate] if args.candidate else [])]:
                value = h.execute(case, reset, executable)
                differences = h.differences(observed[0], value) if observed else []
                observed.append(value)
                record["observations"].append(dict(h.observation_json(value), stdout_hex=value.stdout.hex(), stderr_hex=value.stderr.hex(),
                                                   errors=h.expectation_errors(case, value), filesystem_matches=value.filesystem == expected_files,
                                                   differences=differences))
                if value.limit or value.leftover_processes or value.returncode < 0:
                    record["zero_credit"] = True
                    publish()
                    raise h.HarnessError("new NLS bound/signal retained; stop without replay")
            record["passed"] = all(not item["errors"] and item["filesystem_matches"] and not item["differences"] for item in record["observations"])
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
