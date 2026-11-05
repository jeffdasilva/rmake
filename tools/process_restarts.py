#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original process-image restart profiles with exact effects and full filesystems."""
from __future__ import annotations

import argparse
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
FIXTURES = ROOT / 'tests/process-restarts'
IDS = ('process-restart-tempdir', 'process-restart-constructor', 'process-restart-replaced-image')


def models():
    value = json.loads((FIXTURES / 'models.json').read_bytes())
    if value.get('schema_version') != 1 or tuple(row['id'] for row in value['models']) != IDS:
        raise h.HarnessError('invalid process restart inventory')
    hashes = value['fixture_sha256']
    if set(hashes) != {'entry.c', 'replace.py', 'replacement.sh'}:
        raise h.HarnessError('invalid process restart fixture inventory')
    for name, digest in hashes.items():
        if Path(name).name != name or h.sha256_file(FIXTURES / name) != digest:
            raise h.HarnessError('process restart fixture identity changed')
    return value['models']


def prepare(model, path, compiler):
    path.mkdir()
    (path / 'Makefile').write_bytes(bytes.fromhex(model['makefile_hex']))
    (path / 'seen').write_bytes(b'anchor\n')
    compilation = None
    if model['kind'] == 'warning':
        (path / 'not-dir').write_bytes(b'regular\n')
    elif model['kind'] == 'constructor':
        if not compiler:
            raise h.HarnessError('C compiler required for constructor profile')
        source = FIXTURES / 'entry.c'
        shutil.copy2(source, path / 'entry.c')
        args = [compiler, '-std=c11', '-Wall', '-Wextra', '-Werror', '-O2', '-shared', '-fPIC',
                str(path / 'entry.c'), '-o', str(path / 'entry.so')]
        result = subprocess.run(args, capture_output=True, timeout=30, check=True)
        compilation = {'command': args, 'stdout_hex': result.stdout.hex(), 'stderr_hex': result.stderr.hex(),
                       'source_sha256': h.sha256_file(source), 'module_sha256': h.sha256_file(path / 'entry.so')}
    elif model['kind'] == 'replacement':
        shutil.copy2(FIXTURES / 'replacement.sh', path / 'replacement.sh')
        (path / 'replacement.sh').chmod(0o755)
        shutil.copy2(FIXTURES / 'replace.py', path / 'replace.py')
    else:
        raise h.HarnessError('unknown process restart model kind')
    for entry in path.iterdir():
        os.utime(entry, ns=(1_600_000_000_123_456_789,) * 2)
    return h.Case(path, model['id'], 'restart.process-image', 'required',
                  ('-rR', '--no-print-directory'), 6.0, {}, {}), compilation


def expected(model, runtime):
    roots = {name: runtime / name for name in ('work', 'home', 'tmp', 'bin')}
    # Input alias is chosen independently for each engine before execution.
    filesystem = h.snapshot(roots)
    filesystem['work/generated.mk'] = file_entry(b'READY:=yes\n')
    filesystem['work/seen'] = file_entry(bytes.fromhex(model['seen_hex']))
    if model['kind'] == 'replacement':
        filesystem.pop('work/replacement.sh')
        filesystem['bin/make'] = file_entry((FIXTURES / 'replacement.sh').read_bytes(), 0o755)
    stderr = ((b'make: MAKE_TMPDIR value ' + os.fsencode(runtime / 'work/not-dir')
               + b': not a directory\n') * 2 if model['kind'] == 'warning' else b'')
    return roots, filesystem, stderr


def file_entry(content, mode=0o644):
    import base64
    return {'type': 'file', 'mode': mode, 'size': len(content),
            'sha256': hashlib.sha256(content).hexdigest(), 'content': base64.b64encode(content).decode()}


def assess(observation, filesystem, stderr):
    errors = []
    if observation.stdout:
        errors.append('exact empty stdout')
    if observation.stderr != stderr:
        errors.append('exact restart warning/diagnostic bytes')
    if observation.returncode != 0 or observation.limit or observation.leftover_processes:
        errors.append('exact success and process bounds')
    if observation.filesystem != filesystem:
        errors.append('complete work/home/tmp/bin filesystem')
    return errors


def run(selected, reference, candidate, compiler, directory):
    runtime = directory / 'runtime'
    runtime.mkdir()
    records = []
    for model in selected:
        case, compilation = prepare(model, directory / model['id'], compiler)
        executions = []
        for binary in ([reference, reference] if candidate is None else [reference, reference, candidate]):
            h.reset_workspace(case, runtime, binary)
            env = {}
            if model['kind'] == 'warning':
                env['MAKE_TMPDIR'] = str(runtime / 'work/not-dir')
            if model['kind'] == 'constructor':
                env['LD_PRELOAD'] = str(runtime / 'work/entry.so')
            roots, filesystem, stderr = expected(model, runtime)
            out, err, status, bound, left = h.capture([str(runtime / 'bin/make'), *case.args],
                runtime / 'work', h.clean_environment(runtime, env), case.timeout)
            observed = h.Observation(out, err, status, bound, h.snapshot(roots), left)
            errors = assess(observed, filesystem, stderr)
            executions.append({'binary': str(binary), 'stdout_hex': out.hex(), 'stderr_hex': err.hex(),
                'status': status, 'bound': bound, 'leftovers': left, 'filesystem': observed.filesystem,
                'expected_filesystem': filesystem, 'expected_stderr_hex': stderr.hex(), 'errors': errors})
        passed = not any(row['errors'] for row in executions)
        records.append({'id': model['id'], 'compilation': compilation, 'observations': executions, 'passed': passed})
        print(('PASS ' if passed else 'FAIL ') + model['id'], flush=True)
    passed = sum(row['passed'] for row in records)
    return {'classification': 'Original GNU2 process-entry/image restart profiles; no general performance or milestone claim',
            'records': records, 'passed': passed == len(records),
            'totals': {'cases': len(records), 'pass': passed, 'fail': len(records) - passed}}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--oracle-only', action='store_true')
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--case', action='append', default=[])
    args = parser.parse_args(argv)
    if args.oracle_only == (args.candidate is not None):
        parser.error('select oracle-only or one candidate')
    report = {'passed': False}
    try:
        selected = models()
        if set(args.case) - {row['id'] for row in selected}:
            raise h.HarnessError('unknown exact process restart case')
        selected = [row for row in selected if not args.case or row['id'] in args.case]
        profile = oracle.verify(args.oracle)
        binaries = {'oracle': h.executable_identity(args.oracle, oracle=True)}
        if args.candidate is not None:
            binaries['candidate'] = h.executable_identity(args.candidate)
        with tempfile.TemporaryDirectory(prefix='rmake-process-restarts-') as temporary:
            report = run(selected, Path(binaries['oracle']['path']),
                         Path(binaries['candidate']['path']) if args.candidate else None,
                         shutil.which('cc'), Path(temporary))
        for name, identity in binaries.items():
            if h.sha256_file(Path(identity['path'])) != identity['sha256']:
                raise h.HarnessError(name + ' executable changed during comparison')
        report['binaries'] = binaries
        report['oracle_profile_id'] = profile['profile_id']
        report['source_hashes'] = {str(path.relative_to(ROOT)): h.sha256_file(path) for path in
            [Path(__file__).resolve(), FIXTURES / 'models.json', FIXTURES / 'entry.c',
             FIXTURES / 'replacement.sh', FIXTURES / 'replace.py', ROOT / 'tools/conformance.py']}
        status = 0 if report['passed'] else 1
    except (h.HarnessError, oracle.OracleError, OSError, ValueError, subprocess.SubprocessError, KeyError, TypeError) as error:
        report.update(error=str(error), passed=False)
        print('ERROR ' + str(error), flush=True)
        status = 2
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    return status


if __name__ == '__main__':
    raise SystemExit(main())
