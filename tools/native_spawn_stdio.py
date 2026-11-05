#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare ordinary child launch stream ownership and output order."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

import conformance as h
import oracle
from process_restarts import file_entry

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/native-spawn-stdio'
HEADER = "load ./spawn_probe.so\n.PHONY: all first\n"
RECIPE = "all:\n\t@$(stage_spawn @MODE@)./child @ACTION@\n"
RECIPE_RESTORE = """all: first
	@$(file >>seen,restored-$(restore_spawn )):
first:
	@$(stage_spawn @MODE@)./child @ACTION@
"""
EXPORT = """export MARK = $(stage_spawn public)visible
all:
	@./child export
"""
SHELL = """ARM := $(stage_spawn @MODE@)
CAPTURE := $(shell ./child @ACTION@)
STATUS := $(.SHELLSTATUS)
@RESTORE@$(file >>seen,capture-$(STATUS)-$(CAPTURE))
$(info capture=[$(CAPTURE)] status=[$(STATUS)])
all:;@:
"""
ASSIGN = SHELL.replace("CAPTURE := $(shell ./child @ACTION@)", "CAPTURE != ./child @ACTION@")
RESTORE = "$(file >>seen,restored-$(restore_spawn ))\n"
PRIVATE = b"private bytes"
# ID, Make template, native stream mode, child action, stdout, stderr, ordered file witnesses.
MODELS = (
    ("recipe-private", RECIPE, "private", "private", b"size=0\n", b"", b"child-size=0\n"),
    ("recipe-private-full", RECIPE, "private-full", "effect", b"", b"", b"child-effect\n"),
    ("recipe-public", RECIPE, "public", "public", b"old-out:child-out\n", b"old-err:child-err\n", b"child-public\n"),
    ("recipe-stderr-full", RECIPE_RESTORE, "stderr-full", "effect", b"", b"", b"child-effect\nrestored-error\n"),
    ("recipe-export-public", EXPORT, "public", "export", b"child-out\nold-out:", b"child-err\nold-err:", b"child-export\n"),
    ("shell-private", SHELL, "private", "private", b"capture=[size=0] status=[0]\n", b"", b"child-size=0\ncapture-0-size=0\n"),
    ("shell-public", SHELL, "public", "public", b"old-out:capture=[child-out] status=[0]\n", b"child-err\nold-err:", b"child-public\ncapture-0-child-out\n"),
    ("shell-stderr-full", SHELL.replace("@RESTORE@", RESTORE), "stderr-full", "effect", b"capture=[] status=[0]\n", b"", b"child-effect\nrestored-clean\ncapture-0-\n"),
    ("assignment-private-full", ASSIGN, "private-full", "effect", b"capture=[] status=[0]\n", b"", b"child-effect\ncapture-0-\n"),
)
MODELS += (
    ("recipe-public-merged", RECIPE, "public", "public", b"old-out:old-err:child-err\nchild-out\n", b"", b"child-public\n"),
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--cc', default='/usr/bin/cc')
    args = parser.parse_args()
    reference = args.oracle.resolve(strict=True)
    candidate = args.candidate.resolve(strict=True)
    profile = oracle.verify(reference)
    binaries = [('gnu-first', reference), ('gnu-second', reference), ('candidate', candidate)]
    report = {
        'oracle_profile': profile['profile_id'],
        'functional_cpu_affinity': sorted(os.sched_getaffinity(0)),
        'binaries': {name: h.executable_identity(binary, oracle=index < 2)
                     for index, (name, binary) in enumerate(binaries)},
        'source_hashes': {path.name: h.sha256_file(path) for path in
                          (Path(__file__), FIXTURE / 'spawn_probe.c', FIXTURE / 'child.c')},
        'cases': [],
    }
    with tempfile.TemporaryDirectory(prefix='rmake-native-spawn-stdio-') as directory:
        root = Path(directory)
        runtime = root / 'runtime'
        runtime.mkdir()
        for source, target, flags in (('spawn_probe.c', 'spawn_probe.so', ['-fPIC', '-shared']),
                                      ('child.c', 'child', [])):
            subprocess.run([args.cc, '-std=c11', '-Wall', '-Wextra', '-Werror', *flags,
                            str(FIXTURE / source), '-o', str(root / target)], timeout=30, check=True)
        report['compiled_hashes'] = {name: h.sha256_file(root / name) for name in ('spawn_probe.so', 'child')}
        roots = {name: runtime / name for name in ('work', 'home', 'tmp')}
        for case_id, recipe, mode, action, stdout, stderr, seen in MODELS:
            fixture = root / case_id
            fixture.mkdir()
            makefile = HEADER + recipe.replace('@MODE@', mode).replace('@ACTION@', action).replace('@RESTORE@', '')
            (fixture / 'Makefile').write_text(makefile)
            (fixture / 'seen').write_bytes(b'anchor\n')
            for name, mode_bits in (('child', 0o755), ('spawn_probe.so', 0o644)):
                path = fixture / name
                shutil.copyfile(root / name, path)
                path.chmod(mode_bits)
            h.validate_fixture(fixture)
            expected = {'exit_code': 0, 'stdout_hex': stdout.hex(), 'stderr_hex': stderr.hex()}
            output = 'merged' if case_id == 'recipe-public-merged' else 'separate'
            case = h.Case(fixture, case_id, 'native.spawn.stdio', 'required',
                          ('-rR', '--no-print-directory'), 5, {}, expected, output)
            h.reset_workspace(case, runtime, reference)
            filesystem = h.snapshot(roots)
            filesystem['work/seen'] = file_entry(b'anchor\narmed-' + mode.encode() + b'\n' + seen)
            if mode == 'private':
                filesystem['work/private-stream'] = file_entry(PRIVATE)
            row = {'id': case_id, 'output': output, 'args': case.args, 'expected': expected,
                   'filesystem': filesystem, 'observations': []}
            report['cases'].append(row)
            bounded = False
            for name, binary in binaries:
                observed = h.execute(case, runtime, binary)
                errors = h.expectation_errors(case, observed)
                if observed.filesystem != filesystem:
                    errors.append('independent complete filesystem')
                bounded = bool(observed.limit or observed.leftover_processes or observed.returncode < 0)
                row['observations'].append({
                    **h.observation_json(observed), 'engine': name, 'errors': errors,
                    'stdout_hex': observed.stdout.hex(), 'stderr_hex': observed.stderr.hex(),
                })
                if bounded:
                    break
            row['passed'] = not bounded and all(not run['errors'] for run in row['observations'])
            print('PASS' if row['passed'] else 'FAIL', case_id,
                  [run['errors'] for run in row['observations']], flush=True)
            if bounded:
                break
    report['passed'] = len(report['cases']) == len(MODELS) and all(row['passed'] for row in report['cases'])
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    return int(not report['passed'])


if __name__ == '__main__':
    raise SystemExit(main())
