#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Strict GNU2 comparison of the bounded enabled Guile frontend suites."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import tempfile

import conformance as h
import locale_data

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "tests/guile-frontend"
ENVIRONMENT = ("LD_LIBRARY_PATH", "GUILE_LOAD_PATH", "GUILE_LOAD_COMPILED_PATH", "GUILE_AUTO_COMPILE")

def read_profile(path):
    profile = json.loads(path.read_bytes())
    for identity in [profile["binary"], *profile["runtime_dependencies"]]:
        if h.sha256_file(Path(identity["path"])) != identity["sha256"]:
            raise h.HarnessError("enabled reference identity changed: " + identity["path"])
    return profile

def safe_relative(name):
    result = PurePosixPath(name)
    if not result.parts or result.is_absolute() or ".." in result.parts:
        raise h.HarnessError("invalid fixture path: " + name)
    return result

def source_file(directory, name):
    return directory.joinpath(*safe_relative(name).parts)

def prepare(specification, row, directory):
    source = DATA / specification["source"] if "source" in specification else None
    schema = specification["schema"]
    files = row["files"] if schema != "phase" else {
        "Makefile": row["makefile_hex"], "seen": b"anchor\n".hex(), "phase.c": source.read_bytes().hex(),
    }
    directory.mkdir()
    for name, encoded in files.items():
        target = source_file(directory, name)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(bytes.fromhex(encoded))
        if schema == "signal" and name == "Makefile":
            os.utime(target, (1_600_000_000, 1_600_000_000))
    compile_spec = []
    if schema == "phase":
        flags = ["-pthread", *(["-DPHASE_REDIRECT=1"] if row["redirect"] else [])]
        output = "phase_redirect.so" if row["redirect"] else "phase_probe.so"
        compile_spec.append(("phase.c", output, flags, ["-ldl"]))
    if schema == "signal" or "compile_shared" in row:
        if source is None or files.get(source.name) != source.read_bytes().hex():
            raise h.HarnessError("shared fixture source differs from its pinned plan")
        compile_spec.extend((name, target, [], []) for name, target in row["compile_shared"])
    records = []
    for name, target, flags, libraries in compile_spec:
        src = source_file(directory, name)
        dst = source_file(directory, target)
        command = ["/usr/bin/cc", "-std=c11", "-Wall", "-Wextra", "-Werror", "-fPIC", "-shared", *flags,
                   str(src), *libraries, "-o", str(dst)]
        out, err, status, limit, leftovers = h.capture(command, directory, os.environ.copy(), 10, False)
        record = {"command": command, "source_sha256": h.sha256_file(src), "status": status, "limit": limit,
                  "leftovers": leftovers, "stdout_hex": out.hex(), "stderr_hex": err.hex()}
        if status or limit or leftovers or out or err:
            return files, [*records, record], False
        record["binary_sha256"] = h.sha256_file(dst)
        records.append(record)
    return files, records, True

def file_fact(content):
    return {"content": base64.b64encode(content).decode(), "size": len(content), "sha256": hashlib.sha256(content).hexdigest()}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", type=Path, required=True)
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--suite", action="append", choices=("language", "phase", "home", "signal", "stdin", "valid-text", "callback-debug", "module", "module-callback", "live-converter", "pair-traversal", "bridge-effects", "gmk-var-shape"), default=[])
    parser.add_argument("--case", action="append", default=[])
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--locale-root", type=Path, default=ROOT / ".tools/locale-data")
    args = parser.parse_args()
    if args.report.exists():
        raise h.HarnessError("refusing to replace existing evidence")
    profile = read_profile(args.profile)
    oracle = Path(profile["binary"]["path"])
    environment = {key: profile["environment"][key] for key in ENVIRONMENT}
    environment["GNUMAKEFLAGS"] = ""
    inventory = json.loads((DATA / "suites.json").read_bytes())
    selected = []
    for spec in inventory["suites"]:
        if args.suite and spec["name"] not in args.suite:
            continue
        plan_path = DATA / spec["plan"]
        if h.sha256_file(plan_path) != spec["plan_sha256"]:
            raise h.HarnessError("model plan hash differs")
        plan = json.loads(plan_path.read_bytes())
        if len(plan["rows"]) != spec["count"]:
            raise h.HarnessError("model inventory differs")
        if "source" in spec and h.sha256_file(DATA / spec["source"]) != spec["source_sha256"]:
            raise h.HarnessError("fixture source hash differs")
        selected.extend((spec, row) for row in plan["rows"])
    unknown = set(args.case) - {row["id"] for _, row in selected}
    if unknown:
        raise h.HarnessError("unknown model IDs: " + ", ".join(sorted(unknown)))
    selected = [(spec, row) for spec, row in selected if not args.case or row["id"] in args.case]
    if not selected:
        raise h.HarnessError("empty model selection")
    args.report.parent.mkdir(parents=True, exist_ok=True)
    report = {"classification": "Bounded strict Guile frontend comparison; no complete profile or M6 claim",
              "profile_sha256": h.sha256_file(args.profile), "model_inventory_sha256": h.sha256_file(DATA / "suites.json"),
              "oracle": h.executable_identity(oracle, oracle=True), "candidate": h.executable_identity(args.candidate) if args.candidate else None,
              "functional_cpu_affinity": sorted(os.sched_getaffinity(0)), "records": [], "passed": False}
    locales = {row["locale"] for _, row in selected if "locale" in row}
    locale_profile = None
    if locales:
        if locales != {"fr_FR.UTF-8"}:
            raise h.HarnessError("unexpected Guile fixture locale")
        locale_profile = locale_data.provision(args.locale_root.resolve())
        if locale_profile["locale"] not in locales:
            raise h.HarnessError("locale profile differs from the fixture")
        report["locale_profile"] = locale_profile
    def publish():
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    with tempfile.TemporaryDirectory(prefix="rmake-guile-frontend-") as temporary:
        root = Path(temporary)
        runtime = root / "reset"
        runtime.mkdir()
        for spec, row in selected:
            directory = source_file(root, row["id"])
            files, compilation, compiled = prepare(spec, row, directory)
            record = {"id": row["id"], "suite": spec["name"], "input_files_hex": files, "compilation": compilation,
                      "model": row, "observations": [], "passed": False}
            report["records"].append(record)
            if not compiled:
                publish()
                raise h.HarnessError("original fixture compile failed before GNU")
            expected = row["expected"]
            case_environment = dict(environment)
            case_environment["GNUMAKEFLAGS"] = row.get("gnumakeflags", "")
            case_environment.update(row.get("environment", {}))
            if "locale" in row:
                case_environment.update(locale_profile["environment"])
                case_environment.update({"LC_ALL": row["locale"], "LANG": row["locale"]})
            case = h.Case(directory, row["id"], "guile.frontend", "required", tuple(row["args"]), 5, case_environment,
                          {"exit_code": expected["status"], "stdout_hex": expected["stdout"].encode().hex(),
                           "stderr_hex": expected["stderr"].encode().hex()}, "separate")
            h.reset_workspace(case, runtime, oracle)
            filesystem = h.snapshot({name: runtime / name for name in ("work", "home", "tmp")})
            filesystem["work/seen"].update(file_fact(expected["seen"].encode()))
            for name, content in row.get("expected_additions_hex", {}).items():
                filesystem["work/" + str(safe_relative(name))] = {"type": "file", "mode": 0o644, **file_fact(bytes.fromhex(content))}
            record["expected_filesystem"] = filesystem
            actual = []
            for executable in [oracle, oracle, *([args.candidate] if args.candidate else [])]:
                if spec["schema"] == "stdin":
                    h.reset_workspace(case, runtime, executable)
                    # Redirect one original named fixture into the same real
                    # argv[0] for each engine. The shell is replaced by make.
                    command = ["/bin/sh", "-c", 'exec "$@" < input.mk', "guile-stdin",
                               str(runtime / "bin/make"), *case.args]
                    out, err, status, limit, leftovers = h.capture(
                        command, runtime / "work", h.clean_environment(runtime, case.environment), case.timeout)
                    snapshot = h.snapshot({name: runtime / name for name in ("work", "home", "tmp")})
                    observed = h.Observation(out, err, status, limit, snapshot, leftovers)
                else:
                    observed = h.execute(case, runtime, executable)
                errors = h.expectation_errors(case, observed)
                difference = h.differences(actual[0], observed) if actual else []
                actual.append(observed)
                record["observations"].append(dict(h.observation_json(observed), stdout_hex=observed.stdout.hex(), stderr_hex=observed.stderr.hex(),
                                                   errors=errors, filesystem_matches=observed.filesystem == filesystem, differences=difference))
                if observed.limit or observed.leftover_processes or observed.returncode < 0:
                    record["zero_credit"] = True
                    publish()
                    raise h.HarnessError("new bound/signal retained; stop without replay")
            record["passed"] = all(not item["errors"] and item["filesystem_matches"] and not item["differences"] for item in record["observations"])
            print(row["id"], "PASS" if record["passed"] else "FAIL", flush=True)
            publish()
    report["passed"] = all(record["passed"] for record in report["records"])
    report["totals"] = {"cases": len(report["records"]), "pass": sum(record["passed"] for record in report["records"])}
    publish()
    return 0 if report["passed"] else 1

if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (h.HarnessError, locale_data.ProfileError) as error:
        raise SystemExit(str(error)) from error
