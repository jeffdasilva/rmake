#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original mixed-engine jobserver campaign: trusted fixtures, not a sandbox."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import tempfile

import conformance as harness
import oracle

ROOT = Path(__file__).resolve().parents[1]
NODES = ('outer1', 'outer2', 'inner1', 'inner2', 'inner3', 'inner4')


def anchors(work: Path, observation: harness.Observation, fail: bool = False) -> list[str]:
    errors = []
    if fail:
        if observation.returncode != 2 or observation.stdout or b'Error 7' not in observation.stderr:
            errors.append('failure status or output')
    elif (observation.returncode, observation.stdout, observation.stderr) != (0, b'complete\n', b''):
        errors.append('status or exact output')
    if observation.limit or observation.leftover_processes:
        errors.append('process containment')
    for name, value in [('active', b'0'), ('maximum', b'3'),
                        *[(node, b'complete\n') for node in NODES],
                        *([] if fail else [('recursive.done', b'')])]:
        if not (work / name).is_file() or (work / name).read_bytes() != value:
            errors.append('file anchor: ' + name)
    return errors


def execute(root: Path, parent: Path, child: Path, style: str, borrowed: bool, token_bytes: bytes = b"++", fail: bool = False):
    for name in ('work', 'home', 'tmp', 'bin'):
        path = root / name
        if path.exists():
            shutil.rmtree(path)
        path.mkdir()
    shutil.copytree(ROOT / 'tests/jobserver', root / 'work', dirs_exist_ok=True)
    for name in ('active', 'maximum'):
        (root / 'work' / name).write_text('0')
    (root / 'bin/make').symlink_to(parent)
    (root / 'bin/child-make').symlink_to(child)
    # Both engines derive diagnostics from basename make, including recursion.
    (root / 'bin/child').mkdir()
    (root / 'bin/child/make').symlink_to(child)
    args = ['-rR', '--no-print-directory', f'CHILD={root / "bin/child/make"}']
    if fail:
        args += ['-k', 'FAIL=1']
    environment = harness.clean_environment(root)
    descriptor = None
    external = root / 'borrowed.fifo'
    try:
        if borrowed:
            os.mkfifo(external, 0o600)
            descriptor = os.open(external, os.O_RDWR | os.O_NONBLOCK)
            os.write(descriptor, token_bytes)
            environment['MAKEFLAGS'] = f'rR -j --jobserver-auth=fifo:{external} --no-print-directory'
        else:
            args += ['-j3', '--jobserver-style=' + style]
        out, err, status, limit, leftovers = harness.capture(
            [str(root / 'bin/make'), *args], root / 'work', environment, 8)
        files = harness.snapshot({name: root / name for name in ('work', 'home', 'tmp')})
        observation = harness.Observation(out, err, status, limit, files, leftovers)
        errors = anchors(root / 'work', observation, fail)
        if borrowed:
            if not external.exists():
                errors.append('borrowed FIFO removed')
            try:
                tokens = os.read(descriptor, 64)
            except BlockingIOError:
                tokens = b''
            if sorted(tokens) != sorted(token_bytes):
                errors.append('borrowed token bytes changed/lost')
        if any((root / 'tmp').iterdir()):
            errors.append('jobserver temporary resource leaked')
        return observation, errors
    finally:
        if descriptor is not None:
            os.close(descriptor)
        external.unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=oracle.PREFIX / 'bin/make')
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--repeat', type=int, default=3)
    parser.add_argument('--artifacts', type=Path, default=ROOT / 'artifacts/jobserver')
    args = parser.parse_args()
    if not 1 <= args.repeat <= 20:
        parser.error('repeat must be between 1 and 20')
    profile = oracle.verify(args.oracle.resolve())
    reference = args.oracle.resolve()
    candidate = args.candidate.resolve()
    identity = harness.executable_identity(candidate)
    args.artifacts.mkdir(parents=True, exist_ok=True)
    destination = Path(tempfile.mkdtemp(prefix='interop-', dir=args.artifacts.resolve()))
    results = []
    for repeat in range(args.repeat):
        for style in ('fifo', 'pipe'):
            for fail in (False, True):
                baseline, baseline_errors = execute(destination, reference, reference, style, False, fail=fail)
                for parent_name, child_name, parent, child in (
                    ('gnu', 'gnu', reference, reference), ('gnu', 'rmake', reference, candidate),
                    ('rmake', 'gnu', candidate, reference), ('rmake', 'rmake', candidate, candidate),
                ):
                    observed, errors = execute(destination, parent, child, style, False, fail=fail)
                    errors += baseline_errors + harness.differences(baseline, observed)
                    row = dict(repeat=repeat, style=style, fail=fail, parent=parent_name, child=child_name, errors=errors)
                    results.append(row)
                    print(('FAIL' if errors else 'PASS'), style, parent_name, child_name, repeat, fail, errors)
                    if errors:
                        (destination / f'failure-{len(results)}.json').write_text(json.dumps(
                            dict(row=row, stdout=observed.stdout.hex(), stderr=observed.stderr.hex(),
                                 files=observed.filesystem), indent=2))
        for fail in (False, True):
            for name, binary in (('gnu', reference), ('rmake', candidate)):
                observed, errors = execute(destination, binary, binary, 'fifo', True, fail=fail)
                results.append(dict(repeat=repeat, style='borrowed', fail=fail, parent=name, child=name, errors=errors))
                print(('FAIL' if errors else 'PASS'), 'borrowed', name, repeat, fail, errors)
    observed, errors = execute(destination, candidate, candidate, 'fifo', True, b'QZ')
    results.append(dict(style='sentinel-bytes', parent='rmake', child='rmake', errors=errors))
    print(('FAIL' if errors else 'PASS'), 'sentinel token bytes', errors)
    report = dict(profile_id=profile['profile_id'], candidate=identity,
                  fixture_sha256={p.name: harness.sha256_file(p) for p in (ROOT / 'tests/jobserver').iterdir()},
                  results=results)
    (destination / 'report.json').write_text(json.dumps(report, indent=2))
    failed = sum(bool(row['errors']) for row in results)
    print(f'Jobserver interop: passed={len(results)-failed}, failed={failed}; report={destination / "report.json"}')
    return int(bool(failed))


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (harness.HarnessError, oracle.OracleError, OSError) as error:
        raise SystemExit(f'jobserver interop: {error}') from error
