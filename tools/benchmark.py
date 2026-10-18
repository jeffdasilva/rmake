#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original no-op/expansion workloads; optional paired, output-checked comparison."""

import argparse
import hashlib
import json
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import tempfile
import time

import conformance
import oracle as reference_oracle

ROOT = Path(__file__).resolve().parent.parent


def workloads():
    yield "tiny-noop", b"all: ;\n"
    names = [f"node{i}" for i in range(10000)]
    yield "large-noop", (
        "all: " + " ".join(names) + "\n" + "".join(f"{name}: ;\n" for name in names)
    ).encode()
    yield "expansion", (
        "WORD := alpha beta gamma\n"
        + "".join(f"V{i} := $(subst beta,B,$(WORD))\n" for i in range(5000))
        + "all: ;\n"
    ).encode()


def span_workloads():
    """Fixed large-list costs, including early exits and required full scans."""
    prelude = b"TEXT := " + b"one  two\t" * 100000 + b"\n"
    expressions = (
        ("wordlist-prefix", b"$(wordlist 1,1,$(TEXT))"),
        ("wordlist-middle", b"$(wordlist 100000,100002,$(TEXT))"),
        ("wordlist-full", b"$(wordlist 1,9223372036854775807,$(TEXT))"),
        ("wordlist-outside", b"$(wordlist 9223372036854775807,9223372036854775807,$(TEXT))"),
        ("let-remainder", b"$(let first rest,$(TEXT),$(lastword $(value rest)))"),
        ("patsubst-literal", b"$(patsubst one,replaced,$(TEXT))"),
    )
    for name, expression in expressions:
        yield name, (prelude + b"OUT := " + expression + b"\n"
                     b"$(info [$(words $(OUT))|$(firstword $(OUT))|$(lastword $(OUT))])\n"
                     b"all: ;\n")


def selected_workloads(suite):
    if suite in ("core", "all"):
        yield from workloads()
    if suite in ("word-spans", "all"):
        yield from span_workloads()


def paired_order(iteration, candidate):
    engines = ["oracle", "candidate"] if candidate else ["oracle"]
    return engines if iteration % 2 == 0 else list(reversed(engines))


def summary(samples):
    return {"samples_ns": samples, "median_ns": statistics.median(samples),
            "min_ns": min(samples), "max_ns": max(samples)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle", type=Path, default=ROOT / ".tools/oracle/bin/make")
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--repeat", type=int, default=15)
    parser.add_argument("--suite", choices=("core", "word-spans", "all"), default="core")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.repeat < 3 or args.repeat > 1000:
        parser.error("--repeat must be between 3 and 1000")
    oracle = args.oracle.resolve()
    profile = reference_oracle.verify(oracle)
    identities = {"oracle": conformance.executable_identity(oracle, oracle=True)}
    identities["oracle"]["profile"] = profile
    executables = {"oracle": oracle}
    if args.candidate:
        identities["candidate"] = conformance.executable_identity(args.candidate)
        executables["candidate"] = Path(identities["candidate"]["path"])
    report = {
        "schema": 2,
        "suite": args.suite,
        "identities": identities,
        "platform": platform.platform(),
        "tool": "wall time including startup; two warmups per engine; OS page cache warm",
        "method": "alternating engine order; same path, environment, arguments and exact output",
        "limitations": ["no CPU pinning or isolation", "no CPU/RSS/I/O measurements",
                        "no confidence intervals or performance acceptance thresholds",
                        "selected early workloads, not a representative full-compatibility benchmark"],
        "workloads": [],
    }
    with tempfile.TemporaryDirectory(prefix="rmake-bench-") as directory:
        path = Path(directory)
        env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "TZ": "UTC", "HOME": directory}
        alias = path / "make"
        for name, source in selected_workloads(args.suite):
            (path / "Makefile").write_bytes(source)
            samples = {label: [] for label in executables}
            expected = None
            for iteration in range(args.repeat + 2):
                for label in paired_order(iteration, args.candidate is not None):
                    alias.unlink(missing_ok=True)
                    alias.symlink_to(executables[label])
                    start = time.perf_counter_ns()
                    result = subprocess.run(
                        [str(alias), "-rR", "--no-print-directory"], cwd=path, env=env,
                        capture_output=True, timeout=30, check=True,
                    )
                    elapsed = time.perf_counter_ns() - start
                    output = (result.stdout, result.stderr)
                    if expected is not None and expected != output:
                        raise RuntimeError(f"benchmark output disagreement: {name} ({label})")
                    expected = output
                    if iteration >= 2:
                        samples[label].append(elapsed)
            record = {
                "name": name, "source_sha256": hashlib.sha256(source).hexdigest(),
                "measurements": {label: summary(values) for label, values in samples.items()},
            }
            if args.candidate:
                record["paired_oracle_over_candidate"] = [
                    left / right for left, right in zip(samples["oracle"], samples["candidate"])]
                record["median_paired_ratio"] = statistics.median(record["paired_oracle_over_candidate"])
            report["workloads"].append(record)
            display = ", ".join(f"{label} median {values['median_ns'] / 1_000_000:.3f} ms"
                                for label, values in record["measurements"].items())
            print(f"{name}: {display}")
    output_path = args.output or ROOT / f"artifacts/bench/{'comparison' if args.candidate else 'oracle'}.json"
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Report: {output_path}")


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, subprocess.SubprocessError, reference_oracle.OracleError,
            conformance.HarnessError) as error:
        print(f"benchmark: {error}", file=sys.stderr)
        sys.exit(2)
