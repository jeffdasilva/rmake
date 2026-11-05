#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Exact verbose debug decisions with fixed timestamps in C and French."""

import argparse
import json
import os
from pathlib import Path
import tempfile

import conformance as h
import nls_diagnostics as nls
import oracle

ROOT = Path(__file__).resolve().parents[1]
MODELS = ROOT / 'tests/debug-verbose/models.json'
NS_PER_SECOND = 1_000_000_000
SOURCE_TIME = 1_600_000_000


def set_mtime(path, seconds):
    stamp = seconds * NS_PER_SECOND
    os.utime(path, ns=(stamp, stamp))


def times(work):
    return {str(path.relative_to(work)): path.stat().st_mtime_ns
            for path in work.rglob('*') if path.is_file() and not path.is_symlink()}


def prepare(root, model):
    root.mkdir()
    makefile = root / 'Makefile'
    makefile.write_bytes(model['makefile'].encode())
    set_mtime(makefile, SOURCE_TIME)
    for name, seconds in model['initial_file_offsets'].items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b'file')
        set_mtime(path, SOURCE_TIME + seconds)


def observation(case, runtime, executable, model, expected):
    actual = h.execute(case, runtime, executable)
    mtime = times(runtime / 'work')
    wanted = h.Observation(expected['stdout'].encode(), expected['stderr'].encode(),
                           expected['returncode'], None, expected['filesystem'])
    differences = h.differences(wanted, actual)
    if mtime != {name: seconds * NS_PER_SECOND for name, seconds in model['mtime_seconds'].items()}:
        differences.append('mtime_ns')
    return {**h.observation_json(actual), 'stdout_hex': actual.stdout.hex(),
            'stderr_hex': actual.stderr.hex(), 'mtime_ns': mtime, 'differences': differences}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=oracle.PREFIX / 'bin/make')
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--locale-root', type=Path, required=True)
    output = parser.add_mutually_exclusive_group(required=True)
    output.add_argument('--report', type=Path)
    output.add_argument('--report-dir', type=Path)
    parser.add_argument('--case', action='append')
    args = parser.parse_args()
    if args.report is not None and args.report.exists():
        parser.error('refusing to replace retained evidence')
    data = json.loads(MODELS.read_bytes())
    if data['schema_version'] != 1:
        parser.error('unsupported debug-verbose model schema')
    models = [model for model in data['models'] if not args.case or model['id'] in args.case]
    if not models or (args.case and {model['id'] for model in models} != set(args.case)):
        parser.error('unknown verbose-debug case')
    catalog = json.loads((nls.DATA / 'models.json').read_bytes())['reference_catalog_sha256']
    reference, profile, _, locale = nls.prepare_reference(args.oracle, catalog, args.locale_root)
    candidate = args.candidate.resolve()
    report = {'model_sha256': h.sha256_file(MODELS), 'tool_sha256': h.sha256_file(Path(__file__)),
              'affinity': sorted(os.sched_getaffinity(0)), 'reference_profile_id': profile['profile_id'],
              'locale_profile': locale, 'candidate': h.executable_identity(candidate), 'cases': []}
    with tempfile.TemporaryDirectory(prefix='rmake-verbose-debug-') as temporary:
        scratch = Path(temporary)
        runtime = scratch / 'runtime'
        runtime.mkdir()
        for model in models:
            source = scratch / model['id']
            prepare(source, model)
            for language, template in model['expected'].items():
                environment = {**locale['environment'], 'LC_ALL': language, 'LANG': language}
                case = h.Case(source, model['id'], 'debug.verbose', 'required',
                              ('-rR', '--no-print-directory', *model['args']), 5, environment, {})
                h.reset_workspace(case, runtime, reference)
                filesystem = h.snapshot({name: runtime / name for name in ('work', 'home', 'tmp')})
                expected = {**template, 'filesystem': filesystem}
                records = [observation(case, runtime, executable, model, expected)
                           for executable in (reference, reference, candidate)]
                passed = all(not record['differences'] for record in records)
                report['cases'].append({'id': model['id'], 'locale': language, 'runs': records, 'passed': passed})
                print('PASS' if passed else 'FAIL', language, model['id'], flush=True)
    report['passed'] = sum(row['passed'] for row in report['cases'])
    report['total'] = len(report['cases'])
    path = args.report
    if path is None:
        args.report_dir.mkdir(parents=True, exist_ok=True)
        path = Path(tempfile.mkdtemp(prefix='run-', dir=args.report_dir)) / 'report.json'
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('x') as output:
        json.dump(report, output, indent=2)
        output.write('\n')
    print(f"Verbose debug gate: {report['passed']}/{report['total']} ({path})")
    return int(report['passed'] != report['total'])


if __name__ == '__main__':
    raise SystemExit(main())
