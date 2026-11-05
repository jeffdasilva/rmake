# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare strict capped-write observations without output-derived identities."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import tempfile

import conformance as h
import oracle

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'tests/resource-file-size'
MODELS = FIXTURES / 'models.json'


def file_record(value, mode=0o644):
    raw = value.encode()
    return {'type': 'file', 'mode': mode, 'size': len(raw),
            'sha256': hashlib.sha256(raw).hexdigest(), 'content': base64.b64encode(raw).decode()}


def execute(model, inputs, runtime, binaries):
    fixture = inputs / model['id']
    fixture.mkdir()
    for name, contents in model['files'].items():
        (fixture / name).write_text(contents)
    shutil.copyfile(FIXTURES / 'launch.py', fixture / 'launch.py')
    h.validate_fixture(fixture)
    case = h.Case(fixture, model['id'], 'resource.file-size', 'required', tuple(model['args']), 5, {}, {})
    observations = []
    for binary in binaries:
        h.reset_workspace(case, runtime, binary)
        roots = {name: runtime / name for name in ('work', 'home', 'tmp', 'bin')}
        expected = h.snapshot(roots)
        command = ['/usr/bin/python3', 'launch.py', model['mode'], str(model['limit']),
                   str(roots['bin'] / 'make'), *case.args]
        out, err, status, bound, left = h.capture(command, roots['work'],
                                                 h.clean_environment(runtime), case.timeout)
        filesystem = h.snapshot(roots)
        observer = roots['work'] / 'observer'
        raw = observer.read_bytes() if observer.is_file() else b''
        source = re.match(rb'^create (Gm[A-Za-z0-9]{6})\n', raw)
        expected_error = model['stderr'].replace('@TMP@', str(roots['tmp']))
        observes_stdin = model['mode'].startswith('stdin')
        witness_valid = not observes_stdin or source is not None
        if source is not None and observes_stdin:
            name = source.group(1).decode('ascii')
            expected['work/observer'] = file_record(model['observer'].replace('@SOURCE@', name))
            expected_error = expected_error.replace('@SOURCE@', name)
            if model['source_remains']:
                expected['tmp/' + name] = file_record(model['source_contents'], mode=0o600)
        for name, contents in model['created'].items():
            expected['work/' + name] = file_record(contents)
        errors = [name for name, different in (
            ('stdout', out != model['stdout'].encode()), ('stderr', err != expected_error.encode()),
            ('status', status != model['status']), ('bounds', bool(bound or left)),
            ('source lifecycle witness', not witness_valid), ('complete filesystem', filesystem != expected),
        ) if different]
        observations.append({'command': command, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(),
                             'status': status, 'bound': bound, 'leftovers': left, 'filesystem': filesystem,
                             'expected_filesystem': expected, 'expected_stderr_hex': expected_error.encode().hex(),
                             'observer_hex': raw.hex(), 'errors': errors})
    if model['mode'].startswith('stdin'):
        # Each run independently matches the sole kernel CREATE identity,
        # ordered lifecycle, raw streams and complete post-run filesystem.
        repeated = all(not item['errors'] for item in observations[:2])
    else:
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
    previous = os.umask(0o022)
    try:
        with tempfile.TemporaryDirectory(prefix='rmake-file-size-') as temp:
            root = Path(temp)
            inputs, runtime = root / 'inputs', root / 'runtime'
            inputs.mkdir()
            runtime.mkdir()
            rows = [execute(row, inputs, runtime, binaries) for row in models]
    finally:
        os.umask(previous)
    report = {'oracle_profile': profile['profile_id'], 'oracle_sha256': h.sha256_file(reference),
              'candidate_sha256': h.sha256_file(candidate) if candidate is not None else None,
              'functional_cpu_affinity': sorted(os.sched_getaffinity(0)),
              'source_hashes': {str(path.relative_to(ROOT)): h.sha256_file(path) for path in
                                (Path(__file__).resolve(), MODELS, FIXTURES / 'launch.py')},
              'results': rows}
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
