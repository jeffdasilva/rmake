# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Exact process-replacement control for the incoming jobserver pipe."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import tempfile

import conformance as harness
import database_output as fixtures
import oracle

ROOT = Path(__file__).resolve().parents[1]
MODEL = ROOT / 'tests/restart-pipe/model.json'
CASE = 'restart-inherited-pipe-success'


def read_model(path: Path) -> fixtures.Model:
    row = json.loads(path.read_bytes())
    if row['schema_version'] != 1:
        raise harness.HarnessError('unsupported inherited-pipe model schema')
    return fixtures.Model(
        CASE, {name: bytes.fromhex(value) for name, value in row['files_hex'].items()},
        tuple(row['args']), row['environment'], row['fixed_mtime_ns'],
        row['expected_status'], bytes.fromhex(row['expected_seen_hex']),
        bytes.fromhex(row['expected_stdout_hex']), bytes.fromhex(row['expected_stderr_hex']),
        (), (), (), (), compile_shared=tuple(map(tuple, row['compile_shared'])),
        extra_files=tuple((name, bytes.fromhex(value))
                          for name, value in row['expected_additions_hex'].items()),
        removed_files=tuple(row['expected_removals']))


def run(model: fixtures.Model, binaries: list[Path]) -> dict:
    compilations, observations = [], []
    previous_umask = os.umask(0o022)
    try:
        with tempfile.TemporaryDirectory(prefix='rmake-restart-pipe-') as temporary:
            root = Path(temporary)
            runtime = root / 'runtime'
            runtime.mkdir()
            case, expected_filesystem = fixtures.prepare(model, root / 'inputs', runtime, compilations)
            expected_stderr = fixtures.render_paths(model.stderr, runtime)
            for binary in binaries:
                harness.reset_workspace(case, runtime, binary)
                roots = {name: runtime / name for name in ('work', 'home', 'tmp', 'bin')}
                expected = dict(expected_filesystem, bin={'mode': 0o755, 'type': 'directory'})
                expected['bin/make'] = {'mode': 0o777, 'type': 'symlink', 'target': str(binary)}
                stdout, stderr, status, limit, leftovers = harness.capture(
                    ['/usr/bin/python3', 'launch.py', str(runtime / 'bin/make'), *model.args],
                    runtime / 'work', harness.clean_environment(runtime, model.environment),
                    case.timeout)
                filesystem = harness.snapshot(roots)
                errors = [name for name, bad in (
                    ('stdout', stdout != model.stdout),
                    ('stderr', stderr != expected_stderr),
                    ('status', status != model.expected_status),
                    ('bounds', bool(limit or leftovers)),
                    ('complete filesystem and ordered witness', filesystem != expected),
                ) if bad]
                observations.append({
                    'stdout_hex': stdout.hex(), 'stderr_hex': stderr.hex(), 'status': status,
                    'limit': limit, 'leftover_processes': leftovers, 'filesystem': filesystem,
                    'expected_filesystem': expected, 'errors': errors,
                })
    finally:
        os.umask(previous_umask)
    gnu2 = observations[0] == observations[1] and not observations[0]['errors']
    return {
        'id': model.id, 'expected_seen_hex': model.seen.hex(), 'compilations': compilations,
        'observations': observations, 'gnu2_qualified': gnu2,
        'passed': gnu2 and all(not value['errors'] for value in observations),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--oracle-only', action='store_true')
    parser.add_argument('--report', type=Path, default=ROOT / 'artifacts/restart-pipe/campaign.json')
    parser.add_argument('--case', choices=[CASE], action='append')
    options = parser.parse_args()
    if options.oracle_only == (options.candidate is not None):
        parser.error('supply either --candidate or --oracle-only')
    report = {'passed': False}
    try:
        reference = options.oracle.resolve()
        oracle.verify(reference)
        identities = {'oracle': harness.executable_identity(reference, oracle=True)}
        binaries = [reference, reference]
        if options.candidate is not None:
            candidate = options.candidate.resolve()
            identities['candidate'] = harness.executable_identity(candidate)
            binaries.append(candidate)
        record = run(read_model(MODEL), binaries)
        report = {
            'classification': 'Original exact inherited-pipe control; no ordinary-ledger or performance credit',
            'passed': record['passed'], 'oracle_only': options.oracle_only,
            'identities': identities, 'functional_cpu_affinity': sorted(os.sched_getaffinity(0)),
            'source_hashes': {name: harness.sha256_file(path) for name, path in (
                ('model', MODEL), ('driver', Path(__file__)),
                ('fixture_helper', Path(fixtures.__file__)),
                ('conformance', Path(harness.__file__)))},
            'totals': {'cases': 1, 'gnu2': int(record['gnu2_qualified']),
                       'pass': int(record['passed']), 'fail': int(not record['passed'])},
            'records': [record],
        }
        status = 0 if report['passed'] else 1
    except (harness.HarnessError, oracle.OracleError, OSError, ValueError, TypeError, KeyError) as error:
        report.update(error=str(error))
        status = 2
        print(f'ERROR {error}', flush=True)
    options.report.parent.mkdir(parents=True, exist_ok=True)
    options.report.write_text(json.dumps(report, indent=2) + '\n')
    print('Inherited pipe restart gate:', report.get('totals', {'error': True}), flush=True)
    return status


if __name__ == '__main__':
    raise SystemExit(main())
