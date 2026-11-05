#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Build and qualify the optional Guile application without mixing Cargo profiles."""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

from guile_frontend import ENVIRONMENT, read_profile

ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile', type=Path, required=True)
    parser.add_argument('--default-debug', type=Path, required=True)
    parser.add_argument('--default-release', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--locale-root', type=Path, default=ROOT / '.tools/locale-data')
    args = parser.parse_args()
    profile = read_profile(args.profile)
    # This ordinary Make recipe starts isolated campaigns, not recursive jobs.
    # It does not inherit a jobserver whose descriptors were closed by Make.
    base = dict(os.environ)
    for key in (
        'GNUMAKEFLAGS', 'MAKEFLAGS', 'MFLAGS', 'MAKELEVEL', 'MAKE_RESTARTS', 'CARGO_MAKEFLAGS',
        # Custom test binaries use these variables only in their own children.
        'RMAKE_GUILE_COMPONENT_CASE', 'RMAKE_NATIVE_ENTRY_CASE', 'RMAKE_STARTUP_OWNER_UNIT_CHILD',
    ):
        base.pop(key, None)
    enabled = {**base, **{key: profile['environment'][key] for key in (*ENVIRONMENT, 'RMAKE_GUILE_PREFIX')}}
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    directory = Path(tempfile.mkdtemp(prefix='run-', dir=output))
    target = output / 'cargo-target'
    build_env = {**enabled, 'CARGO_TARGET_DIR': str(target)}
    python = sys.executable

    def run(argv: list[str], environment: dict[str, str] = base) -> None:
        subprocess.run(argv, cwd=ROOT, env=environment, check=True)

    cargo = [python, 'tools/rust.py', 'cargo']
    common = ['--workspace', '--features', 'guile', '--locked']
    run([*cargo, 'test', '--all-targets', *common], build_env)
    run([*cargo, 'clippy', '--all-targets', *common, '--', '-D', 'warnings'], build_env)
    run([*cargo, 'build', *common], build_env)
    run([*cargo, 'build', '--release', *common], build_env)
    compare = [python, 'tools/guile_frontend.py', '--profile', str(args.profile.resolve()),
               '--locale-root', str(args.locale_root.resolve())]
    detached = [python, 'tools/guile_detached.py', '--profile', str(args.profile.resolve())]
    pressure = [python, 'tools/guile_eventfd_pressure.py', '--profile', str(args.profile.resolve())]
    parallel = [python, 'tools/guile_detached_parallel.py', '--profile', str(args.profile.resolve())]
    parallel_pressure = [python, 'tools/guile_parallel_pressure.py', '--profile', str(args.profile.resolve())]
    for label, default in (('debug', args.default_debug), ('release', args.default_release)):
        default_frozen = directory / f'default-{label}'
        enabled_frozen = directory / f'enabled-{label}'
        shutil.copy2(default, default_frozen)
        shutil.copy2(target / label / 'rmake', enabled_frozen)
        run([*compare, '--candidate', str(default_frozen), '--suite', 'signal',
             '--report', str(directory / f'default-{label}-signal.json')])
        run([*compare, '--candidate', str(enabled_frozen), '--report', str(directory / f'enabled-{label}.json')])
        run([*detached, '--candidate', str(enabled_frozen),
             '--report', str(directory / f'enabled-{label}-detached.json')])
        run([*pressure, '--candidate', str(enabled_frozen),
             '--report', str(directory / f'enabled-{label}-eventfd-pressure.json')])
        run([*parallel, '--candidate', str(enabled_frozen),
             '--report', str(directory / f'enabled-{label}-detached-parallel.json')])
        run([*parallel_pressure, '--candidate', str(enabled_frozen),
             '--report', str(directory / f'enabled-{label}-parallel-pressure.json')])
        run([str(enabled_frozen), '-rR', '--no-print-directory', 'build', 'check-core'], enabled)
    print(f'Guile application reports: {directory}', flush=True)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
