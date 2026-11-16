#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare native streams and stdin source ownership across real restarts."""
import argparse
from dataclasses import dataclass
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
SETUP_SOURCE = ROOT / 'tests/native/setup_result.c'
STDIO_MODELS = (
    ('private-file', 'private', False, b'', b''),
    ('private-stdin', 'private', True, b'', b''),
    ('public', 'public', False, b'old-out:', b'old-err:'),
    ('public-write-failure', 'full', False, b'', b''),
    ('private-write-failure', 'private-full', False, b'', b''),
)



@dataclass(frozen=True)
class Model:
    case: h.Case
    stdin: str | None
    updates: dict[str, bytes]
    replacements: tuple[tuple[str, str], ...] = ()


def make_model(fixture, case_id, arguments, stdout, stderr, stdin, updates, replacements=()):
    argv = ('-rR', '--no-print-directory', *arguments)
    expected = {'exit_code': 0, 'stdout_hex': stdout.hex(), 'stderr_hex': stderr.hex()}
    case = h.Case(fixture, case_id, 'native.restart.stdio', 'required', argv, 5, {}, expected)
    return Model(case, stdin, updates, replacements)


def stdio_models(fixture):
    for case_id, mode, stdin, stdout_prefix, stderr in STDIO_MODELS:
        updates = {'seen': b'anchor\nread-\narmed-' + mode.encode() + b'\nread-1\ngoal-1\n',
                   'generated.mk': b'READY := yes\n'}
        if mode == 'private':
            updates['private-stream'] = b''
        arguments = ('MODE=' + mode, *(('-f', '-') if stdin else ()))
        yield make_model(fixture, case_id, arguments, stdout_prefix + b'ready=yes restart=1\n',
                         stderr, 'Makefile' if stdin else None, updates)


def two_restart_models(fixture):
    for phase in ('read', 'cli'):
        for stdin in (False, True):
            kind = 'stdin' if stdin else 'named'
            case_id = f'two-restarts-load-{phase}-{kind}'
            input_name = 'restart-input.mk'
            args = ('-f', 'restart-prelude.mk', '-f', '-' if stdin else input_name,
                    'LOAD_ON_READ=yes' if phase == 'read' else '--eval=load ./probe.so')
            loaded = b'seeded probe.so' if phase == 'read' else b'seeded'
            visible = b'1' if stdin else b'0'
            seen = (b'anchor\nctor-old\nsetup-old\nread-|' + loaded + b'|old|2|' + visible + b'\n'
                    b'dtor-old\nremake-module\nctor-new\nsetup-new\n'
                    b'read-1|' + loaded + b'|new|2|' + visible + b'\n'
                    b'remake-include\nctor-new\nsetup-new\n'
                    b'read-2|' + loaded + b'|new|3|' + visible + b'\n'
                    b'goal-2|' + loaded + b'|new|3|0\ndtor-new\n')
            yield make_model(fixture, case_id, args, b'child=[export-2-ready]\n', b'',
                             input_name if stdin else None, {'seen': seen, 'part.mk': b'PART := ready\n'},
                             (('probe.so', 'replacement.so'),))


def compile_fixture(compiler, source, output, *defines):
    subprocess.run([compiler, '-std=c11', '-Wall', '-Wextra', '-Werror', '-fPIC', '-shared',
                    *defines, str(source), '-o', str(output)], timeout=30, check=True)


def prepare_fixtures(root, compiler):
    stdio, two_restarts = root / 'stdio', root / 'two-restarts'
    stdio.mkdir()
    two_restarts.mkdir()
    compile_fixture(compiler, FIXTURE / 'stdio_probe.c', stdio / 'stdio_probe.so')
    shutil.copyfile(FIXTURE / 'Makefile', stdio / 'Makefile')
    for name in ('restart-prelude.mk', 'restart-input.mk'):
        shutil.copyfile(FIXTURE / name, two_restarts / name)
    epoch = 1_600_000_000
    for name, image, stamp in (('probe.so', 'old', epoch), ('replacement.so', 'new', epoch + 2)):
        destination = two_restarts / name
        compile_fixture(compiler, SETUP_SOURCE, destination, f'-DIMAGE="{image}"', '-DSETUP_RESULT=1')
        os.utime(destination, (stamp, stamp))
    for fixture in (stdio, two_restarts):
        (fixture / 'seen').write_bytes(b'anchor\n')
        for path in fixture.iterdir():
            path.chmod(0o644)
        h.validate_fixture(fixture)
    return stdio, two_restarts


def check_model(model, runtime, binaries):
    case = model.case
    row = {'id': case.id, 'argv': case.args, 'expected': case.expected, 'observations': []}
    roots = {name: runtime / name for name in ('work', 'home', 'tmp', 'bin')}
    bounded = False
    for name, binary in binaries:
        h.reset_workspace(case, runtime, binary)
        filesystem = h.snapshot(roots)
        for path, content in model.updates.items():
            filesystem['work/' + path] = file_entry(content)
        for destination, source in model.replacements:
            filesystem['work/' + destination] = filesystem['work/' + source]
        command = [str(runtime / 'bin' / 'make'), *case.args]
        if model.stdin:
            command = ['/bin/sh', '-c', 'input=$1; shift; exec "$@" < "$input"',
                       'stdin-source', model.stdin, *command]
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
    return row, bounded


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
    sources = (Path(__file__), FIXTURE / 'Makefile', FIXTURE / 'stdio_probe.c', SETUP_SOURCE,
               FIXTURE / 'restart-prelude.mk', FIXTURE / 'restart-input.mk')
    report = {
        'oracle_profile': profile['profile_id'],
        'functional_cpu_affinity': sorted(os.sched_getaffinity(0)),
        'binaries': {name: h.executable_identity(binary, oracle=index < 2)
                     for index, (name, binary) in enumerate(binaries)},
        'source_hashes': {path.name: h.sha256_file(path) for path in sources},
        'cases': [],
    }
    with tempfile.TemporaryDirectory(prefix='rmake-native-restart-stdio-') as directory:
        root = Path(directory)
        runtime = root / 'runtime'
        runtime.mkdir()
        stdio, two_restarts = prepare_fixtures(root, args.cc)
        report['fixture_hashes'] = {path.name: h.sha256_file(path) for path in sorted(stdio.iterdir())}
        report['two_restart_fixture_hashes'] = {path.name: h.sha256_file(path)
                                                for path in sorted(two_restarts.iterdir())}
        models = [*stdio_models(stdio), *two_restart_models(two_restarts)]
        for model in models:
            row, bounded = check_model(model, runtime, binaries)
            report['cases'].append(row)
            print('PASS' if row['passed'] else 'FAIL', row['id'],
                  [run['errors'] for run in row['observations']], flush=True)
            if bounded:
                break
    report['passed'] = len(report['cases']) == len(models) and all(row['passed'] for row in report['cases'])
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    return int(not report['passed'])


if __name__ == '__main__':
    raise SystemExit(main())
