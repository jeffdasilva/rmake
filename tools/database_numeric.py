#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Strict original native/database rounding and registration-cache profiles.

Literal full database transcripts retain every numeric spelling and logical
counter. Only positioned libc clocks and declared physical identities vary.
The original public-ABI module is compiled once per case before GNU2/candidate
executions share identical reset inputs. No default-ledger credit is implied.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import tempfile
from typing import Any

import conformance as h
import database_output as database
from database_expected import Preparation
import native_fenv as fenv
import native_modules as native
import oracle as reference_oracle

ROOT = Path(__file__).resolve().parents[1]
MODELS = ROOT / 'tests/database-output/numeric.json'


def models() -> list[database.Model]:
    data = json.loads(MODELS.read_bytes())
    if data.get('schema_version') != 1:
        raise h.HarnessError('unsupported database numeric schema')
    if h.sha256_file(fenv.SOURCE) != data['fixture_sha256']:
        raise h.HarnessError('original numeric module source identity changed')
    result = []
    for row in data['models']:
        if not row['id'].startswith('database-native-numeric-'):
            raise h.HarnessError('numeric model needs an exact scoped ID')
        model = database.Model(
            row['id'], {name: bytes.fromhex(value) for name, value in row['files'].items()},
            tuple(row['args']), row['environment'], row['fixed_mtime_ns'], row['expected_status'],
            bytes.fromhex(row['expected_seen_hex']), bytes.fromhex(row['stdout_template_hex']),
            bytes.fromhex(row['stderr_hex']), tuple(row['directory_slots']),
            tuple(bytes.fromhex(value) for value in row['required_fragments_hex']),
            tuple(bytes.fromhex(value) for value in row['forbidden_fragments_hex']),
            tuple(bytes.fromhex(value) for value in row['expected_sections_hex']),
            dump_count=row['dump_count'])
        database.safe_relative(model.id)
        for name in model.files:
            database.safe_relative(name)
        for name in model.directories:
            database.safe_relative(name, dot=True)
        if model.dump_count != 1 or len(set(model.directories)) != len(model.directories):
            raise h.HarnessError('numeric profiles require one dump and unique directory slots')
        database.compile_template(model, Path('/predetermined/reset/workspace'))
        result.append(model)
    if not result or len({model.id for model in result}) != len(result):
        raise h.HarnessError('empty or duplicate numeric model inventory')
    return result


def fixture(model: database.Model) -> native.Model:
    return native.Model(model.id, model.files,
                        {'exit_code': model.expected_status, 'stdout_hex': '', 'stderr_hex': ''},
                        model.seen, (native.Module(fenv.SOURCE.name, 'fenv.so'),),
                        model.args, model.environment)


def prepare(model: database.Model, directory: Path, runtime: Path,
            oracle: Path, compiler: str) -> tuple[h.Case, dict[str, Any], dict[str, Any]]:
    original = fixture(model)
    case, compilation = fenv.prepare(original, directory, compiler)
    for path in case.path.rglob('*'):
        if path.is_file():
            os.utime(path, ns=(model.fixed_mtime_ns, model.fixed_mtime_ns))
    filesystem = fenv.expected_filesystem(original, case, runtime, oracle)
    return case, compilation, filesystem


def run_campaign(selected: list[database.Model], oracle: Path, candidate: Path | None,
                 compiler: str, root: Path, *, expected: Preparation | None = None) -> dict[str, Any]:
    inputs = root / 'inputs'; inputs.mkdir()
    runtime = root / 'runtime'; runtime.mkdir()
    records = []
    for model in selected:
        case, compilation, filesystem = prepare(model, inputs / model.id, runtime, oracle, compiler)
        binaries = [oracle, oracle] if candidate is None else [oracle, oracle, candidate]
        executions = [database.execute(model, case, runtime, binary, expected=expected) for binary in binaries]
        judgment = database.assess(model, runtime, filesystem, executions)
        records.append({'id': model.id, 'args': list(model.args), 'environment': model.environment,
                        'expected_status': model.expected_status, 'stdout_template_hex': model.stdout.hex(),
                        'stderr_hex': model.stderr.hex(), 'expected_seen_hex': model.seen.hex(),
                        'input_files_hex': {name: value.hex() for name, value in model.files.items()},
                        'directory_slots': list(model.directories), 'fixed_mtime_ns': model.fixed_mtime_ns,
                        'expected_filesystem': filesystem, 'compilation': compilation, **judgment})
        print(('PASS ' if judgment['passed'] else 'FAIL ') + model.id, flush=True)
    return {'classification': 'Original full native/database numeric protocol qualification; no ordinary-ledger, exception, full milestone or performance credit',
            'oracle_only': candidate is None, 'passed': all(row['passed'] for row in records),
            'runtime_path': str(runtime), 'functional_cpu_affinity': sorted(os.sched_getaffinity(0)),
            'compiler_contract': 'One original public-ABI DSO compilation with explicit libm per case, before identical reset GNU2/candidate inputs',
            'limitations': ['Only pinned C locale, UTC, Linux/x86-64 configuration is qualified here',
                            'Locale-aware formatting is exercised through libc, but non-C numeric locales remain a separate acceptance surface',
                            'Full literal transcripts were derived after retained raw observations; original independent rounding, occupancy and effect assertions are mandatory',
                            'Only declared positioned libc-clock and independently captured physical-identity relations vary'],
            'totals': {'cases': len(records), 'pass': sum(row['passed'] for row in records),
                       'fail': sum(not row['passed'] for row in records)}, 'records': records}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--oracle-only', action='store_true')
    parser.add_argument('--cc', default='cc')
    parser.add_argument('--report', type=Path, default=ROOT / 'artifacts/database-output/numeric.json')
    parser.add_argument('--case', action='append', default=[])
    args = parser.parse_args(argv)
    if args.oracle_only == (args.candidate is not None):
        parser.error('supply either --candidate or --oracle-only')
    report: dict[str, Any] = {'passed': False}
    try:
        available = models()
        unknown = set(args.case) - {model.id for model in available}
        if unknown:
            raise h.HarnessError('unknown database numeric cases: ' + ', '.join(sorted(unknown)))
        selected = [model for model in available if not args.case or model.id in args.case]
        compiler = shutil.which(args.cc)
        if compiler is None:
            raise h.HarnessError('C compiler executable not found: ' + args.cc)
        profile = reference_oracle.verify(args.oracle)
        identities = {'oracle': h.executable_identity(args.oracle, oracle=True)}
        if args.candidate is not None:
            identities['candidate'] = h.executable_identity(args.candidate)
        with tempfile.TemporaryDirectory(prefix='rmake-database-numeric-') as temporary:
            report = run_campaign(selected, Path(identities['oracle']['path']),
                                  Path(identities['candidate']['path']) if args.candidate is not None else None,
                                  compiler, Path(temporary),
                                  expected=database.expected_preparation(profile, Path(temporary)))
        report['identities'] = identities
        report['compiler'] = compiler
        report['source_hashes'] = {name: h.sha256_file(path) for name, path in {
            'driver': Path(__file__), 'models': MODELS, 'fixture': fenv.SOURCE,
            'database_driver': ROOT / 'tools/database_output.py',
            'native_fenv_driver': ROOT / 'tools/native_fenv.py',
            'native_models': ROOT / 'tools/native_modules.py',
            'conformance': ROOT / 'tools/conformance.py'}.items()}
        report['source_hashes'].update(database.expected_source_hashes())
        status = 0 if report['passed'] else 1
    except (h.HarnessError, reference_oracle.OracleError, OSError, ValueError, KeyError) as error:
        report.update(error=str(error), passed=False)
        print('ERROR ' + str(error), flush=True)
        status = 2
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print('Database numeric gate:', report.get('totals', {'error': True}), flush=True)
    return status


if __name__ == '__main__':
    raise SystemExit(main())
