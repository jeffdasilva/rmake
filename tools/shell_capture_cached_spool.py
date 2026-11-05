#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare low shell-capture descriptors with an already active output spool."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

import conformance as h
import oracle


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/shell-capture-cached-spool'
MODELS = (
    ('02-success', 'stdin-stderr', './capture', 0, 'closed', 'capture', 'child-stderr\n'),
    ('02-denied', 'stdin-stderr', './denied', 127, 'pipe-02', '',
     'make: ./denied: Permission denied\n'),
    ('012-success', 'all', './capture', 23, 'closed', '', 'child-stderr\n'),
    ('012-denied', 'all', './denied', 127, 'pipe-01', '',
     'make: ./denied: Permission denied\n'),
    ('02-path-retry', 'stdin-stderr', 'capture', 0, 'closed', 'capture', 'child-stderr\n'),
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, required=True)
    engine = parser.add_mutually_exclusive_group(required=True)
    engine.add_argument('--candidate', type=Path)
    engine.add_argument('--oracle-only', action='store_true')
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--cc', default='/usr/bin/cc')
    args = parser.parse_args()
    reference = args.oracle.resolve(strict=True)
    profile = oracle.verify(reference)
    binaries = [('gnu-first', reference), ('gnu-second', reference)]
    if args.candidate:
        binaries.append(('candidate', args.candidate.resolve(strict=True)))
    report = {
        'oracle_profile': profile['profile_id'],
        'functional_cpu_affinity': sorted(os.sched_getaffinity(0)),
        'binaries': {name: h.executable_identity(binary, oracle=index < 2)
                     for index, (name, binary) in enumerate(binaries)},
        'source_hashes': {path.name: h.sha256_file(path) for path in
                          (Path(__file__), FIXTURE / 'Makefile', FIXTURE / 'spool_slots.c', FIXTURE / 'capture.c')},
        'cases': [],
    }
    with tempfile.TemporaryDirectory(prefix='rmake-shell-capture-cached-spool-') as directory:
        root = Path(directory)
        fixture, runtime = root / 'input', root / 'runtime'
        fixture.mkdir()
        runtime.mkdir()
        for source, target, flags in (('spool_slots.c', 'spool_slots.so', ['-fPIC', '-shared']),
                                      ('capture.c', 'capture', [])):
            subprocess.run([args.cc, '-std=c11', '-Wall', '-Wextra', '-Werror', *flags,
                            str(FIXTURE / source), '-o', str(fixture / target)], timeout=30, check=True)
        shutil.copyfile(FIXTURE / 'Makefile', fixture / 'Makefile')
        (fixture / 'seen').write_bytes(b'anchor\n')
        (fixture / 'denied').write_bytes(b'not executable\n')
        for name in ('Makefile', 'seen', 'denied', 'spool_slots.so'):
            (fixture / name).chmod(0o644)
        (fixture / 'capture').chmod(0o755)
        for directory, name in (('denied-path', 'denied'), ('valid-path', 'capture')):
            (fixture / directory).mkdir()
            (fixture / directory).chmod(0o755)
            shutil.copy2(fixture / name, fixture / directory / 'capture')
        h.validate_fixture(fixture)
        report['fixture_hashes'] = {str(path.relative_to(fixture)): h.sha256_file(path)
                                    for path in sorted(fixture.rglob('*')) if path.is_file()}
        roots = {name: runtime / name for name in ('work', 'home', 'tmp')}
        for case_id, closed, command, status, descriptors, value, stderr in MODELS:
            witness = f'status=[{status}]|slots=[{descriptors}]|result=[{value}]\n'.encode()
            expected = {'exit_code': 0, 'stdout_hex': (b'spool-before\n' + witness).hex(),
                        'stderr_hex': stderr.encode().hex()}
            argv = ('-rR', '--no-print-directory', '-j2', '-Otarget',
                    'LAYOUT=' + closed, 'CAPTURE_COMMAND=' + command)
            if case_id.endswith('retry'):
                argv += ('PATH=denied-path:valid-path:/usr/bin:/bin',)
            case = h.Case(fixture, case_id, 'shell.capture.cached-spool', 'required',
                          argv, 5, {}, expected, 'separate')
            h.reset_workspace(case, runtime, reference)
            filesystem = h.snapshot(roots)
            seen = b'anchor\n' + witness
            filesystem['work/seen'].update(content=base64.b64encode(seen).decode(),
                                           size=len(seen), sha256=hashlib.sha256(seen).hexdigest())
            row = {'id': case_id, 'argv': argv, 'expected': expected,
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
            row['passed'] = (not bounded and len(row['observations']) == len(binaries)
                             and all(not run['errors'] for run in row['observations']))
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
