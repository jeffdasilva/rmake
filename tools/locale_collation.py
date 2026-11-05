#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Required original locale collation profiles; exact bytes and complete files."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

import conformance as h
import locale_data
import oracle

ROOT = Path(__file__).resolve().parents[1]
MODELS = ROOT / 'tests/locale-collation/models.json'
MODULE = ROOT / 'tests/locale-collation/locale_switch.c'


def relative_name(name):
    if not name or b'\0' in name or name.startswith(b'/') or b'..' in name.split(b'/'):
        raise h.HarnessError('invalid collation fixture path')
    return name


def models():
    value = json.loads(MODELS.read_text())
    if value.get('schema_version') != 1:
        raise h.HarnessError('unknown collation model schema')
    rows = value['models']
    ids = [row['id'] for row in rows]
    if not rows or len(set(ids)) != len(ids) or any(not re.fullmatch(r'[a-z0-9-]+', name) for name in ids):
        raise h.HarnessError('invalid collation model identities')
    for row in rows:
        for name, content in row['files_hex'].items():
            relative_name(os.fsencode(name))
            bytes.fromhex(content)
        for item in row['raw_files']:
            if 'name_hex' in item:
                relative_name(bytes.fromhex(item['name_hex']))
                bytes.fromhex(item['content_hex'])
            elif 'module' in item:
                relative_name(os.fsencode(item['module']))
            elif 'archive' in item:
                relative_name(os.fsencode(item['archive']))
                archive([bytes.fromhex(name) for name in item['members_hex']])
            else:
                raise h.HarnessError('unknown collation fixture kind')
        for name in ('stdout_hex', 'stderr_hex'):
            bytes.fromhex(row[name])
    return value, rows


def archive(names):
    """Original small SysV ar fixture; no host ar ordering or source dependency."""
    result = bytearray(b'!<arch>\n')
    for name in names:
        if not name or len(name) > 15 or any(byte in name for byte in (0, 10, 47)):
            raise h.HarnessError('invalid short archive fixture member')
        result += (name + b'/' + b' ' * (15 - len(name)) + b'1           '
                   + b'0     ' + b'0     ' + b'100644  ' + b'0         ' + b'`\n')
    return bytes(result)


def fixture(path, model, module):
    path.mkdir(mode=0o755)
    for name, content in model['files_hex'].items():
        (path / name).write_bytes(bytes.fromhex(content))
        (path / name).chmod(0o644)
    for item in model['raw_files']:
        if 'module' in item:
            target = path / item['module']
            target.write_bytes(module.read_bytes())
            target.chmod(0o755)
        elif 'archive' in item:
            target = path / item['archive']
            target.write_bytes(archive([bytes.fromhex(name) for name in item['members_hex']]))
            target.chmod(0o644)
        else:
            name = os.fsencode(path) + b'/' + bytes.fromhex(item['name_hex'])
            os.makedirs(os.path.dirname(name), mode=0o755, exist_ok=True)
            with open(name, 'wb') as output:
                output.write(bytes.fromhex(item['content_hex']))
            os.chmod(name, 0o644)


def assess(model, observation, initial):
    errors = []
    if observation.stdout != bytes.fromhex(model['stdout_hex']):
        errors.append('exact stdout')
    if observation.stderr != bytes.fromhex(model['stderr_hex']):
        errors.append('exact stderr')
    if observation.returncode != model['status']:
        errors.append('exact exit status')
    if observation.limit or observation.leftover_processes:
        errors.append('process bounds')
    if observation.filesystem != initial:
        errors.append('unchanged complete filesystem')
    return errors


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--locale-root', type=Path, default=ROOT / '.tools/locale-data')
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--case', action='append')
    args = parser.parse_args(argv)
    value, selected = models()
    if args.case:
        if set(args.case) - {row['id'] for row in selected}:
            parser.error('unknown exact collation case')
        selected = [row for row in selected if row['id'] in args.case]
    profile = locale_data.provision(args.locale_root.resolve())
    oracle.verify(args.oracle.resolve())
    binaries = [args.oracle.resolve(), args.oracle.resolve()]
    if args.candidate:
        binaries.append(args.candidate.resolve())
    report = {
        'classification': value['classification'], 'models_sha256': h.sha256_file(MODELS),
        'tool_sha256': h.sha256_file(Path(__file__)), 'observer_sha256': h.sha256_file(Path(h.__file__)),
        'module_source_sha256': h.sha256_file(MODULE), 'locale_profile': profile,
        'binaries': [h.executable_identity(binary) for binary in binaries],
        'affinity': sorted(os.sched_getaffinity(0)), 'oracle_only': args.candidate is None, 'cases': [],
    }
    with tempfile.TemporaryDirectory(prefix='rmake-collation-') as temporary:
        root = Path(temporary)
        module = root / 'locale_switch.so'
        command = ['/usr/bin/cc', '-shared', '-fPIC', '-O2', '-Wall', '-Wextra', '-Werror', str(MODULE), '-o', str(module)]
        subprocess.run(command, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
        report['module_build'] = {'command': command, 'compiler_sha256': h.sha256_file(Path('/usr/bin/cc').resolve()),
                                  'binary_sha256': h.sha256_file(module)}
        runtime = root / 'runtime'
        runtime.mkdir(mode=0o755)
        for model in selected:
            inputs = root / model['id']
            fixture(inputs, model, module)
            environment = {**model['environment'], **profile['environment']}
            case = h.Case(inputs, model['id'], 'locale.collation', 'required', tuple(model['args']), 5, environment, {})
            row = {'id': model['id'], 'model': model, 'environment': environment, 'observations': [], 'errors': []}
            observations = []
            for binary in binaries:
                h.reset_workspace(case, runtime, binary)
                roots = {name: runtime / name for name in ('work', 'home', 'tmp')}
                initial = h.snapshot(roots)
                out, err, status, limit, left = h.capture([str(runtime / 'bin/make'), *case.args], runtime / 'work',
                    h.clean_environment(runtime, environment), case.timeout)
                observation = h.Observation(out, err, status, limit, h.snapshot(roots), left)
                observations.append(observation)
                row['observations'].append({**h.observation_json(observation), 'stdout_hex': out.hex(), 'stderr_hex': err.hex()})
                row['errors'].append(assess(model, observation, initial))
            row['differences'] = [h.differences(observations[0], other) for other in observations[1:]]
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
