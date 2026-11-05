# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Exact GNU comparisons for output-sync spools at real descriptor limits."""
import argparse
import base64
import hashlib
import json
import os
import shutil
from pathlib import Path
import tempfile

import conformance as h
import oracle

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/resource-fd-spool"
MODELS = FIXTURES / "models.json"


def file_record(value):
    raw = value.encode()
    return {'type': 'file', 'mode': 0o644, 'size': len(raw),
            'sha256': hashlib.sha256(raw).hexdigest(), 'content': base64.b64encode(raw).decode()}


def run(row, root, runtime, binaries, compiler):
    path = root / row['id']
    path.mkdir()
    for name, value in row['files'].items():
        (path / name).write_text(value)
    compilation = None
    if row.get('compile'):
        command = [compiler, '-std=c11', '-Wall', '-Wextra', '-Werror', '-O2',
                   '-o', str(path / 'stream-id'), str(FIXTURES / 'stream_id.c')]
        out, err, status, bound, left = h.capture(
            command, path, {'PATH': '/usr/bin:/bin', 'LC_ALL': 'C'}, 10)
        if status or bound or left:
            raise h.HarnessError(f'original spool witness compilation failed: {status}, {err!r}')
        compilation = {'command': command, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(),
                       'binary_sha256': h.sha256_file(path / 'stream-id')}
    h.validate_fixture(path)
    case = h.Case(path, row['id'], 'resource.pool', 'required', tuple(row['args']), 6, {}, {})
    observations = []
    for binary in binaries:
        h.reset_workspace(case, runtime, binary)
        roots = {name: runtime / name for name in ('work', 'home', 'tmp', 'bin')}
        expected = h.snapshot(roots)
        expected['bin/make'] = {'type': 'symlink', 'mode': 0o777, 'target': str(binary)}
        for name, value in row['created'].items():
            expected['work/' + name] = file_record(value)
        nofile = row['nofile']
        command = ['/usr/bin/prlimit', f'--nofile={nofile}:{nofile}', '--',
                   str(roots['bin'] / 'make'), *case.args]
        out, err, status, bound, left = h.capture(
            command, roots['work'], h.clean_environment(runtime), case.timeout,
            merged=row.get('merged', False))
        filesystem = h.snapshot(roots)
        errors = [key for key, bad in (
            ('stdout', out != row['stdout'].encode()), ('stderr', err != row['stderr'].encode()),
            ('status', status != row['status']), ('process bounds', bool(bound or left)),
            ('complete filesystem', filesystem != expected),
        ) if bad]
        observations.append({'command': command, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(),
                             'status': status, 'bound': bound, 'leftovers': left,
                             'filesystem': filesystem, 'expected_filesystem': expected, 'errors': errors})
    gnu2 = observations[0] == observations[1] and not observations[0]['errors']
    return {'model': row, 'gnu2_qualified': gnu2,
            'passed': gnu2 and all(not item['errors'] for item in observations),
            'observations': observations, 'compilation': compilation}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--oracle-only', action='store_true')
    parser.add_argument('--report', type=Path, default=ROOT / 'artifacts/resource-fd-spool/campaign.json')
    parser.add_argument('--case', action='append', default=[])
    parser.add_argument('--cc', default='cc')
    options = parser.parse_args()
    if options.oracle_only == (options.candidate is not None):
        parser.error('supply either --candidate or --oracle-only')
    reference = options.oracle.resolve()
    profile = oracle.verify(reference)
    binaries = [reference, reference]
    if options.candidate is not None:
        binaries.append(options.candidate.resolve())
    data = json.loads(MODELS.read_bytes())
    if data['schema_version'] != 1:
        raise h.HarnessError('unsupported original descriptor spool model schema')
    models = [row for row in data['models'] if not options.case or row['id'] in options.case]
    if not models or set(options.case) - {row['id'] for row in models}:
        raise h.HarnessError('unknown or empty finite pool case selection')
    compiler = shutil.which(options.cc)
    if compiler is None and any(row.get('compile') for row in models):
        raise h.HarnessError('selected spool witnesses require a C compiler')
    previous_umask = os.umask(0o022)
    try:
        with tempfile.TemporaryDirectory(prefix='rmake-low-fd-pools-') as temporary:
            root = Path(temporary)
            runtime = root / 'runtime'
            runtime.mkdir()
            records = [run(row, root, runtime, binaries, compiler) for row in models]
    finally:
        os.umask(previous_umask)
    for row in records:
        print(row['model']['id'], 'GNU2', row['gnu2_qualified'], 'match', row['passed'],
              [item['errors'] for item in row['observations']], flush=True)
    totals = {'cases': len(records), 'gnu2': sum(row['gnu2_qualified'] for row in records),
              'pass': sum(row['passed'] for row in records), 'fail': sum(not row['passed'] for row in records)}
    report = {'oracle_profile': profile['profile_id'], 'records': records, 'totals': totals,
              'passed': not totals['fail'], 'model_sha256': h.sha256_file(MODELS),
              'driver_sha256': h.sha256_file(Path(__file__)),
              'witness_sha256': h.sha256_file(FIXTURES / 'stream_id.c'),
              'functional_cpu_affinity': sorted(os.sched_getaffinity(0)),
              'binaries': {str(path): h.sha256_file(path) for path in binaries}}
    options.report.parent.mkdir(parents=True, exist_ok=True)
    with options.report.open('w') as stream:
        json.dump(report, stream, indent=2)
        stream.write('\n')
    print('Low descriptor spool gate', totals, flush=True)
    return int(bool(totals['fail']))


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (h.HarnessError, oracle.OracleError, OSError, KeyError, ValueError) as error:
        raise SystemExit(f'low descriptor spool gate: {error}') from error
