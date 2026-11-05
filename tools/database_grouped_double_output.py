#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare exact grouped double-colon branch state and physical effects."""
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
UNCHECKED = '#  Modification time never checked.'
ORIGINAL = '#  Last modified 2020-09-13 12:26:50'
FORCED = '#  Last modified 2020-09-13 12:26:45.123456789'
INPUT = FIXED + 20_000_000_000
EXISTING = FIXED + 10_000_000_000
REAL = FIXED + 5_123_456_789
CLOCK = re.compile(rb'(?m)^# (Make data base, printed on|Finished Make data base on) ([^\n]{24})$')
SKIP = "printf '%s\\n' skipped > $@"
SECOND = "printf '%s\\n' second > $@"
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
    group_records: int = 2
    not_updated: tuple[str, ...] = ()


def models():
    first = 'a b &:: input ; @' + SKIP + '\n'
    second = 'a b &:: other ; @' + SECOND + '\n'
    missing = (('a:: input', MISSING), ('b:: input', MISSING))
    yield Model('group-double-missing', first, ('a',), missing, (SKIP.replace('$@', 'a'),))
    yield Model('group-double-existing-peer', first, ('a',),
                (('a:: input', MISSING), ('b:: input', ORIGINAL)), (SKIP.replace('$@', 'a'),), (('b', EXISTING),))
    yield Model('group-double-both-declarations', first + second, ('a', 'b'),
                missing + (('a:: other', MISSING), ('b:: other', MISSING)),
                (SKIP.replace('$@', 'a'), SECOND.replace('$@', 'a')), group_records=4)
    ordinary = 'a:: other ; @' + SECOND + '\n'
    yield Model('group-double-mixed-standalone', first + ordinary, ('a',),
                (('a:: input', SYNTHETIC), ('a:: other', SYNTHETIC), ('b:: input', MISSING)),
                (SKIP.replace('$@', 'a'), SECOND.replace('$@', 'a')))
    yield Model('group-double-forced-physical', 'a b &:: input ; +@' + FORCE_GROUP + '\n',
                ('a',), (('a:: input', FORCED), ('b:: input', FORCED)), (FORCE_GROUP,), actual=('a', 'b'))
    yield Model('group-double-question-without-start', first, ('a',),
                (('a:: input', MISSING), ('b:: input', UNCHECKED)), flags=('-q',), status=1,
                not_updated=('b:: input',))
    yield Model('group-double-already-current', 'a b &:: older ; @' + SKIP + '\n', ('a',),
                (('a:: older', ORIGINAL), ('b:: older', ORIGINAL)),
                original=(('a', EXISTING), ('b', EXISTING)), not_updated=('b:: older',))


def fixture(model, path):
    path.mkdir()
    (path / 'Makefile').write_text(model.makefile)
    for name in ('input', 'other', 'older'):
        (path / name).write_bytes(b'known input\n')
    for name, _ in model.original:
        (path / name).write_bytes(b'physical target\n')
    args = ('-rR', '-s', '--no-print-directory', '-p', '-f', 'Makefile', *model.flags, *model.goals)
    return h.Case(path, model.id, 'database.dry-run-group-time', 'required', args, 15, {}, {})


def file_times(work):
    return {path.name: path.stat().st_mtime_ns for path in sorted(work.iterdir()) if path.is_file()}


def check_stdout(model, output, start, end):
    fields = list(CLOCK.finditer(output))
    if [field[1] for field in fields] != [b'Make data base, printed on', b'Finished Make data base on']:
        raise h.HarnessError('expected a complete database dump')
    a, b = (db.parse_clock(field[2]) for field in fields)
    if not start <= a <= b <= end or b'(device ' in output:
        raise h.HarnessError('unexpected positioned clock or physical directory fields')
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
        updated = rule not in model.not_updated
        marker = b'#  File has been updated.' if updated else b'#  File has not been updated.'
        if marker not in body.splitlines():
            raise h.HarnessError(f'{rule} did not have the expected update state')
        if updated and model.status == 0 and b'#  Successfully updated.' not in body.splitlines():
            raise h.HarnessError(f'{rule} expected a successful branch result')
    membership = re.findall(rb'(?m)^#  Also makes: (.*)$', section)
    expected_membership = [b'b a'] * model.group_records
    if membership != expected_membership:
        raise h.HarnessError(f'group member order differs: {membership!r}')
    prefix = output[:fields[0].start()].splitlines()
    commands = tuple(line.decode() for line in prefix if line.startswith(b'printf '))
    if commands != model.echo:
        raise h.HarnessError(f'unexpected visible recipe commands: {commands!r}')
    return CLOCK.sub(lambda field: b'# ' + field[1] + b' <checked libc ctime>', output)


def run(model, case, runtime, engine):
    h.reset_workspace(case, runtime, engine)
    work = runtime / 'work'
    mtimes = {'input': INPUT, 'other': INPUT, **dict(model.original)}
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
    start = db.libc_time()
    out, err, status, limit, leftovers = h.capture(argv, work, h.clean_environment(runtime), case.timeout)
    end = db.libc_time()
    filesystem = h.snapshot(roots)
    times = file_times(work)
    result = {'engine': str(engine), 'argv': argv, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(), 'status': status,
              'limit': limit, 'leftovers': leftovers, 'filesystem': filesystem, 'expected_filesystem': expected,
              'physical_mtime_ns': times, 'expected_mtime_ns': expected_times, 'libc_window': [start, end]}
    try:
        if (status, err, limit, leftovers, filesystem, times) != (model.status, b'', None, False, expected, expected_times):
            raise h.HarnessError('unexpected status, streams, process bound or physical file effect')
        result['comparison_sha256'] = hashlib.sha256(check_stdout(model, out, start, end)).hexdigest()
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
        parser.error('unknown grouped double-colon database case')
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
