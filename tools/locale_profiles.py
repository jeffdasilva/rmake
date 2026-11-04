#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original required glibc C/C.UTF8 profiles, separate from NLS acceptance."""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import tempfile

import conformance as h
import oracle

ROOT = Path(__file__).resolve().parents[1]
MODELS = ROOT / 'tests/locales/models.json'


def models():
    data = json.loads(MODELS.read_text())
    if data.get('schema_version') != 1:
        raise h.HarnessError('unknown locale model schema')
    names = [bytes.fromhex(name) for name in data['filenames_hex']]
    if len(set(names)) != len(names) or any(not name or b'/' in name or b'\0' in name or name in (b'.', b'..') for name in names):
        raise h.HarnessError('invalid locale filename fixture')
    result = data['models']
    ids = [model['id'] for model in result]
    if not result or len(set(ids)) != len(ids) or any(not re.fullmatch(r'[a-z0-9-]+', value) for value in ids):
        raise h.HarnessError('invalid locale case identities')
    return data, names, result


def file_entry(content):
    return {'mode': 0o644, 'type': 'file', 'content': base64.b64encode(content).decode(),
            'size': len(content), 'sha256': hashlib.sha256(content).hexdigest()}


def expected_files(source, names):
    result = {key: {'mode': 0o755, 'type': 'directory'} for key in ('work', 'work/names', 'home', 'tmp')}
    result['work/Makefile'] = file_entry(source)
    result.update({'work/names/' + os.fsdecode(name): file_entry(b'') for name in names})
    return result


def assess(model, observation, initial):
    errors = []
    if (observation.stdout != bytes.fromhex(model['stdout_hex']) or observation.stderr
            or observation.returncode != 0 or observation.limit or observation.leftover_processes):
        errors.append('independent exact streams/status/process bounds')
    if observation.filesystem != initial:
        errors.append('independent unchanged complete filesystem')
    return errors


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--case', action='append')
    args = parser.parse_args(argv)
    data, names, selected = models()
    if args.case:
        if set(args.case) - {model['id'] for model in selected}:
            parser.error('unknown exact locale case')
        selected = [model for model in selected if model['id'] in args.case]
    oracle.verify(args.oracle.resolve())
    binaries = [args.oracle.resolve(), args.oracle.resolve()]
    if args.candidate:
        binaries.append(args.candidate.resolve())
    report = {'classification': data['classification'], 'manifest_sha256': h.sha256_file(MODELS),
              'tool_sha256': h.sha256_file(Path(__file__)), 'observer_sha256': h.sha256_file(Path(h.__file__)),
              'binaries': [h.executable_identity(binary) for binary in binaries],
              'host': {'libc': platform.libc_ver(), 'kernel': platform.uname()._asdict()},
              'affinity': sorted(os.sched_getaffinity(0)), 'oracle_only': args.candidate is None, 'cases': []}
    with tempfile.TemporaryDirectory(prefix='rmake-locale-profiles-') as tmp:
        root = Path(tmp); runtime = root / 'runtime'; runtime.mkdir(mode=0o755)
        report['runtime_root'] = str(runtime)
        for model in selected:
            source = bytes.fromhex(model['source_hex'])
            fixture = root / model['id']; fixture.mkdir(mode=0o755)
            (fixture / 'names').mkdir(mode=0o755)
            (fixture / 'Makefile').write_bytes(source); (fixture / 'Makefile').chmod(0o644)
            for name in names:
                descriptor = os.open(os.fsencode(fixture / 'names') + b'/' + name, os.O_WRONLY | os.O_CREAT, 0o644)
                os.close(descriptor)
            initial = expected_files(source, names)
            case = h.Case(fixture, model['id'], 'locale.ctype', 'required', tuple(model['arguments']), 5, model['environment'], {})
            row = {'id': model['id'], 'model': model, 'observations': [], 'errors': []}
            values = []
            for binary in binaries:
                h.reset_workspace(case, runtime, binary)
                out, err, status, limit, left = h.capture([str(runtime / 'bin/make'), *case.args], runtime / 'work',
                    h.clean_environment(runtime, case.environment), case.timeout)
                observation = h.Observation(out, err, status, limit,
                    h.snapshot({name: runtime / name for name in ('work', 'home', 'tmp')}), left)
                values.append(observation)
                row['observations'].append({**h.observation_json(observation), 'stdout_hex': out.hex(), 'stderr_hex': err.hex()})
                row['errors'].append(assess(model, observation, initial))
            row['differences'] = [h.differences(values[0], value) for value in values[1:]]
            row['passed'] = not any(row['errors']) and not any(row['differences'])
            report['cases'].append(row)
            print(('PASS ' if row['passed'] else 'FAIL ') + model['id'], flush=True)
    report['totals'] = {'cases': len(selected), 'pass': sum(row['passed'] for row in report['cases']),
                        'fail': sum(not row['passed'] for row in report['cases'])}
    report['passed'] = report['totals']['fail'] == 0
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
