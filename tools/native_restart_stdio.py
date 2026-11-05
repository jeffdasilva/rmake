#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare public and plugin-private buffered streams across real restarts."""
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
FIXTURE = ROOT / 'tests/native-restart-stdio'
MODELS = (
    ('private-file', 'private', False, b'', b''),
    ('private-stdin', 'private', True, b'', b''),
    ('public', 'public', False, b'old-out:', b'old-err:'),
    ('public-write-failure', 'full', False, b'', b''),
    ('private-write-failure', 'private-full', False, b'', b''),
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
                          (Path(__file__), FIXTURE / 'Makefile', FIXTURE / 'stdio_probe.c')},
        'cases': [],
    }
    with tempfile.TemporaryDirectory(prefix='rmake-native-restart-stdio-') as directory:
        root = Path(directory)
        fixture, runtime = root / 'input', root / 'runtime'
        fixture.mkdir()
        runtime.mkdir()
        subprocess.run([args.cc, '-std=c11', '-Wall', '-Wextra', '-Werror', '-fPIC', '-shared',
                        str(FIXTURE / 'stdio_probe.c'), '-o', str(fixture / 'stdio_probe.so')],
                       timeout=30, check=True)
        shutil.copyfile(FIXTURE / 'Makefile', fixture / 'Makefile')
        (fixture / 'seen').write_bytes(b'anchor\n')
        for path in fixture.iterdir():
            path.chmod(0o644)
        h.validate_fixture(fixture)
        report['fixture_hashes'] = {path.name: h.sha256_file(path) for path in sorted(fixture.iterdir())}
        roots = {name: runtime / name for name in ('work', 'home', 'tmp', 'bin')}
        for case_id, mode, stdin, stdout_prefix, stderr in MODELS:
            stdout = stdout_prefix + b'ready=yes restart=1\n'
            expected = {'exit_code': 0, 'stdout_hex': stdout.hex(), 'stderr_hex': stderr.hex()}
            argv = ('-rR', '--no-print-directory', 'MODE=' + mode, *(('-f', '-') if stdin else ()))
            case = h.Case(fixture, case_id, 'native.restart.stdio', 'required', argv, 5, {}, expected, 'separate')
            row = {'id': case_id, 'argv': argv, 'expected': expected, 'observations': []}
            report['cases'].append(row)
            bounded = False
            for name, binary in binaries:
                h.reset_workspace(case, runtime, binary)
                filesystem = h.snapshot(roots)
                filesystem['work/seen'] = file_entry(
                    b'anchor\nread-\narmed-' + mode.encode() + b'\nread-1\ngoal-1\n')
                filesystem['work/generated.mk'] = file_entry(b'READY := yes\n')
                if mode == 'private':
                    filesystem['work/private-stream'] = file_entry(b'')
                command = [str(runtime / 'bin/make'), *argv]
                if stdin:
                    command = ['/bin/sh', '-c', 'exec "$@" < Makefile', 'stdin-source', *command]
                out, err, status, bound, leftovers = h.capture(
                    command, runtime / 'work', h.clean_environment(runtime), case.timeout)
                observed = h.Observation(out, err, status, bound, h.snapshot(roots), leftovers)
                errors = h.expectation_errors(case, observed)
                if observed.filesystem != filesystem:
                    errors.append('independent complete filesystem')
                bounded = bool(bound or leftovers or status < 0)
                row['observations'].append({
                    **h.observation_json(observed), 'engine': name, 'errors': errors,
                    'stdout_hex': out.hex(), 'stderr_hex': err.hex(), 'expected_filesystem': filesystem,
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
