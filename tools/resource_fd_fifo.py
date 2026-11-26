# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original FIFO resource observations with independent physical witnesses."""
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
FIXTURES = ROOT / 'tests/resource-fd-fifo'
MODELS = FIXTURES / 'models.json'


def file_record(raw):
    return {'type': 'file', 'mode': 0o644, 'size': len(raw),
            'sha256': hashlib.sha256(raw).hexdigest(), 'content': base64.b64encode(raw).decode()}


def compile_witness(command, root, output):
    out, err, status, bound, left = h.capture(command, root, {'PATH': '/usr/bin:/bin', 'LC_ALL': 'C'}, 10)
    if status or bound or left:
        raise h.HarnessError(f'FIFO witness compilation failed: {status}, {err!r}')
    return {'command': command, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(),
            'sha256': h.sha256_file(output)}


def execute(model, inputs, runtime, binaries, wire, errno_module, errno_catalog, locale):
    fixture = inputs / model['id']
    fixture.mkdir()
    for name, content in model['files'].items():
        (fixture / name).write_text(content)
    shutil.copyfile(FIXTURES / 'launch.py', fixture / 'launch.py')
    if model.get('wire'):
        shutil.copy2(wire, fixture / 'wire')
    environment = {}
    if errno_module is not None:
        shutil.copy2(errno_module, fixture / 'include_errno.so')
        catalog = fixture / 'errno-catalog/fr/LC_MESSAGES/libc.mo'
        catalog.parent.mkdir(parents=True)
        catalog.write_bytes(errno_catalog)
        environment.update(locale['environment'], LC_ALL='C.UTF-8', LANG='C.UTF-8')
    expected_stderr = bytes.fromhex(model['stderr_hex']) if 'stderr_hex' in model else model['stderr'].encode()
    h.validate_fixture(fixture)
    case = h.Case(fixture, model['id'], 'resource.fifo', 'required', tuple(model['args']), 7, {}, {})
    observations = []
    for binary in binaries:
        h.reset_workspace(case, runtime, binary)
        roots = {name: runtime / name for name in ('work', 'home', 'tmp', 'bin')}
        expected = h.snapshot(roots)
        command = ['/usr/bin/python3', 'launch.py', model['mode'], str(model['limit']),
                   model.get('seed', ''), str(roots['bin'] / 'make'), *case.args]
        out, err, status, bound, left = h.capture(command, roots['work'],
                                                 h.clean_environment(runtime, environment), case.timeout)
        filesystem = h.snapshot(roots)
        observer = roots['work'] / 'observer'
        raw = observer.read_bytes() if observer.is_file() else b''
        if model['mode'] == 'borrowed':
            expected['tmp/borrowed.fifo'] = {'type': 'fifo', 'mode': 0o600}
        for name, content in dict(model['created'], observer=model['observer']).items():
            expected['work/' + name] = file_record(content.encode())
        if 'seen_hex' in model:
            expected['work/seen'] = file_record(bytes.fromhex(model['seen_hex']))
        errors = [name for name, different in (
            ('stdout', out != model['stdout'].encode()), ('stderr', err != expected_stderr),
            ('status', status != model['status']), ('bounds', bool(bound or left)),
            ('complete filesystem', filesystem != expected),
        ) if different]
        observations.append({'command': command, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(),
                             'status': status, 'bound': bound, 'leftovers': left,
                             'filesystem': filesystem, 'expected_filesystem': expected,
                             'observer_hex': raw.hex(), 'errors': errors})
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
    parser.add_argument('--locale-root', type=Path, default=ROOT / '.tools/locale-data')
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    if args.oracle_only == (args.candidate is not None):
        parser.error('supply either --candidate or --oracle-only')
    reference = args.oracle.resolve(strict=True)
    profile = oracle.verify(reference)
    candidate = args.candidate.resolve(strict=True) if args.candidate is not None else None
    binaries = [reference, reference]
    if candidate is not None:
        binaries.append(candidate)
    data = json.loads(MODELS.read_bytes())
    models = data['models']
    if args.case:
        if unknown := set(args.case) - {row['id'] for row in models}:
            parser.error(f'unknown case: {", ".join(sorted(unknown))}')
        models = [row for row in models if row['id'] in args.case]
    encodings = {row['errno_locale'] for row in models if 'errno_locale' in row}
    locale = None
    errno_catalog = b''
    errno_builds = {}
    fixture_source = ROOT / 'tests/nls/include_errno.c'
    if encodings:
        import nls_diagnostics

        if not encodings <= {'latin1', 'utf8'}:
            raise h.HarnessError('unsupported live errno encoding')
        key = json.loads((nls_diagnostics.DATA / 'models.json').read_bytes())['reference_catalog_sha256']
        _, _, _, locale = nls_diagnostics.prepare_reference(reference, key, args.locale_root)
        errno_catalog = bytes.fromhex(data['errno_catalog_hex'])
        pins = json.loads((nls_diagnostics.DATA / 'include.json').read_bytes())['fixture_sha256']
        if h.sha256_file(fixture_source) != pins[fixture_source.name]:
            raise h.HarnessError('FIFO live errno fixture identity differs')
    previous = os.umask(0o022)
    try:
        with tempfile.TemporaryDirectory(prefix='rmake-fifo-resource-') as temp:
            root = Path(temp)
            inputs, runtime = root / 'inputs', root / 'runtime'
            inputs.mkdir()
            runtime.mkdir()
            wire = inputs / 'wire'
            cc = ['cc', '-std=c11', '-Wall', '-Wextra', '-Werror', '-O2', '-o', str(wire),
                  str(FIXTURES / 'wire.c')]
            build = compile_witness(cc, inputs, wire)
            errno_modules = {}
            for encoding in sorted(encodings):
                module = inputs / (encoding + '.so')
                defines = ['-DNLS_ERRNO_EMFILE']
                if encoding == 'latin1':
                    defines.append('-DNLS_ERRNO_LATIN1')
                cc = ['cc', '-std=c11', '-Wall', '-Wextra', '-Werror', '-shared', '-fPIC', *defines,
                      '-o', str(module), str(fixture_source)]
                errno_builds[encoding] = compile_witness(cc, inputs, module)
                errno_modules[encoding] = module
            rows = [execute(row, inputs, runtime, binaries, wire,
                            errno_modules.get(row.get('errno_locale')), errno_catalog, locale) for row in models]
    finally:
        os.umask(previous)
    report = {'oracle_profile': profile['profile_id'], 'oracle_sha256': h.sha256_file(reference),
              'candidate_sha256': h.sha256_file(candidate) if candidate is not None else None,
              'functional_cpu_affinity': sorted(os.sched_getaffinity(0)), 'build': build,
              'source_hashes': {str(path.relative_to(ROOT)): h.sha256_file(path) for path in
                                (Path(__file__).resolve(), MODELS, FIXTURES / 'launch.py', FIXTURES / 'wire.c')},
              'results': rows}
    if locale is not None:
        report.update(locale_profile=locale, errno_builds=errno_builds)
        report['source_hashes'][str(fixture_source.relative_to(ROOT))] = h.sha256_file(fixture_source)
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
