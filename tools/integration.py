#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Pinned, reviewed real-project GNU2/rmake builds, never a security sandbox."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import platform
import sys
import tarfile
import tempfile
import urllib.error
import urllib.request

import conformance as h
import integration_records
import oracle

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / 'tests/integration/projects.json'
SOURCE_TIME = 1600000000000000000


def safe_name(value):
    path = PurePosixPath(value)
    if not value or path.is_absolute() or '..' in path.parts or '\0' in value:
        raise h.HarnessError('integration path escapes its reviewed source')
    return path


def archive_for(project, cache):
    if not re.fullmatch(r'[a-z0-9][a-z0-9._-]*', project['id']):
        raise h.HarnessError('invalid integration project identifier')
    cache.mkdir(parents=True, exist_ok=True)
    archive = cache / (project['id'] + '.tar.gz')
    expected = project['archive_sha256']
    if archive.is_symlink():
        raise h.HarnessError('integration cache archive is a symlink')
    if archive.exists():
        if h.sha256_file(archive) != expected:
            raise h.HarnessError('cached source archive digest changed')
        return archive
    last = None
    for url in [project['source_url'], *([project['fallback_url']] if project.get('fallback_url') else [])]:
        try:
            with urllib.request.urlopen(url, timeout=30) as response:
                data = response.read(h.MAX_FILE_BYTES + 1)
            if len(data) > h.MAX_FILE_BYTES or hashlib.sha256(data).hexdigest() != expected:
                raise h.HarnessError('downloaded source fails pinned size/digest')
            with tempfile.NamedTemporaryFile(dir=cache, prefix='.download-', delete=False) as output:
                temporary = Path(output.name)
                output.write(data)
            temporary.replace(archive)
            return archive
        except (urllib.error.HTTPError, urllib.error.URLError) as error:
            last = error
    raise h.HarnessError(f'cannot fetch pinned source: {last}')


def extract(project, archive, destination):
    destination.mkdir()
    top = safe_name(project['topdir'])
    if len(top.parts) != 1:
        raise h.HarnessError('integration archive top must be one directory')
    with tarfile.open(archive, 'r:gz') as source:
        members = source.getmembers()
        if len(members) > h.MAX_FILES or sum(m.size for m in members if m.isfile()) > h.MAX_FILE_BYTES:
            raise h.HarnessError('integration source exceeds fixture bounds')
        for member in members:
            name = safe_name(member.name)
            if name.parts[0] != str(top) or not (member.isfile() or member.isdir() or member.issym() or member.islnk()):
                raise h.HarnessError('unreviewed archive root/type')
        # The data filter also checks resolved symlink/hardlink destinations.
        source.extractall(destination, members=members, filter='data')
    fixture = destination / str(top)
    h.validate_fixture(fixture)
    license_file = fixture / str(safe_name(project['license_file']))
    if h.sha256_file(license_file) != project['license_sha256']:
        raise h.HarnessError('source license differs from reviewed archive')
    return fixture


def raw_observation(value):
    return {**h.observation_json(value), 'stdout_hex': value.stdout.hex(), 'stderr_hex': value.stderr.hex()}


def observe(command, runtime, environment, timeout=120):
    out, err, status, limit, leftovers = h.capture(command, runtime / 'work', environment, timeout)
    files = h.snapshot({name: runtime / name for name in ('work', 'home', 'tmp')})
    return h.Observation(out, err, status, limit, files, leftovers)


def successful(value):
    return value.returncode == 0 and value.limit is None and not value.leftover_processes


def validate_smoke(value, expected):
    return successful(value) and value.stdout == expected and not value.stderr


def freshness_errors(fresh, noop, rebuilt):
    errors = []
    if fresh != noop:
        errors.append('no-op changed watched output timestamps')
    if set(fresh) != set(rebuilt) or any(rebuilt.get(name, 0) <= stamp for name, stamp in fresh.items()):
        errors.append('header mutation did not rebuild watched outputs')
    return errors


def rebuilt_header(project, original):
    change = project.get('header_rebuild')
    if change is None:
        return original + b'\n/* rmake independent integration freshness control */\n'
    before = bytes.fromhex(change['before_hex'])
    after = bytes.fromhex(change['after_hex'])
    if not before or before == after or original.count(before) != 1:
        raise h.HarnessError('declared header rebuild needs exactly one changed source fragment')
    return original.replace(before, after, 1)


def persist_part(directory, label, value):
    return integration_records.persist(directory, label, value)


def campaign(project, fixture, runtime, binaries, plan, parts):
    case = h.Case(fixture, project['id'], 'integration.real-projects', 'required', (), 120, plan['environment'], {})
    row = {'id': project['id'], 'runs': [], 'input': persist_part(parts, project['id'] + '-input', h.snapshot({'work': fixture}))}
    comparisons = []
    change = project.get('header_rebuild')
    expected_steps = 5 if change is not None else 4
    for index, binary in enumerate(binaries):
        h.reset_workspace(case, runtime, binary)
        environment = h.clean_environment(runtime, plan['environment'])
        command = [str(runtime / 'bin/make'), *project.get('args', plan['args']), project['target']]
        run = {'binary': str(binary), 'command': command, 'parts': [], 'errors': []}
        row['runs'].append(run)
        steps = []
        def step(label, arguments, timeout=120):
            value = observe(arguments, runtime, environment, timeout)
            steps.append(value)
            run['parts'].append(persist_part(parts, f"{project['id']}-{index}-{label}", raw_observation(value)))
            if not successful(value):
                raise h.HarnessError(label + ': status/process bounds')
            return value
        try:
            step('fresh', command)
            fresh = {name: (runtime / 'work' / str(safe_name(name))).stat().st_mtime_ns for name in project['watched']}
            for name in project['watched']:
                data = (runtime / 'work' / name).read_bytes()
                if not data.startswith(b'!<arch>\n' if name.endswith('.a') else b'\x7fELF'):
                    raise h.HarnessError('missing archive/ELF output')
            if change is not None:
                smoke = step('fresh-smoke', project['smoke'], 10)
                if not validate_smoke(smoke, bytes.fromhex(change['fresh_smoke_stdout_hex'])):
                    run['errors'].append('independent fresh executable smoke bytes')
            step('noop', command)
            noop = {name: (runtime / 'work' / name).stat().st_mtime_ns for name in project['watched']}
            header = runtime / 'work' / str(safe_name(project['header']))
            header.write_bytes(rebuilt_header(project, header.read_bytes()))
            stamp = max(fresh.values()) + 1
            os.utime(header, ns=(stamp, stamp))
            step('rebuild', command)
            rebuilt = {name: (runtime / 'work' / name).stat().st_mtime_ns for name in project['watched']}
            run['mtime_witness'] = {'fresh': fresh, 'noop': noop, 'header': stamp, 'rebuilt': rebuilt}
            run['errors'].extend(freshness_errors(fresh, noop, rebuilt))
            smoke = step('smoke', project['smoke'], 10)
            if not validate_smoke(smoke, bytes.fromhex(project['smoke_stdout_hex'])):
                run['errors'].append('independent executable smoke bytes')
        except (h.HarnessError, OSError, ValueError) as error:
            run['errors'].append(str(error))
        comparisons.append(steps)
    row['differences'] = []
    for index in range(1, len(comparisons)):
        pair = comparisons[index]
        differences = (['incomplete build sequence'] if len(pair) != expected_steps or len(comparisons[0]) != expected_steps
                       else [h.differences(a, b) for a, b in zip(comparisons[0], pair)])
        row['differences'].append(differences)
    row['passed'] = not any(run['errors'] for run in row['runs']) and not any(
        item for pair in row['differences'] for item in pair)
    return row


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--cache', type=Path, default=ROOT / '.tools/integration')
    parser.add_argument('--report', type=Path, default=ROOT / 'artifacts/integration/report.json')
    parser.add_argument('--bootstrap-only', action='store_true')
    parser.add_argument('--case', action='append')
    args = parser.parse_args(argv)
    plan = json.loads(MANIFEST.read_text())
    if plan.get('schema_version') != 1:
        raise h.HarnessError('unknown integration manifest schema')
    selected = plan['projects']
    if args.case:
        if set(args.case) - {p['id'] for p in selected}: parser.error('unknown exact project')
        selected = [p for p in selected if p['id'] in args.case]
    archives = [(p, archive_for(p, args.cache)) for p in selected]
    if args.bootstrap_only:
        with tempfile.TemporaryDirectory(prefix='rmake-integration-source-check-') as tmp:
            for project, archive in archives:
                extract(project, archive, Path(tmp) / project['id'])
                print('Verified', project['id'], project['archive_sha256'])
        return 0
    oracle.verify(args.oracle.resolve())
    binaries = [args.oracle.resolve(), args.oracle.resolve()]
    if args.candidate: binaries.append(args.candidate.resolve())
    args.report.parent.mkdir(parents=True, exist_ok=True)
    parts = Path(tempfile.mkdtemp(prefix=args.report.stem + '-raw-', dir=args.report.parent))
    report = {'classification': plan['classification'], 'manifest': plan,
              'tool_sha256': h.sha256_file(Path(__file__)), 'observer_sha256': h.sha256_file(Path(h.__file__)),
              'record_tool_sha256': h.sha256_file(Path(integration_records.__file__)),
              'affinity': sorted(os.sched_getaffinity(0)), 'oracle_only': args.candidate is None,
              'binaries': [h.executable_identity(b) for b in binaries],
              'compiler': h.executable_identity(Path('/usr/bin/cc')), 'projects': []}
    report['host'] = {'python': sys.version, 'kernel': platform.uname()._asdict(),
                      'libc': platform.libc_ver(), 'locale': 'C'}
    report['build_tools'] = {name: h.executable_identity(Path('/usr/bin') / name)
                             for name in ('cc', 'as', 'ar', 'ld')}
    shell = Path('/bin/sh').resolve()
    report['shell'] = {'path': str(shell), 'sha256': h.sha256_file(shell)}
    try:
        with tempfile.TemporaryDirectory(prefix='rmake-integration-') as tmp:
            root = Path(tmp)
            runtime = root / 'runtime'; runtime.mkdir()
            report['runtime_root'] = str(runtime)
            for project, archive in archives:
                fixture = extract(project, archive, root / (project['id'] + '-source'))
                prepare = root / (project['id'] + '-prepare'); prepare.mkdir()
                for name in ('home', 'tmp'): (prepare / name).mkdir()
                environment = h.clean_environment(prepare, {**plan['environment'], 'CC': '/usr/bin/cc', 'CFLAGS': '-O1 -g0'})
                preparation = None
                if project.get('configure'):
                    out, err, status, limit, leftovers = h.capture(project['configure'], fixture, environment, 60)
                    preparation = persist_part(parts, project['id'] + '-prepare',
                                               {'stdout_hex': out.hex(), 'stderr_hex': err.hex(), 'status': status,
                                                'limit': limit, 'leftovers': leftovers})
                    if (status, limit, leftovers) != (0, None, False):
                        raise h.HarnessError('common configure preparation failed')
                (fixture / 'empty.input').write_bytes(b'')
                for path in fixture.rglob('*'):
                    if path.is_file() and not path.is_symlink(): os.utime(path, ns=(SOURCE_TIME, SOURCE_TIME))
                h.validate_fixture(fixture)
                row = campaign(project, fixture, runtime, binaries, plan, parts)
                row['preparation'] = preparation
                report['projects'].append(row)
                print(('PASS ' if row['passed'] else 'FAIL ') + project['id'], flush=True)
                args.report.write_text(json.dumps(report, indent=2) + '\n')
        report['passed'] = all(row['passed'] for row in report['projects'])
    except (h.HarnessError, OSError, ValueError, tarfile.TarError) as error:
        report['passed'] = False
        report['error'] = str(error)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
