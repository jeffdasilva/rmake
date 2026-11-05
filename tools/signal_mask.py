# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Bounded original GNU mask-phase witnesses. No signals are delivered."""
from pathlib import Path
import argparse
import base64
import hashlib
import json
import os
import shutil
import tempfile

import conformance as h
import oracle

REPO = Path(__file__).resolve().parents[1]
ROOT = REPO / 'tests/signal-mask'
MODELS = ROOT / 'models.json'
SOURCE_NAMES = ('mask_bits.h', 'entry.c', 'loader.c', 'module.c', 'launch.py')


def file_record(value):
    raw = value.encode()
    return {'type': 'file', 'mode': 0o644, 'size': len(raw),
            'sha256': hashlib.sha256(raw).hexdigest(), 'content': base64.b64encode(raw).decode()}


def prepare(root, cc):
    for name in SOURCE_NAMES:
        shutil.copyfile(ROOT / name, root / name)
    compilations = []
    for source, output, shared in (('entry.c', 'entry', False), ('loader.c', 'loader.so', True),
                                   ('module.c', 'mask.so', True)):
        command = [cc, '-std=c11', '-Wall', '-Wextra', '-Werror', '-O2']
        if shared:
            command.extend(('-fPIC', '-shared'))
        command.extend(('-o', output, source))
        out, err, status, bound, left = h.capture(command, root, {'PATH': '/usr/bin:/bin', 'LC_ALL': 'C'}, 10)
        if status or bound or left:
            raise h.HarnessError(f'fixture compilation failed: {command!r}, {status}, {err!r}')
        compilations.append({'command': command, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(),
                             'sha256': h.sha256_file(root / output)})
    h.validate_fixture(root)
    return compilations


def run(row, root, runtime, binaries, common):
    inputs = root / row['id']
    shutil.copytree(common, inputs)
    (inputs / 'Makefile').write_text(row['makefile'])
    (inputs / 'seen').write_text('anchor\n')
    args = ['-rR', '--no-print-directory', *row['args']]
    case = h.Case(inputs, row['id'], 'resource.mask-phase', 'required', tuple(args), 6.0, {}, {})
    observations = []
    for binary in binaries:
        h.reset_workspace(case, runtime, binary)
        roots = {key: runtime / key for key in ('work', 'home', 'tmp', 'bin')}
        expected = h.snapshot(roots)
        expected['work/seen'] = file_record(row['seen'])
        expected['bin/make'] = {'type': 'symlink', 'mode': 0o777, 'target': str(binary)}
        for name, value in row.get('created', {}).items():
            expected['work/' + name] = file_record(value)
        command = ['/usr/bin/python3', 'launch.py', row['initial'], str(row.get('nofile', 32)),
                   row.get('sigpipe', 'inherit'), str(roots['bin'] / 'make'), *args]
        out, err, status, bound, left = h.capture(command, roots['work'], h.clean_environment(runtime), 6)
        filesystem = h.snapshot(roots)
        seen = filesystem.get('work/seen', {}).get('content', '')
        errors = [name for name, bad in (
            ('stdout', out != row['stdout'].encode()), ('stderr', err != row['stderr'].encode()),
            ('status', status != row['status']), ('bounds/leftovers', bool(bound or left)),
            ('full filesystem', filesystem != expected),
        ) if bad]
        observations.append({'command': command, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(),
                             'status': status, 'bounds': bound, 'leftovers': left,
                             'filesystem': filesystem, 'expected_filesystem': expected,
                             'seen': base64.b64decode(seen).decode(), 'errors': errors})
    repeated = observations[0] == observations[1]
    gnu2 = repeated and not observations[0]['errors']
    return {'model': row, 'gnu2_qualified': gnu2, 'repeated_exact': repeated,
            'passed': gnu2 and not any(item['errors'] for item in observations),
            'observations': observations}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=REPO / '.tools/oracle/bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--oracle-only', action='store_true')
    parser.add_argument('--report', type=Path, default=REPO / 'artifacts/signal-mask/campaign.json')
    parser.add_argument('--case', action='append', default=[])
    parser.add_argument('--cc', default='cc')
    options = parser.parse_args()
    if options.oracle_only == (options.candidate is not None):
        parser.error('supply either --candidate or --oracle-only')
    reference = options.oracle.resolve()
    profile = oracle.verify(reference)
    binaries = [reference, reference]
    identities = {'oracle': h.executable_identity(reference, oracle=True)}
    if options.candidate is not None:
        candidate = options.candidate.resolve()
        binaries.append(candidate)
        identities['candidate'] = h.executable_identity(candidate)
    data = json.loads(MODELS.read_bytes())
    if data['schema_version'] != 1:
        raise h.HarnessError('unsupported original mask model schema')
    names = {row['id'] for row in data['models']}
    if not names or len(names) != len(data['models']) or set(options.case) - names:
        raise h.HarnessError('empty, unknown, or duplicate signal-mask case')
    selected = [row for row in data['models'] if not options.case or row['id'] in options.case]
    cc = shutil.which(options.cc)
    if cc is None:
        raise h.HarnessError('original signal-mask witnesses require cc')
    previous_umask = os.umask(0o022)
    try:
        with tempfile.TemporaryDirectory(prefix='rmake-inherited-sigmask-') as directory:
            root = Path(directory)
            common = root / 'common'
            common.mkdir()
            compilations = prepare(common, cc)
            runtime = root / 'runtime'
            runtime.mkdir()
            records = [run(row, root, runtime, binaries, common) for row in selected]
    finally:
        os.umask(previous_umask)
    for row in records:
        print(row['model']['id'], 'GNU2', row['gnu2_qualified'], 'match', row['passed'],
              [item['errors'] for item in row['observations']], flush=True)
    totals = {'cases': len(records), 'gnu2': sum(row['gnu2_qualified'] for row in records),
              'pass': sum(row['passed'] for row in records), 'fail': sum(not row['passed'] for row in records)}
    report = {
        'classification': 'Original startup, native, child, and real self-exec mask profiles',
        'passed': not totals['fail'], 'totals': totals, 'identities': identities,
        'oracle_profile': profile['profile_id'], 'compilations': compilations, 'records': records,
        'binaries': {str(binary): h.sha256_file(binary) for binary in binaries},
        'source_hashes': {path.name: h.sha256_file(path) for path in
                          [MODELS, Path(__file__), Path(h.__file__), *[ROOT / name for name in SOURCE_NAMES]]},
        'functional_cpu_affinity': sorted(os.sched_getaffinity(0)),
    }
    options.report.parent.mkdir(parents=True, exist_ok=True)
    with options.report.open('w') as stream:
        json.dump(report, stream, indent=2)
        stream.write('\n')
    print('Signal mask gate', totals, flush=True)
    return int(bool(totals['fail']))


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (h.HarnessError, oracle.OracleError, OSError, KeyError, ValueError) as error:
        raise SystemExit(f'signal mask gate: {error}') from error
