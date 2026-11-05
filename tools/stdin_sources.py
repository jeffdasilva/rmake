#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Strict original stdin-source lifetimes, real exec and owned-file identities."""
from __future__ import annotations

import argparse
import base64
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import sys
import tempfile
from typing import Any

import conformance as h
import oracle as reference_oracle

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'tests/stdin-sources'
MODELS = FIXTURES / 'models.json'


def relative(name: str) -> Path:
    value = Path(name)
    if not name or value.is_absolute() or '..' in value.parts or '\0' in name or value == Path('.'):
        raise h.HarnessError('stdin fixture must stay inside its declared directory')
    return value


def models() -> list[dict[str, Any]]:
    data = json.loads(MODELS.read_bytes())
    if data.get('schema_version') != 1:
        raise h.HarnessError('unsupported stdin source schema')
    for name, expected in data['source_hashes'].items():
        if h.sha256_file(FIXTURES / relative(name)) != expected:
            raise h.HarnessError('original stdin fixture identity changed: ' + name)
    rows = data['models']
    if not rows or len({row['id'] for row in rows}) != len(rows):
        raise h.HarnessError('empty or duplicate stdin inventory')
    for row in rows:
        relative(row['id'])
        if not row['id'].startswith('stdin-owned-'):
            raise h.HarnessError('stdin case requires exact scoped ID')
        for name in (*row['files_hex'], *row['directory_slots'], *row['expected_updates_hex'], *row['expected_removals']):
            relative(name)
        for value in row['args_hex']:
            if b'\0' in bytes.fromhex(value): raise h.HarnessError('stdin argv contains a NUL')
        if row['native']:
            if row['module_source'] not in ('probe.c', 'legacy_probe.c'):
                raise h.HarnessError('unknown original stdin module')
            relative(row['module_name'])
        if row['wrapper_mode'] not in ('pipe', 'signal', 'identity', 'cosmetic', 'merged'):
            raise h.HarnessError('unknown stdin launch mode')
        identity = row.get('owned_temp_identity')
        if identity is not None and (identity['directory'] != '@TMP@' or identity['basename_pattern'] != 'Gm[A-Za-z0-9]{6}' or identity['mode'] != 0o600 or not identity['must_remain']):
            raise h.HarnessError('unsupported declared owned-temp identity relation')
    return rows


def render(value: bytes, runtime: Path) -> bytes:
    return value.replace(b'@WORK@', os.fsencode(runtime / 'work')).replace(b'@TMP@', os.fsencode(runtime / 'tmp'))


def file_record(content: bytes, mode: int = 0o644) -> dict[str, Any]:
    return {'type': 'file', 'mode': mode, 'size': len(content),
            'sha256': hashlib.sha256(content).hexdigest(), 'content': base64.b64encode(content).decode()}


def prepare(row: dict, directory: Path, compiler: str) -> tuple[h.Case, dict | None]:
    directory.mkdir()
    for name, encoded in row['files_hex'].items():
        path = directory / relative(name); path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(bytes.fromhex(encoded)); path.chmod(0o644)
    for name in row['directory_slots']: (directory / name).mkdir(parents=True, exist_ok=True)
    (directory / 'launch.py').write_bytes((FIXTURES / 'launch.py').read_bytes())
    compilation = None
    if row['native']:
        source = directory / 'module.c'; source.write_bytes((FIXTURES / row['module_source']).read_bytes())
        output = directory / row['module_name']
        command = [compiler, '-std=c11', '-O2', '-g0', '-Wall', '-Wextra', '-Werror', '-shared', '-fPIC', str(source), '-o', str(output)]
        out, err, status, limit, leftovers = h.capture(command, directory, {'PATH': '/usr/bin:/bin', 'LC_ALL': 'C', 'TZ': 'UTC'}, 15)
        compilation = {'argv': command, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(), 'status': status,
                       'limit': limit, 'leftover_processes': leftovers, 'source_sha256': h.sha256_file(source)}
        if status or limit or leftovers: raise h.HarnessError('original stdin module compilation failed: ' + json.dumps(compilation))
        compilation['module_sha256'] = h.sha256_file(output)
        if row.get('change_directory'): shutil.copy2(output, directory / 'sub' / row['module_name'])
    h.validate_fixture(directory)
    return h.Case(directory, row['id'], 'stdin.owned', 'required', (), 8, {}, {}), compilation


def expected_filesystem(row: dict, case: h.Case, runtime: Path, oracle: Path) -> dict:
    h.reset_workspace(case, runtime, oracle)
    expected = h.snapshot({name: runtime / name for name in ('work', 'home', 'tmp')})
    expected['work/seen'] = file_record(bytes.fromhex(row['expected_seen_hex']))
    for name, encoded in row['expected_updates_hex'].items(): expected['work/' + name] = file_record(bytes.fromhex(encoded))
    for name in row['expected_removals']: del expected['work/' + name]
    return expected


@dataclass(frozen=True)
class Execution:
    observation: h.Observation
    identity: dict | None
    final_identity: dict | None
    identity_raw_hex: str | None


def execute(row: dict, case: h.Case, runtime: Path, binary: Path, identity_output: Path) -> Execution:
    h.reset_workspace(case, runtime, binary)
    environment = h.clean_environment(runtime, {key: os.fsdecode(render(os.fsencode(value), runtime)) for key, value in row['environment'].items()})
    arguments = [render(bytes.fromhex(value), runtime) for value in row['args_hex']]
    command = [os.fsencode(sys.executable), b'-B', os.fsencode(runtime / 'work/launch.py'), os.fsencode(runtime / 'work/input.mk'),
               row['wrapper_mode'].encode(), os.fsencode(identity_output), os.fsencode(runtime / 'bin/make'), *arguments]
    out, err, status, limit, leftovers = h.capture(command, runtime / 'work', environment, case.timeout)
    filesystem = h.snapshot({name: runtime / name for name in ('work', 'home', 'tmp')})
    identity = None; raw = None; final = None
    if identity_output.exists():
        content = identity_output.read_bytes(); raw = content.hex(); identity = json.loads(content)
        path = Path(os.fsdecode(bytes.fromhex(identity['path_hex'])))
        # Only independently sampled paths inside the predetermined temp root
        # may be queried. Validation below still requires exact raw spelling.
        if path.parent == runtime / 'tmp':
            try:
                info = path.lstat()
                final = {'device': info.st_dev, 'inode': info.st_ino, 'mode': stat.S_IMODE(info.st_mode), 'regular': stat.S_ISREG(info.st_mode)}
            except FileNotFoundError: pass
    return Execution(h.Observation(out, err, status, limit, filesystem, leftovers), identity, final, raw)


def assess_execution(row: dict, runtime: Path, base_filesystem: dict, execution: Execution) -> dict:
    errors = []; observation = execution.observation; expected = dict(base_filesystem)
    if observation.stdout != render(bytes.fromhex(row['expected_stdout_hex']), runtime): errors.append('exact stdout')
    if observation.stderr != render(bytes.fromhex(row['expected_stderr_hex']), runtime): errors.append('exact stderr')
    if observation.returncode != row['expected_status']: errors.append('exit status')
    if observation.limit or observation.leftover_processes: errors.append('process/output/time containment')
    if base64.b64decode(observation.filesystem.get('work/seen', {}).get('content', '')) != bytes.fromhex(row['expected_seen_hex']): errors.append('independent ordered effect witness')
    declared = row.get('owned_temp_identity'); identity = execution.identity
    if declared:
        if identity is None: errors.append('missing independently captured pre-exit source')
        else:
            selected = Path(os.fsdecode(bytes.fromhex(identity['path_hex'])))
            expected_dir = Path(os.fsdecode(render(os.fsencode(declared['directory']), runtime)))
            valid = selected.parent == expected_dir and re.fullmatch(declared['basename_pattern'], selected.name) is not None
            if not valid: errors.append('owned source outside exact declared namespace')
            if identity.get('captured_before_exit') is not True: errors.append('source not sampled before exit')
            if identity['mode'] != declared['mode'] or identity['content_hex'] != declared['content_hex'] or identity['size'] != len(bytes.fromhex(declared['content_hex'])):
                errors.append('independently captured source mode/size/full bytes')
            expected_final = {'device': identity['device'], 'inode': identity['inode'], 'mode': declared['mode'], 'regular': True}
            if execution.final_identity != expected_final: errors.append('final owned source physical identity')
            if valid: expected[str(selected.relative_to(runtime))] = file_record(bytes.fromhex(declared['content_hex']), declared['mode'])
    elif identity is not None: errors.append('undeclared observed source identity')
    if observation.filesystem != expected: errors.append('independent complete filesystem')
    return {'passed': not errors, 'errors': errors, 'expected_filesystem': expected,
            'independent_owned_identity': identity, 'identity_raw_hex': execution.identity_raw_hex, 'final_owned_identity': execution.final_identity,
            'observation': dict(h.observation_json(observation), stdout_hex=observation.stdout.hex(), stderr_hex=observation.stderr.hex())}


def assess(row: dict, runtime: Path, filesystem: dict, executions: list[Execution]) -> dict:
    if len(executions) not in (2, 3): raise h.HarnessError('stdin comparison needs GNU2 and optional candidate')
    judgments = [assess_execution(row, runtime, filesystem, execution) for execution in executions]
    # Every full filesystem is independently mandatory. For declared paths the
    # host binds exactly one physical identity before exit; no observed file or
    # output byte is deleted/rewritten merely to compare engine observations.
    repeat = []
    if not judgments[0]['passed'] or not judgments[1]['passed']: repeat.append('independent full model')
    if not row.get('owned_temp_identity'):
        repeat.extend(h.differences(executions[0].observation, executions[1].observation))
    return {'passed': all(item['passed'] for item in judgments) and not repeat,
            'oracle_repeat_differences': repeat, 'executions': judgments}


def run_campaign(selected: list[dict], oracle: Path, candidate: Path | None, compiler: str, root: Path) -> dict:
    inputs = root / 'inputs'; inputs.mkdir(); runtime = root / 'runtime'; runtime.mkdir(); records = []
    for row in selected:
        case, compilation = prepare(row, inputs / row['id'], compiler)
        expected = expected_filesystem(row, case, runtime, oracle)
        binaries = [oracle, oracle] if candidate is None else [oracle, oracle, candidate]
        executions = [execute(row, case, runtime, binary, root / (row['id'] + '-' + str(index) + '-identity.json')) for index, binary in enumerate(binaries)]
        judgment = assess(row, runtime, expected, executions)
        records.append({'id': row['id'], 'model': row, 'compilation': compilation, 'base_expected_filesystem': expected, **judgment})
        print(('PASS ' if judgment['passed'] else 'FAIL ') + row['id'], flush=True)
    return {'classification': 'Original strict stdin-source differential profiles; no ordinary-ledger/safety-exception/full-milestone/performance credit',
            'passed': all(row['passed'] for row in records), 'oracle_only': candidate is None,
            'functional_cpu_affinity': sorted(os.sched_getaffinity(0)),
            'identity_policy': 'Only declared single-temp identities are bound independently before exit and must retain exact final dev/ino/mode/full bytes; otherwise the complete filesystem is static. No output normalization.',
            'totals': {'cases': len(records), 'pass': sum(row['passed'] for row in records), 'fail': sum(not row['passed'] for row in records)}, 'records': records}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--candidate', type=Path); parser.add_argument('--oracle-only', action='store_true')
    parser.add_argument('--report', type=Path, default=ROOT / 'artifacts/stdin-sources/campaign.json')
    parser.add_argument('--case', action='append', default=[]); parser.add_argument('--cc', default='cc')
    args = parser.parse_args(argv)
    if args.oracle_only == (args.candidate is not None): parser.error('supply either --candidate or --oracle-only')
    report = {'passed': False}
    try:
        available = models(); unknown = set(args.case) - {row['id'] for row in available}
        if unknown: raise h.HarnessError('unknown stdin cases: ' + ', '.join(sorted(unknown)))
        selected = [row for row in available if not args.case or row['id'] in args.case]
        compiler = shutil.which(args.cc)
        if compiler is None: raise h.HarnessError('C compiler not found: ' + args.cc)
        reference_oracle.verify(args.oracle)
        identities = {'oracle': h.executable_identity(args.oracle, oracle=True)}
        if args.candidate is not None: identities['candidate'] = h.executable_identity(args.candidate)
        with tempfile.TemporaryDirectory(prefix='rmake-stdin-sources-') as temporary:
            report = run_campaign(selected, Path(identities['oracle']['path']), Path(identities['candidate']['path']) if args.candidate is not None else None, compiler, Path(temporary))
        report.update(identities=identities, compiler=compiler)
        report['source_hashes'] = {str(path.resolve().relative_to(ROOT)): h.sha256_file(path) for path in [Path(__file__), MODELS, ROOT / 'tools/conformance.py', FIXTURES / 'probe.c', FIXTURES / 'legacy_probe.c', FIXTURES / 'launch.py']}
        status = 0 if report['passed'] else 1
    except (h.HarnessError, reference_oracle.OracleError, OSError, ValueError, KeyError) as error:
        report.update(error=str(error), passed=False); print('ERROR ' + str(error), flush=True); status = 2
    args.report.parent.mkdir(parents=True, exist_ok=True); args.report.write_text(json.dumps(report, indent=2) + '\n')
    print('Stdin source gate:', report.get('totals', {'error': True}), flush=True)
    return status


if __name__ == '__main__': raise SystemExit(main())
