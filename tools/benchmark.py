#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Reproducible original oracle workloads; wall-clock baselines, not speedup claims."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import tempfile
import time

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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle", type=Path, default=ROOT / ".tools/oracle/bin/make")
    parser.add_argument("--repeat", type=int, default=15)
    parser.add_argument("--output", type=Path, default=ROOT / "artifacts/bench/oracle.json")
    args = parser.parse_args()
    if args.repeat < 3 or args.repeat > 1000:
        parser.error("--repeat must be between 3 and 1000")
    oracle = args.oracle.resolve()
    subprocess.run(
        [sys.executable, str(ROOT / "tools/oracle.py"), "verify", "--binary", str(oracle)],
        check=True,
    )
    report = {
        "schema": 1,
        "oracle_sha256": hashlib.sha256(oracle.read_bytes()).hexdigest(),
        "platform": platform.platform(),
        "tool": "original wall-clock baseline; includes startup; OS page cache warm",
        "workloads": [],
    }
    with tempfile.TemporaryDirectory(prefix="rmake-bench-") as directory:
        path = Path(directory)
        env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "TZ": "UTC", "HOME": directory}
        for name, source in workloads():
            (path / "Makefile").write_bytes(source)
            samples = []
            expected = None
            for iteration in range(args.repeat + 2):
                start = time.perf_counter_ns()
                result = subprocess.run(
                    [str(oracle), "-rR", "--no-print-directory"], cwd=path, env=env,
                    capture_output=True, timeout=30, check=True,
                )
                elapsed = time.perf_counter_ns() - start
                output = (result.stdout, result.stderr)
                if expected is not None and expected != output:
                    raise RuntimeError(f"non-deterministic benchmark output: {name}")
                expected = output
                if iteration >= 2:
                    samples.append(elapsed)
            record = {
                "name": name, "source_sha256": hashlib.sha256(source).hexdigest(),
                "samples_ns": samples, "median_ns": statistics.median(samples),
                "min_ns": min(samples), "max_ns": max(samples),
            }
            report["workloads"].append(record)
            print(f"{name}: median {record['median_ns'] / 1_000_000:.3f} ms")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Report: {args.output}")


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"benchmark: {error}", file=sys.stderr)
        sys.exit(2)
