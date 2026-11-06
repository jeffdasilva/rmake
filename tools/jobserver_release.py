# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Exact paired jobserver observations for ordinary release and abort."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import shutil
import tempfile

import conformance as h
import oracle

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'tests/jobserver-release'
MODELS = FIXTURES / 'models.json'


def file_record(text):
    raw = text.encode()
    return {'type': 'file', 'mode': 0o644, 'size': len(raw),
            'sha256': hashlib.sha256(raw).hexdigest(), 'content': base64.b64encode(raw).decode()}


def execute(model, inputs, runtime, binaries, witness):
    fixture = inputs / model['id']
    fixture.mkdir()
    makefile = fixture / 'Makefile'
    makefile.write_text(model['makefile'])
    os.utime(makefile, (1_600_000_000, 1_600_000_000))
    shutil.copyfile(FIXTURES / 'launch.py', fixture / 'launch.py')
    shutil.copy2(witness, fixture / 'witness')
    h.validate_fixture(fixture)
    flags = ('-rR', '--no-print-directory', *model.get('args', ['-k']))
    case = h.Case(fixture, model['id'], 'resource.pool.release', 'required', flags, 7, {}, {})
    observations = []
    for binary in binaries:
        h.reset_workspace(case, runtime, binary)
        roots = {name: runtime / name for name in ('work', 'home', 'tmp', 'bin')}
        expected = h.snapshot(roots)
        files = {name: '' for name in model['created']}
        files.update(model.get('other_files', {}))
        files['observer'] = model['observer']
        for name, value in files.items():
            expected['work/' + name] = file_record(value)
        command = ['/usr/bin/python3', 'launch.py', model['mode'], model['seed'],
                   str(roots['bin'] / 'make'), *flags]
        out, err, status, bound, left = h.capture(command, roots['work'], h.clean_environment(runtime),
                                                 case.timeout)
        filesystem = h.snapshot(roots)
        observer = roots['work'] / 'observer'
        issues = [name for name, different in (
            ('stdout', out != model['stdout'].encode()), ('stderr', err != model['stderr'].encode()),
            ('status', status != model['status']), ('bounds', bool(bound or left)),
            ('filesystem', filesystem != expected),
        ) if different]
        observations.append({'command': command, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(),
                             'status': status, 'bound': bound, 'leftovers': left,
                             'filesystem': filesystem, 'expected_filesystem': expected,
                             'observer_hex': observer.read_bytes().hex() if observer.is_file() else None,
                             'errors': issues})
    repeated = observations[0] == observations[1]
    qualified = repeated and not observations[0]['errors']
    return {'model': model, 'observations': observations, 'gnu2_repeat': repeated,
            'gnu2_qualified': qualified,
            'passed': qualified and all(not observation['errors'] for observation in observations[2:])}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=oracle.PREFIX / 'bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--oracle-only', action='store_true')
    parser.add_argument('--case', action='append', default=[])
    output = parser.add_mutually_exclusive_group(required=True)
    output.add_argument('--report', type=Path)
    output.add_argument('--report-dir', type=Path)
    options = parser.parse_args()
    if options.oracle_only == (options.candidate is not None):
        parser.error('supply either --candidate or --oracle-only')
    reference = options.oracle.resolve(strict=True)
    profile = oracle.verify(reference)
    candidate = options.candidate.resolve(strict=True) if options.candidate else None
    binaries = [reference, reference]
    if candidate is not None:
        binaries.append(candidate)
    if options.report is not None and options.report.exists():
        parser.error('refusing to replace existing evidence')
    models = json.loads(MODELS.read_bytes())['models']
    if options.case:
        if unknown := set(options.case) - {row['id'] for row in models}:
            parser.error(f'unknown case: {", ".join(sorted(unknown))}')
        models = [row for row in models if row['id'] in options.case]
    previous_umask = os.umask(0o022)
    try:
        with tempfile.TemporaryDirectory(prefix='rmake-token-release-') as directory:
            root = Path(directory)
            inputs, runtime = root / 'inputs', root / 'runtime'
            inputs.mkdir()
            runtime.mkdir()
            witness = inputs / 'witness'
            cc = ['cc', '-std=c11', '-Wall', '-Wextra', '-Werror', '-O2', '-o', str(witness),
                  str(FIXTURES / 'witness.c')]
            out, err, status, bound, left = h.capture(cc, inputs, {'PATH': '/usr/bin:/bin', 'LC_ALL': 'C'}, 10)
            if status or bound or left:
                raise h.HarnessError(f'witness compilation failed: {status}, {err!r}, {bound}, {left}')
            build = {'command': cc, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(),
                     'witness_sha256': h.sha256_file(witness)}
            rows = [execute(row, inputs, runtime, binaries, witness)
                    for row in models]
    finally:
        os.umask(previous_umask)
    inputs = [MODELS, FIXTURES / 'witness.c', FIXTURES / 'launch.py', Path(__file__).resolve()]
    report = {'classification': 'Ordinary release and abort. Separate from parent fatal recovery.',
              'oracle_profile': profile['profile_id'], 'oracle_sha256': h.sha256_file(reference),
              'candidate_sha256': h.sha256_file(candidate) if candidate is not None else None,
              'functional_cpu_affinity': sorted(os.sched_getaffinity(0)),
              'source_hashes': {str(path.relative_to(ROOT)): h.sha256_file(path) for path in inputs},
              'build': build, 'results': rows}
    report_path = options.report
    if report_path is None:
        options.report_dir.mkdir(parents=True, exist_ok=True)
        report_path = Path(tempfile.mkdtemp(prefix='run-', dir=options.report_dir)) / 'report.json'
    report_path.parent.mkdir(parents=True, exist_ok=True)
    with report_path.open('x') as stream:
        json.dump(report, stream, indent=2)
        stream.write('\n')
    for row in rows:
        print(row['model']['id'], row['gnu2_qualified'], row['passed'],
              [observation['errors'] for observation in row['observations']])
    print(f'Jobserver release gate: {sum(row["passed"] for row in rows)}/{len(rows)} ({report_path})')
    return int(any(not row['passed'] for row in rows))


if __name__ == '__main__':
    raise SystemExit(main())
