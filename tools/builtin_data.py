#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Verify or regenerate declarative defaults from the pinned oracle's output.

No upstream source is read. No reference process is needed by the product.
The clean environment excludes user makefiles, flags and toolchain overrides.
"""
from __future__ import annotations

import argparse
from pathlib import Path
import subprocess
import tempfile

import oracle

ROOT = Path(__file__).resolve().parents[1]
HEADER = ("# SPDX-License-Identifier: GPL-3.0-or-later\n"
          "# Copyright (C) 1988-2023 Free Software Foundation, Inc.\n"
          "# Observed and organized for rmake in 2026 by Jeff DaSilva.\n"
          "# Linux GNU Make 4.4.1 compatibility data observed with the pinned oracle.\n"
          "# Recipe and variable bytes are declarative compatibility data, not C source.\n")


def extract(text: str) -> dict[str, str]:
    if len(text) > 512 * 1024 or "\0" in text:
        raise ValueError("invalid oracle database size or NUL")
    lines = text.splitlines()
    variables = []
    excluded = {"MAKE", "MAKE_COMMAND", "MAKE_VERSION", "SHELL", "SUFFIXES",
                "MAKEFILES", "MAKE_HOST"}
    for index, line in enumerate(lines[:-1]):
        value = lines[index + 1]
        if line == "# default" and "=" in value:
            name = value.split("=", 1)[0].strip().removesuffix(":").strip()
            if not name.startswith(".") and name not in excluded:
                variables.append(value)
    implicit, files = text.split("# Implicit Rules\n", 1)[1].split("# Files\n", 1)
    suffixes = []
    target = ""
    recipes: list[str] = []
    for line in files.splitlines() + ["sentinel:"]:
        if line and not line.startswith(("#", "\t")) and ":" in line:
            if target.startswith(".") and target != ".SUFFIXES" and recipes:
                suffixes.append((target, recipes))
            target, recipes = line.split(":", 1)[0], []
        elif line.startswith("\t"):
            recipes.append(line)
    patterns = implicit[implicit.index("(%):"):]
    patterns = "\n".join(line for line in patterns.splitlines() if not line.startswith("#"))
    return {
        "default-variables.mk": HEADER + "\n".join(sorted(variables)) + "\n",
        "default-suffix-rules.mk": HEADER + "\n".join(
            name + ":\n" + "\n".join(recipe) + "\n" for name, recipe in sorted(suffixes)),
        "default-pattern-rules.mk": HEADER + patterns,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle", type=Path, default=oracle.PREFIX / "bin/make")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--write", action="store_true")
    args = parser.parse_args()
    oracle.verify(args.oracle.resolve())
    with tempfile.TemporaryDirectory(prefix="rmake-builtins-") as temporary:
        result = subprocess.run([str(args.oracle.resolve()), "-p", "-f", "/dev/null"],
                                cwd=temporary, env=oracle.clean_environment(),
                                capture_output=True, timeout=15, check=False)
    if result.returncode != 2 or result.stderr != b"make: *** No targets.  Stop.\n":
        raise ValueError("unexpected oracle database invocation result")
    data = extract(result.stdout.decode("utf-8"))
    for name, expected in data.items():
        path = ROOT / "crates/rmake-core/data" / name
        if args.write:
            path.write_bytes(expected.encode())
        elif path.read_bytes() != expected.encode():
            raise ValueError(f"builtin data differs from pinned oracle: {path}")
    print(f"Builtin declarative data: {len(data)} files {'written' if args.write else 'verified'}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, OSError, subprocess.TimeoutExpired, oracle.OracleError) as error:
        raise SystemExit(f"builtin data: {error}") from error
