# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare GNU Make's stdout write, flush, and close failures."""
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
import nls_diagnostics

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'tests/stdout-file-size'
MODELS = FIXTURES / 'models.json'


def file_record(value, mode=0o644):
    raw = value.encode()
    return {'type': 'file', 'mode': mode, 'size': len(raw),
            'sha256': hashlib.sha256(raw).hexdigest(), 'content': base64.b64encode(raw).decode()}


def execute(model, inputs, runtime, binaries, emit, native, locale):
    fixture = inputs / model['id']
    fixture.mkdir()
    for name, contents in model['files'].items():
        (fixture / name).write_text(contents)
    shutil.copyfile(FIXTURES / 'launch.py', fixture / 'launch.py')
    if model.get('emit'):
        shutil.copy2(emit, fixture / 'emit')
    if model.get('native'):
        shutil.copy2(native, fixture / 'native_probe.so')
    h.validate_fixture(fixture)
    case = h.Case(fixture, model['id'], 'resource.stdout', 'required', tuple(model['args']), 5,
                  model.get('env', {}), {})
    observations = []
    for binary in binaries:
        h.reset_workspace(case, runtime, binary)
        roots = {name: runtime / name for name in ('work', 'home', 'tmp', 'bin')}
        expected = h.snapshot(roots)
        command = ['/usr/bin/python3', 'launch.py', model['mode'], str(model['limit']),
                   str(roots['bin'] / 'make'), *case.args]
        environment = h.clean_environment(runtime, case.environment)
        if model.get('locale') == 'fr':
            environment.update(locale['environment'], LC_ALL=locale['locale'])
        out, err, status, bound, left = h.capture(command, roots['work'], environment, case.timeout)
        filesystem = h.snapshot(roots)
        for name, contents in model['created'].items():
            expected['work/' + name] = file_record(contents)
        errors = [name for name, different in (
            ('stdout', out != model['stdout'].encode()), ('stderr', err != model['stderr'].encode()),
            ('status', status != model['status']), ('bounds', bool(bound or left)),
            ('complete filesystem', filesystem != expected),
        ) if different]
        observations.append({'command': command, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(),
                             'status': status, 'bound': bound, 'leftovers': left, 'filesystem': filesystem,
                             'expected_filesystem': expected, 'errors': errors})
    repeated = observations[0] == observations[1]
    qualified = repeated and not observations[0]['errors']
    return {'model': model, 'observations': observations, 'gnu2_qualified': qualified,
            'passed': qualified and all(not item['errors'] for item in observations[2:])}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=oracle.PREFIX / 'bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--oracle-only', action='store_true')
    parser.add_argument('--case', action='append', default=[])
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--locale-root', type=Path, default=ROOT / '.tools/locale-data')
    args = parser.parse_args()
    if args.oracle_only == (args.candidate is not None):
        parser.error('supply either --candidate or --oracle-only')
    reference = args.oracle.resolve(strict=True)
    candidate = args.candidate.resolve(strict=True) if args.candidate is not None else None
    binaries = [reference, reference]
    if candidate is not None:
        binaries.append(candidate)
    profile = oracle.verify(reference)
    models = json.loads(MODELS.read_bytes())['models']
    if args.case:
        if unknown := set(args.case) - {row['id'] for row in models}:
            parser.error(f'unknown case: {", ".join(sorted(unknown))}')
        models = [row for row in models if row['id'] in args.case]
    locale = None
    if any(row.get('locale') == 'fr' for row in models):
        key = json.loads((nls_diagnostics.DATA / 'models.json').read_bytes())['reference_catalog_sha256']
        _, _, _, locale = nls_diagnostics.prepare_reference(reference, key, args.locale_root)
    previous = os.umask(0o022)
    try:
        with tempfile.TemporaryDirectory(prefix='rmake-file-size-') as temp:
            root = Path(temp)
            inputs, runtime = root / 'inputs', root / 'runtime'
            inputs.mkdir()
            runtime.mkdir()
            emit = inputs / 'emit'
            native = inputs / 'native_probe.so'
            build = []
            for output, flags, src in ((emit, [], 'emit.c'), (native, ['-shared', '-fPIC'], 'native.c')):
                cc = ['cc', '-std=c11', '-Wall', '-Wextra', '-Werror', '-O2', *flags,
                      '-o', str(output), str(FIXTURES / src)]
                out, err, status, bound, left = h.capture(
                    cc, inputs, {'PATH': '/usr/bin:/bin', 'LC_ALL': 'C'}, 10)
                if status or bound or left:
                    raise h.HarnessError(f'output witness compilation failed: {status}, {err!r}')
                build.append({'command': cc, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(),
                              'sha256': h.sha256_file(output)})
            rows = [execute(row, inputs, runtime, binaries, emit, native, locale)
                    for row in models]
    finally:
        os.umask(previous)
    report = {'oracle_profile': profile['profile_id'], 'oracle_sha256': h.sha256_file(reference),
              'candidate_sha256': h.sha256_file(candidate) if candidate is not None else None,
              'functional_cpu_affinity': sorted(os.sched_getaffinity(0)), 'build': build,
              'source_hashes': {str(path.relative_to(ROOT)): h.sha256_file(path) for path in
                                (Path(__file__).resolve(), MODELS, FIXTURES / 'launch.py',
                                 FIXTURES / 'emit.c', FIXTURES / 'native.c')},
              'locale_profile': locale, 'results': rows}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    with args.report.open('w') as stream:
        json.dump(report, stream, indent=2)
        stream.write('\n')
    for row in rows:
        print(row['model']['id'], row['gnu2_qualified'], row['passed'],
              [item['errors'] for item in row['observations']])
    return int(any(not row['passed'] for row in rows))


if __name__ == '__main__':
    raise SystemExit(main())
