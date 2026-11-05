#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Strict original native floating-point environment GNU2 differential models."""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import shutil
import tempfile
import tomllib
from typing import Any

import conformance as h
import native_modules as native
import oracle as reference_oracle

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'tests/native/fenv_probe.c'
MODELS = ROOT / 'tests/native/fenv.toml'


def models() -> list[native.Model]:
    data = tomllib.loads(MODELS.read_text())
    if data.get('schema') != 1:
        raise h.HarnessError('unsupported native fenv schema')
    result = []
    for row in data['models']:
        if not row['id'].startswith('native-fenv-'):
            raise h.HarnessError('native fenv model needs an exact scoped ID')
        files = {name: bytes.fromhex(value) for name, value in row['files'].items()}
        for name in files:
            native.safe_relative(name)
        result.append(native.Model(
            row['id'], files, row['expected'], bytes.fromhex(row['expected_seen_hex']),
            (native.Module(SOURCE.name, 'fenv.so', tuple(row['defines'])),),
            tuple(row['args']), row['environment'],
        ))
    if len({model.id for model in result}) != len(result):
        raise h.HarnessError('duplicate native fenv model ID')
    return result


def prepare(model: native.Model, directory: Path, compiler: str) -> tuple[h.Case, dict[str, Any]]:
    directory.mkdir()
    for name, value in model.files.items():
        target = directory / native.safe_relative(name)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(value)
    copied = directory / 'module-0.c'
    copied.write_bytes(SOURCE.read_bytes())
    output = directory / 'fenv.so'
    command = [compiler, '-std=c11', '-Wall', '-Wextra', '-Werror', '-fPIC', '-shared',
               *('-D' + value for value in model.modules[0].defines), str(copied), '-o', str(output), '-lm']
    out, err, status, limit, leftovers = h.capture(
        command, directory, {'PATH': '/usr/bin:/bin', 'LC_ALL': 'C', 'LANG': 'C', 'TZ': 'UTC'}, 15)
    compilation = {'command': command, 'source_sha256': h.sha256_file(copied),
                   'stdout_hex': out.hex(), 'stderr_hex': err.hex(), 'exit_code': status,
                   'limit': limit, 'leftover_processes': leftovers}
    if status or limit or leftovers:
        raise h.HarnessError('original native fenv module compilation failed: ' + json.dumps(compilation))
    compilation['module_sha256'] = h.sha256_file(output)
    h.validate_fixture(directory)
    return h.Case(directory, model.id, 'native.fenv', 'required', model.args, 5,
                  model.environment, model.expected), compilation


def expected_filesystem(model: native.Model, case: h.Case, runtime: Path,
                        oracle: Path) -> dict[str, Any]:
    # Predetermine every file from the compiled inputs before any engine runs.
    # The sole declared mutation is the independently authored ordered witness.
    h.reset_workspace(case, runtime, oracle)
    expected = h.snapshot({name: runtime / name for name in ('work', 'home', 'tmp')})
    expected['work/seen'].update(content=base64.b64encode(model.seen).decode(),
                                 size=len(model.seen), sha256=hashlib.sha256(model.seen).hexdigest())
    return expected


def assess(model: native.Model, case: h.Case, expected: dict[str, Any],
           observations: list[h.Observation]) -> dict[str, Any]:
    judgment = native.assess(model, case, observations)
    matches = [observation.filesystem == expected for observation in observations]
    return {**judgment, 'passed': judgment['passed'] and all(matches),
            'expected_filesystem': expected, 'filesystem_matches': matches}


def run_campaign(selected: list[native.Model], oracle: Path, candidate: Path | None,
                 compiler: str, root: Path) -> dict[str, Any]:
    inputs = root / 'inputs'; inputs.mkdir()
    runtime = root / 'runtime'; runtime.mkdir()
    records = []
    for model in selected:
        case, compilation = prepare(model, inputs / model.id, compiler)
        expected = expected_filesystem(model, case, runtime, oracle)
        observations = [h.execute(case, runtime, binary) for binary in
                        ([oracle, oracle] if candidate is None else [oracle, oracle, candidate])]
        judgment = assess(model, case, expected, observations)
        records.append({'id': model.id, 'args': list(model.args), 'environment': model.environment,
                        'expected': model.expected, 'expected_seen_hex': model.seen.hex(),
                        'input_files_hex': {name: value.hex() for name, value in model.files.items()},
                        'defines': list(model.modules[0].defines), 'compilation': compilation, **judgment})
        print(('PASS ' if judgment['passed'] else 'FAIL ') + model.id, flush=True)
    return {'classification': 'Original native fenv profile qualification; no ordinary-ledger or exception credit',
            'oracle_only': candidate is None, 'passed': all(record['passed'] for record in records),
            'functional_cpu_affinity': sorted(os.sched_getaffinity(0)),
            'compiler_contract': 'One original DSO compilation with libm per case, then byte-identical reset GNU2/candidate inputs',
            'totals': {'cases': len(records), 'pass': sum(record['passed'] for record in records),
                       'fail': sum(not record['passed'] for record in records)}, 'records': records}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--oracle-only', action='store_true')
    parser.add_argument('--cc', default='cc')
    parser.add_argument('--report', type=Path, default=ROOT / 'artifacts/native-fenv/campaign.json')
    parser.add_argument('--case', action='append', default=[])
    args = parser.parse_args(argv)
    if args.oracle_only == (args.candidate is not None):
        parser.error('supply either --candidate or --oracle-only')
    report: dict[str, Any] = {'passed': False}
    try:
        available = models()
        unknown = set(args.case) - {model.id for model in available}
        if unknown:
            raise h.HarnessError('unknown native fenv cases: ' + ', '.join(sorted(unknown)))
        selected = [model for model in available if not args.case or model.id in args.case]
        compiler = shutil.which(args.cc)
        if compiler is None:
            raise h.HarnessError('C compiler executable not found: ' + args.cc)
        reference_oracle.verify(args.oracle)
        identities = {'oracle': h.executable_identity(args.oracle, oracle=True)}
        if args.candidate is not None:
            identities['candidate'] = h.executable_identity(args.candidate)
        with tempfile.TemporaryDirectory(prefix='rmake-native-fenv-') as temporary:
            report = run_campaign(selected, Path(identities['oracle']['path']),
                                  Path(identities['candidate']['path']) if args.candidate is not None else None,
                                  compiler, Path(temporary))
        report['identities'] = identities
        report['compiler'] = compiler
        report['source_hashes'] = {name: h.sha256_file(path) for name, path in {
            'driver': Path(__file__), 'fixture': SOURCE, 'models': MODELS,
            'native_assessment': ROOT / 'tools/native_modules.py', 'conformance': ROOT / 'tools/conformance.py',
        }.items()}
        status = 0 if report['passed'] else 1
    except (h.HarnessError, reference_oracle.OracleError, OSError, ValueError, KeyError) as error:
        report.update(error=str(error), passed=False)
        print('ERROR ' + str(error), flush=True)
        status = 2
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print('Native fenv gate:', report.get('totals', {'error': True}), flush=True)
    return status


if __name__ == '__main__':
    raise SystemExit(main())
