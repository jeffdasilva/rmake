#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare exact clock-skew diagnostics on trusted fixed-time Makefiles."""

import argparse
import base64
import json
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib

import conformance as harness
import oracle

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/clock-skew'
EPOCH_NS = 1_600_000_000_000_000_000
OLD = -5_000_000_000


def stamp(path, offset):
    time = EPOCH_NS + offset
    os.utime(path, ns=(time, time), follow_symlinks=False)


def input_path(root, name):
    path = Path(name)
    if path.is_absolute() or not path.parts or '..' in path.parts:
        raise harness.HarnessError(f'invalid clock fixture input path: {name}')
    return root / path


def materialize(model, directory, preload):
    directory.mkdir()
    main = directory / 'Makefile'
    main.write_text(model['makefile'], encoding='utf-8')
    stamp(main, model.get('main_time', OLD))
    for name, spec in model.get('inputs', {}).items():
        offset, content = (spec, b'') if isinstance(spec, int) else (spec[0], bytes.fromhex(spec[1]))
        path = input_path(directory, name)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        stamp(path, offset)
    for name, (referent, offset) in model.get('symlinks', {}).items():
        path = input_path(directory, name)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.symlink_to(referent)
        stamp(path, offset)
    harness.validate_fixture(directory)
    environment = {'LD_PRELOAD': str(preload), 'RMAKE_TEST_CLOCK': model.get('clock', 'fixed')}
    if 'clock_reads' in model:
        environment['RMAKE_TEST_CLOCK_TRACE'] = '1'
    return harness.Case(
        directory, model['id'], 'filesystem.clock-skew', 'required',
        ('-rR', '--no-print-directory', *model.get('args', ())), 10, environment,
        {'exit_code': model['exit_code'], 'stdout': model['output'], 'stderr': ''}, 'merged',
    )


def observe(model, case, workspace, executable):
    harness.reset_workspace(case, workspace, executable)
    work = workspace / 'work'
    for name, (_, offset) in model.get('symlinks', {}).items():
        stamp(work / name, offset)
    out, err, status, limit, leftovers = harness.capture(
        [str(workspace / 'bin' / 'make'), *case.args], work,
        harness.clean_environment(workspace, case.environment), case.timeout, merged=True,
    )
    files = harness.snapshot({name: workspace / name for name in ('work', 'home', 'tmp')})
    return harness.Observation(out, err, status, limit, files, leftovers)


def check(models, identities, artifacts):
    rows = []
    with tempfile.TemporaryDirectory(prefix='rmake-clock-skew-') as temporary:
        root = Path(temporary)
        preload = root / 'clock.so'
        subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-shared', '-fPIC', '-o', str(preload),
                        str(FIXTURE / 'clock.c'), '-ldl'], check=True)
        workspace = root / 'execution'
        workspace.mkdir()
        for model in models:
            case = materialize(model, root / model['id'], preload)
            reference = observe(model, case, workspace, Path(identities['oracle']['path']))
            candidate = observe(model, case, workspace, Path(identities['candidate']['path']))
            errors = harness.expectation_errors(case, reference)
            errors += harness.differences(reference, candidate)
            if 'clock_reads' in model:
                expected = b'realtime\n' * model['clock_reads']
                recorded = reference.filesystem.get('work/clock-events', {}).get('content', '')
                if base64.b64decode(recorded) != expected:
                    errors.append('oracle disagrees with expected clock read count')
            row = {'case': case.id, 'errors': errors}
            if errors:
                artifact = harness.save_artifact(artifacts, case, workspace, identities,
                                                 reference, candidate, errors, 'FAIL')
                row['artifact'] = str(artifact)
            rows.append(row)
            print(f'{"FAIL" if errors else "PASS"} {case.id}'
                  + (': ' + ', '.join(errors) if errors else ''), flush=True)
    failed = sum(bool(row['errors']) for row in rows)
    print(f'Clock skew: pass={len(rows) - failed}, fail={failed}', flush=True)
    return {'identities': identities, 'passed': len(rows) - failed, 'failed': failed, 'cases': rows}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--self-test', action='store_true')
    mode.add_argument('--candidate', type=Path)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--case', action='append', default=[])
    parser.add_argument('--artifacts', type=Path, default=ROOT / 'artifacts/clock-skew')
    parser.add_argument('--report', type=Path)
    args = parser.parse_args(argv)
    try:
        available = tomllib.loads((FIXTURE / 'models.toml').read_text())['cases']
        if set(args.case) - {model['id'] for model in available}:
            raise harness.HarnessError('unknown clock-skew case')
        models = [model for model in available if not args.case or model['id'] in args.case]
        reference = harness.executable_identity(args.oracle, oracle=True)
        reference['profile'] = oracle.verify(Path(reference['path']))
        candidate = reference if args.self_test else harness.executable_identity(args.candidate)
        report = check(models, {'oracle': reference, 'candidate': candidate}, args.artifacts)
        if args.report:
            args.report.parent.mkdir(parents=True, exist_ok=True)
            args.report.write_text(json.dumps(report, indent=2) + '\n')
        return int(report['failed'] != 0)
    except (harness.HarnessError, oracle.OracleError, OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(2, f'Clock skew: {error}\n')


if __name__ == '__main__':
    raise SystemExit(main())
