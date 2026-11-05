#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original strict GNU2 database dry-run time and physical file witnesses."""
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

ROOT = Path(__file__).resolve().parents[1]
import conformance as h
import database_output as db
import oracle as reference

FIXED = 1_600_000_000_000_000_000
SYNTHETIC = '#  Last modified 2514-05-30 01:53:03.107374182'
MISSING = '#  File does not exist.'
OLD_PHYSICAL = '#  Last modified 2020-09-13 12:26:50'
CLOCK = re.compile(rb'(?m)^# (Make data base, printed on|Finished Make data base on) ([^\n]{24})$')
PRINT = "printf '%s\\n' skipped > all"
RECIPE = 'all: input ; @' + PRINT + '\n'
RECURSIVE = "printf '%s\\n' actual > all; touch -d @1600000005.123456789 all"


@dataclass(frozen=True)
class Model:
    id: str
    makefile: str
    records: tuple[tuple[str, str], ...]
    flags: tuple[str, ...] = ('-n',)
    expected_echo: tuple[str, ...] = ()
    target_before: int | None = None
    target_after: int | None = None
    status: int = 0


def models():
    rule = 'all: input'
    yield Model('missing-dry-run', RECIPE, ((rule, SYNTHETIC),), expected_echo=(PRINT,))
    yield Model('existing-stale-dry-run', RECIPE, ((rule, SYNTHETIC),), expected_echo=(PRINT,), target_before=FIXED - 10**10)
    yield Model('existing-current-dry-run', RECIPE, ((rule, OLD_PHYSICAL),), target_before=FIXED + 10**10)
    yield Model('phony-dry-run', '.PHONY: all\n' + RECIPE, ((rule, MISSING),), expected_echo=(PRINT,))
    yield Model('empty-recipe-dry-run', 'all: input ;\n', ((rule, SYNTHETIC),))
    yield Model('question-unchanged', RECIPE, ((rule, MISSING),), flags=('-q',), status=1)
    yield Model('forced-recipe-physical-time', 'all: input ; +@' + RECURSIVE + '\n',
                ((rule, '#  Last modified 2020-09-13 12:26:45.123456789'),), expected_echo=(RECURSIVE,),
                target_after=FIXED + 5_123_456_789)
    child = "printf '%s\\n' skipped-child > child"
    yield Model('parent-sees-dry-run-child', 'all: child ; @' + PRINT + '\nchild: input ; @' + child + '\n',
                (('all: child', SYNTHETIC), ('child: input', SYNTHETIC)), expected_echo=(child, PRINT))
    yield Model('late-reader-dry-run', RECIPE + 'MAKEFLAGS += -n\n', ((rule, SYNTHETIC),), flags=(), expected_echo=(PRINT,))
    yield Model('late-recipe-dry-run', 'all: input ; @$(eval MAKEFLAGS += -n)' + PRINT + '\n',
                ((rule, SYNTHETIC),), flags=(), expected_echo=(PRINT,))


def fixture(model, path):
    path.mkdir()
    (path / 'Makefile').write_text(model.makefile)
    (path / 'input').write_bytes(b'known input\n')
    if model.target_before is not None:
        (path / 'all').write_bytes(b'physical target\n')
    args = ('-rR', '-s', '--no-print-directory', '-p', '-f', 'Makefile', *model.flags, 'all')
    return h.Case(path, model.id, 'database.dry-run-time', 'required', args, 15, {}, {})


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
    prefix = output[:fields[0].start()].splitlines()
    commands = tuple(line.decode() for line in prefix if line.startswith(b'printf '))
    if commands != model.expected_echo:
        raise h.HarnessError(f'unexpected visible recipe commands: {commands!r}')
    return CLOCK.sub(lambda field: b'# ' + field[1] + b' <checked libc ctime>', output)


def run(model, case, runtime, engine):
    h.reset_workspace(case, runtime, engine)
    work = runtime / 'work'
    for path in work.iterdir():
        stamp = model.target_before if path.name == 'all' else FIXED
        os.utime(path, ns=(stamp, stamp))
    roots = {name: runtime / name for name in ('work', 'home', 'tmp')}
    expected = h.snapshot(roots)
    expected_times = file_times(work)
    if model.target_after is not None:
        content = b'actual\n'
        expected['work/all'] = {'type': 'file', 'mode': 0o644, 'size': len(content),
                                'sha256': hashlib.sha256(content).hexdigest(), 'content': base64.b64encode(content).decode()}
        expected_times['all'] = model.target_after
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
    parser.add_argument('--oracle', type=Path, default=reference.PREFIX / 'bin/make')
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--case')
    args = parser.parse_args()
    selected = [model for model in models() if not args.case or args.case == model.id]
    if not selected:
        parser.error(f'unknown case: {args.case}')
    os.umask(0o022)
    oracle = args.oracle.absolute()
    reference.verify(oracle)
    engines = [oracle, oracle, args.candidate.absolute()]
    rows = []
    with tempfile.TemporaryDirectory(prefix='rmake-db-dry-run-') as scratch:
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
    data = {'passed': all(row['passed'] for row in rows), 'oracle_passed': all(row['oracle_passed'] for row in rows),
            'rows': rows, 'source_sha256': h.sha256_file(Path(__file__)), 'affinity': sorted(os.sched_getaffinity(0)),
            'identities': [h.executable_identity(engine, oracle=i < 2) for i, engine in enumerate(engines)]}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    with args.report.open('w') as stream:
        json.dump(data, stream, indent=2)
    return 0 if data['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
