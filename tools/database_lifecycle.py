#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Strict original database/source/restart lifecycle differential profiles.

The generic six profiles use ordinary named files. The eight adopted-stdin
profiles are independently selectable; existing database profiles stay separate.
All literal transcript bytes, counters, environment-vector witnesses and full
filesystem results are mandatory. Only independently bounded positioned clocks
and declared failed-exec paths from the predetermined reset workspace vary.
"""
from __future__ import annotations

import argparse
from dataclasses import replace
import hashlib
import json
import os
from pathlib import Path
import tempfile
import time

import conformance as h
import database_output as database
import oracle as reference_oracle

ROOT = Path(__file__).resolve().parents[1]
MODELS = ROOT / 'tests/database-lifecycle/models.json'
ERROR_MODELS = MODELS.with_name('reexec-errors.json')
PIPE_MODELS = MODELS.with_name('pipe-cleanup.json')
PIPE_REFINEMENT = MODELS.with_name('advertised-pair-refinement.json')
EXEC_FAILURES = {'database-native-restart-exec-failure', 'database-owned-stdin-exec-failure',
                 'database-owned-stdin-pipe-exec-failure'}


def models(group: str = 'all') -> list[database.Model]:
    if group not in ('all', 'generic', 'stdin'):
        raise h.HarnessError('unknown database lifecycle group')
    data = []
    for path in (MODELS, ERROR_MODELS, PIPE_MODELS):
        raw = path.read_bytes()
        document = json.loads(raw)
        if document.get('schema_version') != 1:
            raise h.HarnessError('unsupported database lifecycle schema')
        rows = document['models']
        if path == PIPE_MODELS:
            refinement = json.loads(PIPE_REFINEMENT.read_bytes())
            updates = refinement['models']
            if (refinement.get('schema_version') != 1
                    or refinement.get('base_file') != path.relative_to(ROOT).as_posix()
                    or refinement.get('base_sha256') != hashlib.sha256(raw).hexdigest()
                    or len(rows) != 1 or len(updates) != 1):
                raise h.HarnessError('unbound database pipe observer refinement')
            base, update = rows[0], updates[0]
            if ({k: v for k, v in base.items() if k != 'files'}
                    != {k: v for k, v in update.items() if k != 'files'}
                    or base['files'].keys() != update['files'].keys()):
                raise h.HarnessError('database pipe refinement may only change observer inputs')
            rows = updates
        data.extend(rows)
    result = []
    names = set()
    for row in data:
        if row['id'] in names or row['group'] not in ('clock', 'restart', 'stdin'):
            raise h.HarnessError('duplicate lifecycle ID or invalid declared group')
        names.add(row['id'])
        model = database.Model(
            row['id'], {name: bytes.fromhex(value) for name, value in row['files'].items()},
            tuple(row['args']), row['environment'], row['fixed_mtime_ns'], row['expected_status'],
            bytes.fromhex(row['expected_seen_hex']), bytes.fromhex(row['stdout_template_hex']),
            bytes.fromhex(row['stderr_hex']), tuple(row['directory_slots']),
            tuple(bytes.fromhex(value) for value in row['required_fragments_hex']),
            tuple(bytes.fromhex(value) for value in row['forbidden_fragments_hex']),
            tuple(bytes.fromhex(value) for value in row['expected_sections_hex']),
            dump_count=row['dump_count'],
            compile_shared=tuple(tuple(pair) for pair in row['compile_shared']),
            extra_files=tuple((name, bytes.fromhex(value)) for name, value in row['expected_additions_hex'].items()),
            removed_files=tuple(row['expected_removals']))
        database.safe_relative(model.id)
        for name in model.files:
            database.safe_relative(name)
        for name in model.directories:
            database.safe_relative(name, dot=True)
        for source, output in model.compile_shared:
            database.safe_relative(source); database.safe_relative(output)
            if source not in model.files or output in model.files:
                raise h.HarnessError('lifecycle module source/output conflict')
        failure = row.get('kind') == 'exec-failure'
        if failure != (model.id in EXEC_FAILURES):
            raise h.HarnessError('unknown lifecycle execution contract')
        if failure:
            if (model.dump_count != 0 or model.directories or model.stdout or model.expected_status != 127
                    or model.stderr.count(b'@RUNTIME@') != 1
                    or b'@' in model.stderr.replace(b'@RUNTIME@', b'')):
                raise h.HarnessError('invalid finite exec-failure protocol')
        elif not 1 <= model.dump_count <= 2:
            raise h.HarnessError('invalid lifecycle database dump population')
        if len(set(model.directories)) != len(model.directories):
            raise h.HarnessError('invalid lifecycle clock/physical-identity population')
        database.compile_template(model, Path('/predetermined/reset/workspace'))
        if group == 'stdin' and row['group'] != 'stdin':
            continue
        if group == 'generic' and row['group'] == 'stdin':
            continue
        result.append(model)
    if not result:
        raise h.HarnessError('empty database lifecycle inventory')
    return result


def expected_model(model: database.Model, runtime: Path) -> database.Model:
    # Only the two explicit failure models declare this predetermined path slot.
    # No child output is inspected to obtain it, and all surrounding bytes stay
    # literal. Database models retain the existing strict stream machinery.
    return (replace(model, stderr=database.render_paths(model.stderr, runtime))
            if model.id in EXEC_FAILURES else model)


def execute(model: database.Model, case: h.Case, runtime: Path, binary: Path) -> database.Execution:
    if model.id not in EXEC_FAILURES:
        return database.execute(model, case, runtime, binary)
    h.reset_workspace(case, runtime, binary)
    before = time.time(); before_libc = database.libc_time()
    stdout, stderr, status, limit, leftovers = h.capture(
        ['/usr/bin/python3', 'launch.py', str(runtime / 'bin/make'), *model.args],
        runtime / 'work', h.clean_environment(runtime, model.environment), case.timeout)
    after_libc = database.libc_time(); after = time.time()
    filesystem = h.snapshot({name: runtime / name for name in ('work', 'home', 'tmp')})
    return database.Execution(h.Observation(stdout, stderr, status, limit, filesystem, leftovers),
                              (before, after), {}, {}, (before_libc, after_libc))


def run_campaign(selected: list[database.Model], oracle: Path, candidate: Path | None,
                 root: Path) -> dict:
    inputs = root / 'inputs'; inputs.mkdir()
    runtime = root / 'runtime'; runtime.mkdir()
    records = []
    for model in selected:
        compilations = []
        case, filesystem = database.prepare(model, inputs / model.id, runtime, compilations)
        binaries = [oracle, oracle] if candidate is None else [oracle, oracle, candidate]
        executions = [execute(model, case, runtime, binary) for binary in binaries]
        judgment = database.assess(expected_model(model, runtime), runtime, filesystem, executions)
        records.append({'id': model.id, 'args': list(model.args), 'environment': model.environment,
                        'expected_status': model.expected_status, 'stdout_template_hex': model.stdout.hex(),
                        'stderr_template_hex': model.stderr.hex(), 'expected_seen_hex': model.seen.hex(),
                        'directory_slots': list(model.directories), 'dump_count': model.dump_count,
                        'input_files_hex': {name: value.hex() for name, value in model.files.items()},
                        'fixed_mtime_ns': model.fixed_mtime_ns, 'expected_filesystem': filesystem,
                        'compilations': compilations, **judgment})
        print(('PASS ' if judgment['passed'] else 'FAIL ') + model.id, flush=True)
    return {'oracle_only': candidate is None, 'passed': all(row['passed'] for row in records),
            'runtime_path': str(runtime), 'functional_cpu_affinity': sorted(os.sched_getaffinity(0)),
            'limitations': ['Only declared positioned libc clocks and predetermined stderr reset-path slots vary',
                            'All logical counters, vector order, effects and complete filesystems are literal',
                            'Duplicate process-environment names and broader resource/locale profiles remain unqualified'],
            'totals': {'cases': len(records), 'pass': sum(row['passed'] for row in records),
                       'fail': sum(not row['passed'] for row in records)}, 'records': records}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--oracle-only', action='store_true')
    parser.add_argument('--report', type=Path, default=ROOT / 'artifacts/database-lifecycle/campaign.json')
    parser.add_argument('--group', choices=('all', 'generic', 'stdin'), default='all')
    parser.add_argument('--case', action='append', default=[])
    args = parser.parse_args(argv)
    if args.oracle_only == (args.candidate is not None):
        parser.error('supply either --candidate or --oracle-only')
    report = {'passed': False, 'group': args.group}
    try:
        available = models(args.group)
        unknown = set(args.case) - {model.id for model in available}
        if unknown:
            raise h.HarnessError('unknown database lifecycle cases: ' + ', '.join(sorted(unknown)))
        selected = [model for model in available if not args.case or model.id in args.case]
        reference_oracle.verify(args.oracle)
        identities = {'oracle': h.executable_identity(args.oracle, oracle=True)}
        if args.candidate is not None:
            identities['candidate'] = h.executable_identity(args.candidate)
        with tempfile.TemporaryDirectory(prefix='rmake-database-lifecycle-') as temporary:
            report = run_campaign(selected, Path(identities['oracle']['path']),
                                  Path(identities['candidate']['path']) if args.candidate else None,
                                  Path(temporary).resolve())
        report.update(group=args.group, identities=identities,
                      classification='Original strict lifecycle differential profiles; no ordinary-ledger, exception, full milestone or performance credit',
                      source_hashes={name: h.sha256_file(path) for name, path in {
                          'driver': Path(__file__), 'models': MODELS, 'reexec_errors': ERROR_MODELS,
                          'pipe_cleanup': PIPE_MODELS, 'pipe_refinement': PIPE_REFINEMENT,
                          'database_driver': ROOT / 'tools/database_output.py',
                          'conformance': ROOT / 'tools/conformance.py'}.items()})
        status = 0 if report['passed'] else 1
    except (h.HarnessError, reference_oracle.OracleError, OSError, ValueError, KeyError) as error:
        report.update(error=str(error), passed=False)
        print('ERROR ' + str(error), flush=True)
        status = 2
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print('Database lifecycle gate:', report.get('totals', {'error': True}), flush=True)
    return status


if __name__ == '__main__':
    raise SystemExit(main())
