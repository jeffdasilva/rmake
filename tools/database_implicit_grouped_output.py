#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare exact implicit-group database membership, timestamps and effects."""
from __future__ import annotations

import argparse
import base64
from dataclasses import asdict, dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import tempfile

import conformance as h
import database_output as db
import oracle as reference

FIXED = 1_600_000_000_000_000_000
SYNTHETIC = '#  Last modified 2514-05-30 01:53:03.107374182'
MISSING = '#  File does not exist.'
ORIGINAL = '#  Last modified 2020-09-13 12:26:50'
FORCED = '#  Last modified 2020-09-13 12:26:45.123456789'
INPUT = FIXED + 20_000_000_000
EXISTING = FIXED + 10_000_000_000
REAL = FIXED + 5_123_456_789
CLOCK = re.compile(rb'(?m)^# (Make data base, printed on|Finished Make data base on) ([^\n]{24})$')
SKIP = "printf '%s\\n' skipped > $@"
FORCE_GROUP = "printf '%s\\n' actual > a; printf '%s\\n' actual > b; touch -d @1600000005.123456789 a b"


@dataclass(frozen=True)
class Model:
    id: str
    makefile: str
    goals: tuple[str, ...]
    records: tuple[tuple[str, str], ...]
    echo: tuple[str, ...] = ()
    original: tuple[tuple[str, int], ...] = ()
    actual: tuple[str, ...] = ()
    flags: tuple[str, ...] = ('-n',)
    status: int = 0
    peers: tuple[str, ...] = ("pair.b",)
    declaration: str = "%.a %.b: %.src"


def models():
    pattern = 'pair.src:\n%.a %.b: %.src ; @' + SKIP + '\n'
    yield Model('implicit-group-missing', pattern, ('pair.a',),
                (('pair.a: pair.src', SYNTHETIC), ('pair.b:', MISSING)), (SKIP.replace('$@', 'pair.a'),))
    yield Model('implicit-group-existing-peer', pattern, ('pair.a',),
                (('pair.a: pair.src', SYNTHETIC), ('pair.b:', ORIGINAL)),
                (SKIP.replace('$@', 'pair.a'),), (('pair.b', EXISTING),))
    yield Model('implicit-group-other-owner', pattern, ('pair.b', 'pair.a'),
                (('pair.b: pair.src', SYNTHETIC), ('pair.a:', ORIGINAL)),
                (SKIP.replace('$@', 'pair.b'),), (('pair.a', EXISTING),), peers=('pair.a',))
    real = FORCE_GROUP.replace('> a;', '> pair.a;').replace('> b;', '> pair.b;').removesuffix(' a b') + ' pair.a pair.b'
    yield Model('implicit-group-forced-physical', 'pair.src:\n%.a %.b: %.src ; +@' + real + '\n',
                ('pair.a',), (('pair.a: pair.src', FORCED), ('pair.b:', FORCED)),
                (real,), actual=('pair.a', 'pair.b'))
    three = 'pair.src:\n%.a %.b %.c: %.src ; @' + SKIP + '\n'
    yield Model('implicit-group-three-outputs', three, ('pair.a',),
                (('pair.a: pair.src', SYNTHETIC), ('pair.b:', ORIGINAL), ('pair.c:', MISSING)),
                (SKIP.replace('$@', 'pair.a'),), (('pair.b', EXISTING),),
                peers=('pair.c', 'pair.b'), declaration='%.a %.b %.c: %.src')
    yield Model('implicit-group-empty-recipe', 'pair.src:\n%.a %.b: %.src ;\n', ('pair.a',),
                (('pair.a: pair.src', SYNTHETIC), ('pair.b:', MISSING)))


def fixture(model, path):
    path.mkdir()
    (path / 'Makefile').write_text(model.makefile)
    (path / 'pair.src').write_bytes(b'known input\n')
    for name, _ in model.original:
        (path / name).write_bytes(b'physical target\n')
    args = ('-rR', '-s', '--no-print-directory', '-p', '-f', 'Makefile', *model.flags, *model.goals)
    return h.Case(path, model.id, 'database.dry-run-group-time', 'required', args, 15, {}, {})


def file_times(work):
    return {path.name: path.stat().st_mtime_ns for path in sorted(work.iterdir()) if path.is_file()}


def check_stdout(model, output, start, end, directory):
    fields = list(CLOCK.finditer(output))
    if [field[1] for field in fields] != [b'Make data base, printed on', b'Finished Make data base on']:
        raise h.HarnessError('expected a complete database dump')
    a, b = (db.parse_clock(field[2]) for field in fields)
    if not start <= a <= b <= end:
        raise h.HarnessError('unexpected positioned clock')
    directory_line = f'# . (device {directory.st_dev}, inode {directory.st_ino}): No files, no impossibilities so far.'.encode()
    if output.count(b' (device ') != 1 or output.count(b'\n' + directory_line + b'\n') != 1:
        raise h.HarnessError('expected the checked identity of the owned work directory')
    output = output.replace(directory_line, b'# . <checked identity>: No files, no impossibilities so far.', 1)
    header, footer = b'\n# Files\n', b'\n# files hash-table stats:\n'
    if output.count(header) != 1 or output.count(footer) != 1 or output.index(header) > output.index(footer):
        raise h.HarnessError('invalid file database section')
    section = output[output.index(header) + len(header):output.index(footer)]
    for rule, time in model.records:
        line = b'\n' + rule.encode() + b'\n'
        if section.count(line) != 1:
            raise h.HarnessError('expected one exact database rule line ' + rule)
        body = section.split(line, 1)[1].split(b'\n\n', 1)[0]
        actual = [entry for entry in body.splitlines() if entry.startswith((b'#  Last modified ', b'#  File does not exist.', b'#  Modification time never checked.'))]
        if actual != [time.encode()]:
            raise h.HarnessError(f'{rule} expected {time!r}, observed {actual!r}')
    expected_membership = ' '.join(model.peers).encode()
    members = re.findall(rb'(?m)^#  Also makes: (.*)$', section)
    if members != [expected_membership]:
        raise h.HarnessError(f'implicit peer order differs: {members!r}')
    implicit = output.split(b'\n# Implicit Rules\n', 1)[1].split(header, 1)[0]
    if implicit.count(b'\n' + model.declaration.encode() + b'\n') != 1 or b'\n# 1 implicit rules, 0 (0.0%) terminal.' not in implicit:
        raise h.HarnessError('expected one multi-target implicit declaration')
    prefix = output[:fields[0].start()].splitlines()
    commands = tuple(line.decode() for line in prefix if line.startswith(b'printf '))
    if commands != model.echo:
        raise h.HarnessError(f'unexpected visible recipe commands: {commands!r}')
    return CLOCK.sub(lambda field: b'# ' + field[1] + b' <checked libc ctime>', output)


def run(model, case, runtime, engine):
    h.reset_workspace(case, runtime, engine)
    work = runtime / 'work'
    mtimes = {'pair.src': INPUT, **dict(model.original)}
    for path in work.iterdir():
        stamp = mtimes.get(path.name, FIXED)
        os.utime(path, ns=(stamp, stamp))
    roots = {name: runtime / name for name in ('work', 'home', 'tmp')}
    expected = h.snapshot(roots)
    expected_times = file_times(work)
    for name in model.actual:
        content = b'actual\n'
        expected['work/' + name] = {'type': 'file', 'mode': 0o644, 'size': len(content),
                                    'sha256': hashlib.sha256(content).hexdigest(), 'content': base64.b64encode(content).decode()}
        expected_times[name] = REAL
    argv = [str(runtime / 'bin/make'), *case.args]
    directory = work.stat()
    start = db.libc_time()
    out, err, status, limit, leftovers = h.capture(argv, work, h.clean_environment(runtime), case.timeout)
    end = db.libc_time()
    filesystem = h.snapshot(roots)
    times = file_times(work)
    result = {'engine': str(engine), 'argv': argv, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(), 'status': status,
              'limit': limit, 'leftovers': leftovers, 'filesystem': filesystem, 'expected_filesystem': expected,
              'physical_mtime_ns': times, 'expected_mtime_ns': expected_times, 'libc_window': [start, end], 'physical_work_directory': {'device': directory.st_dev, 'inode': directory.st_ino}}
    try:
        if (status, err, limit, leftovers, filesystem, times) != (model.status, b'', None, False, expected, expected_times):
            raise h.HarnessError('unexpected status, streams, process bound or physical file effect')
        result['comparison_sha256'] = hashlib.sha256(check_stdout(model, out, start, end, directory)).hexdigest()
    except (h.HarnessError, ValueError) as error:
        result['error'] = str(error)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--candidate', type=Path, action='append', default=[])
    parser.add_argument('--oracle', type=Path, default=reference.PREFIX / 'bin/make')
    parser.add_argument('--oracle-profile', type=Path)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--case')
    options = parser.parse_args()
    selected = [model for model in models() if options.case is None or model.id == options.case]
    if not selected:
        parser.error('unknown implicit grouped database case')
    os.umask(0o022)
    oracle = options.oracle.absolute()
    profile = reference.verify(oracle, options.oracle_profile or oracle.parent.parent / 'profile.json')
    engines = [oracle, oracle, *(path.resolve() for path in options.candidate)]
    rows = []
    with tempfile.TemporaryDirectory(prefix='rmake-db-dry-branches-') as scratch:
        path = Path(scratch)
        runtime = path / 'runtime'
        runtime.mkdir()
        for model in selected:
            case = fixture(model, path / model.id)
            observations = [run(model, case, runtime, engine) for engine in engines]
            signature = observations[0].get('comparison_sha256')
            oracle_passed = signature is not None and observations[1].get('comparison_sha256') == signature
            passed = oracle_passed and all(entry.get('comparison_sha256') == signature for entry in observations)
            rows.append({'model': asdict(model), 'oracle_passed': oracle_passed, 'passed': passed, 'observations': observations})
            print(f"{'PASS' if passed else 'FAIL'} {model.id}", flush=True)
    data = {'oracle_profile_id': profile['profile_id'], 'passed': all(row['passed'] for row in rows), 'oracle_passed': all(row['oracle_passed'] for row in rows),
            'rows': rows, 'source_sha256': h.sha256_file(Path(__file__)), 'affinity': sorted(os.sched_getaffinity(0)),
            'identities': [h.executable_identity(engine, oracle=i < 2) for i, engine in enumerate(engines)]}
    options.report.parent.mkdir(parents=True, exist_ok=True)
    options.report.parent.mkdir(parents=True, exist_ok=True)
    with options.report.open('w') as stream:
        json.dump(data, stream, indent=2)
    return 0 if data['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
