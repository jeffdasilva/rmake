#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare failed, interrupted and aliased grouped double-colon branches."""
from __future__ import annotations

import argparse
import base64
from dataclasses import asdict, dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import signal
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
    failed: tuple[str, ...] = ()
    stderr: str = ''
    cancel: bool = False
    directories: tuple[str, ...] = ()
    members: str = 'b a'


def models():
    fail = FORCE_GROUP + '; exit 7'
    failed_records = (('a:: input', FORCED), ('b:: input', FORCED))
    failed = ('a:: input', 'b:: input')
    error = 'make: *** [Makefile:1: a] Error 7\n'
    yield Model('failed-command-stops', 'a b &:: input ; @' + fail + '\n', ('a',),
                failed_records, actual=('a', 'b'), flags=(), status=2, failed=failed, stderr=error)
    next_recipe = "printf '%s\\n' skipped > next"
    yield Model('failed-command-keep-going', 'a b &:: input ; +@' + fail + '\nnext:: other ; @' + next_recipe + '\n', ('a', 'next'),
                failed_records + (('next:: other', SYNTHETIC),), (fail, next_recipe),
                actual=('a', 'b'), flags=('-n', '-k'), status=2, failed=failed, stderr=error)
    yield Model('failed-command-parallel', 'a b &:: input ; @' + fail + '\n', ('a',),
                failed_records, actual=('a', 'b'), flags=('-j2', '--jobserver-style=pipe'), status=2,
                failed=failed, stderr=error)
    expansion = 'a b &:: input ; @$(error branch aborted)true\n'
    yield Model('failed-before-command-start', expansion, ('a',), (('a:: input', MISSING), ('b:: input', UNCHECKED)),
                flags=(), status=2, not_updated=failed, stderr='Makefile:1: *** branch aborted.  Stop.\n')
    interrupted = 'a b &:: input ; @' + FORCE_GROUP + "; printf 'ready\\n'; exec sleep 30\n"
    owner_delete = "make: *** Deleting file 'a'\n"
    yield Model('cancelled-sigterm', interrupted, ('a',), (), flags=(), status=-15, cancel=True,
                stderr=owner_delete + "make: *** [a] Deleting file 'b'\nmake: *** [Makefile:1: a] Terminated\n")
    yield Model('cancelled-sigterm-precious-peer', '.PRECIOUS: b\n' + interrupted, ('a',), (),
                actual=('b',), flags=(), status=-15, cancel=True, stderr=owner_delete + "make: *** [Makefile:2: a] Terminated\n")
    paths = 'VPATH = src\nGPATH = src\n'
    existing = (('src/a', EXISTING), ('src/b', EXISTING))
    yield Model('aliased-gpath-stale', paths + 'a b &:: input ; @' + SKIP + '\n', ('a',),
                (('src/a:: input', ORIGINAL), ('src/b:: input', ORIGINAL)), (SKIP.replace('$@', 'src/a'),),
                existing, directories=('src',), members='b a')
    yield Model('aliased-gpath-current', paths + 'a b &:: older ; @' + SKIP + '\n', ('a',),
                (('src/a:: older', ORIGINAL), ('src/b:: older', ORIGINAL)), original=existing,
                directories=('src',), members='b a', not_updated=('src/b:: older',))
    deleting = '.DELETE_ON_ERROR:\na b &:: input ; @' + fail + '\n'
    yield Model('failed-command-deletes-group', deleting, ('a',),
                (('a:: input', MISSING), ('b:: input', MISSING)), flags=(), status=2, failed=failed,
                stderr="make: *** [Makefile:2: a] Error 7\nmake: *** Deleting file 'a'\nmake: *** [a] Deleting file 'b'\n")


def fixture(model, path):
    path.mkdir()
    (path / 'Makefile').write_text(model.makefile)
    for name in ('input', 'other', 'older'):
        (path / name).write_bytes(b'known input\n')
    for name, _ in model.original:
        (path / name).parent.mkdir(parents=True, exist_ok=True)
        (path / name).write_bytes(b'physical target\n')
    args = ('-rR', '-s', '--no-print-directory', '-p', '-f', 'Makefile', *model.flags, *model.goals)
    return h.Case(path, model.id, 'database.dry-run-group-time', 'required', args, 15, {}, {})


def file_times(work):
    return {path.relative_to(work).as_posix(): path.stat().st_mtime_ns for path in sorted(work.rglob('*')) if path.is_file()}


def check_stdout(model, output, start, end, directories):
    if model.cancel:
        if output != b'ready\n':
            raise h.HarnessError('unexpected output after live recipe cancellation')
        return output
    fields = list(CLOCK.finditer(output))
    if [field[1] for field in fields] != [b'Make data base, printed on', b'Finished Make data base on']:
        raise h.HarnessError('expected a complete database dump')
    a, b = (db.parse_clock(field[2]) for field in fields)
    if not start <= a <= b <= end:
        raise h.HarnessError('unexpected positioned clock')
    identities = re.findall(rb'(?m)^# ([^\n]+) \(device (\d+), inode (\d+)\):', output)
    if sorted(name.decode() for name, _, _ in identities) != sorted(directories):
        raise h.HarnessError(f'unexpected directory identities: {identities!r}')
    for name, device, inode in identities:
        physical = directories[name.decode()]
        if (int(device), int(inode)) != physical:
            raise h.HarnessError(f'wrong physical directory identity: {name!r}')
        output = output.replace(b'# ' + name + b' (device ' + device + b', inode ' + inode + b'):', b'# ' + name + b' <checked identity>:', 1)
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
        if updated:
            marker = b'#  Failed to be updated.' if rule in model.failed else b'#  Successfully updated.'
            if marker not in body.splitlines():
                raise h.HarnessError(f'{rule} expected {marker!r}')
    membership = re.findall(rb'(?m)^#  Also makes: (.*)$', section)
    expected_membership = [model.members.encode()] * model.group_records
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
    for path in work.rglob('*'):
        if path.is_file():
            stamp = mtimes.get(path.relative_to(work).as_posix(), FIXED)
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
    directories = {name: ((work / name).stat().st_dev, (work / name).stat().st_ino) for name in model.directories}
    delivered = False
    def observe(pid, output):
        nonlocal delivered
        if not delivered and output == b'ready\n':
            descriptor = os.pidfd_open(pid)
            try:
                signal.pidfd_send_signal(descriptor, signal.SIGTERM)
                delivered = True
            finally:
                os.close(descriptor)
    start = db.libc_time()
    out, err, status, limit, leftovers = h.capture(argv, work, h.clean_environment(runtime), case.timeout,
                                                  observe=observe if model.cancel else None)
    end = db.libc_time()
    filesystem = h.snapshot(roots)
    times = file_times(work)
    result = {'engine': str(engine), 'argv': argv, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(), 'status': status,
              'limit': limit, 'leftovers': leftovers, 'filesystem': filesystem, 'expected_filesystem': expected,
              'physical_mtime_ns': times, 'expected_mtime_ns': expected_times, 'libc_window': [start, end], 'physical_directories': directories, 'signal_delivered': delivered}
    try:
        if (status, err, limit, leftovers, filesystem, times) != (model.status, model.stderr.encode(), None, False, expected, expected_times):
            raise h.HarnessError('unexpected status, streams, process bound or physical file effect')
        if model.cancel and not delivered:
            raise h.HarnessError('missing recipe-ready cancellation witness')
        result['comparison_sha256'] = hashlib.sha256(check_stdout(model, out, start, end, directories)).hexdigest()
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
        parser.error('unknown grouped branch failure case')
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
