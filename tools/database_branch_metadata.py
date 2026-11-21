#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare double-colon branch metadata, effects, and complete database bytes."""
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
CURRENT = FIXED + 10_000_000_000
NEW = FIXED + 20_000_000_000
WITNESS = FIXED + 40_125_000_000
CLOCK = re.compile(rb'(?m)^# (Make data base, printed on|Finished Make data base on) ([^\n]{24})$')
COMMAND = '#  Command line target.'
IMPLICIT = '#  Implicit rule search has not been done.'
IMPLICIT_DONE = '#  Implicit rule search has been done.'
STEM = "#  Implicit/static pattern stem: ''"
SECONDARY = '#  File is secondary (prerequisite of .SECONDARY).'
NOT_INTERMEDIATE = '#  File is a prerequisite of .NOTINTERMEDIATE.'
COMPLETE = ('#  Last modified 2020-09-13 12:26:50', '#  File has been updated.', '#  Successfully updated.')
AUTOMATIC = re.compile(rb'(?m)^# ([*<+%^?|@]) := (.*)$')
FILE_INPUTS = ('all', 'gate', 'n', 'z')
TARGET_SOURCE = 'all: LOCAL = scope\n'


def recipe(name):
    return f"@printf '%s\\n' 'values[$<][$+][$^][$?][$|][$(LOCAL)]' > seen-{name}; touch -d @1600000040.125 seen-{name}"


@dataclass(frozen=True)
class Model:
    id: str
    newer: tuple[str, ...]
    goal: bool = True
    declaration: str = ''
    special: str = ''
    late: bool = False
    pattern_tail: bool = False
    single_pattern: bool = False
    recipe_print: bool = False

    def recipe(self, name):
        command = recipe(name)
        return '@$(eval MAKEFLAGS += -p)' + command[1:] if self.recipe_print else command

    @property
    def makefile(self):
        if self.single_pattern:
            return f'all:: n | gate\nal%: ; {self.recipe("n")}\n'
        head = f'all:: n | gate ; {recipe("n")}\n'
        tail = (f'all:: z | gate\nal%: n ; {recipe("z")}\n' if self.pattern_tail
                else f'all:: z | gate ; {recipe("z")}\n')
        return (self.declaration + head + tail + self.special
                + ('MAKEFLAGS += -p\n' if self.late else ''))

    @property
    def flags(self):
        return ('-rR', '-s', '--no-print-directory', *(() if self.late or self.recipe_print else ('-p',)), '-f', 'Makefile',
                *(('all',) if self.goal else ()))

    def expected_record(self, ordinal, name):
        inferred = self.single_pattern or (self.pattern_tail and name == 'z')
        active = name in self.newer or inferred
        metadata = ((COMMAND,) if self.goal and ordinal == 0 else ())
        metadata += (IMPLICIT_DONE if inferred else IMPLICIT,)
        if active:
            metadata += ("#  Implicit/static pattern stem: 'l'" if inferred else STEM,)
        if self.special == '.NOTINTERMEDIATE: all\n':
            metadata += (NOT_INTERMEDIATE,)
        elif not active or self.special == '.SECONDARY: all\n':
            metadata += (SECONDARY,)
        # Executing a later double-colon recipe initializes an empty parent table
        # when the head had neither its own recipe expansion nor an assignment.
        local = ordinal == 0 and bool(self.declaration)
        if active:
            occupied = 9 if local else 8
            probes = 19 if local else 17
            if ordinal == 0 and ('z' in self.newer or self.pattern_tail):
                probes += 4
            stats = f'# Load={occupied}/32={28 if local else 25}%, Rehash=0, Collisions=2/{probes}={(200 + probes // 2) // probes}%'
            if self.recipe_print:
                # Live option refresh retains local MAKEFLAGS and MFLAGS too.
                stats = '# Load=10/32=31%, Rehash=0, Collisions=5/25=20%'
        elif ordinal == 0 and (self.newer or local):
            stats = '# Load=1/32=3%, Rehash=0, Collisions=0/6=0%' if local else '# Load=0/32=0%, Rehash=0, Collisions=0/4=0%'
        else:
            stats = None
        declaration = ("# makefile (from 'Makefile', line 1)", 'all: LOCAL = scope') if local else ()
        if self.recipe_print:
            declaration = ('# environment', 'all: MFLAGS = -prRs --no-print-directory',
                           '# makefile', 'all: MAKEFLAGS = prRs --no-print-directory')
        prerequisites = 'n' if self.single_pattern else 'n z' if inferred else name
        first = 'n' if inferred else name
        stem = 'l' if inferred else ''
        automatic = (('@', 'all'), ('*', stem), ('<', first), ('+', prerequisites), ('%', ''),
                     ('^', prerequisites), ('?', first), ('|', 'gate')) if active else ()
        recipe_line = ordinal + 1 + bool(self.declaration) + inferred
        return {
            'rule': f'all:: {prerequisites} | gate', 'metadata': metadata + COMPLETE,
            'stats': stats, 'declaration': declaration, 'automatic': automatic,
            'recipe': (f"#  recipe to execute (from 'Makefile', line {recipe_line}):",
                       '\t ' + self.recipe(name)),
        }


def models():
    yield Model('active-head-explicit', ('n',))
    yield Model('active-tail-explicit', ('z',))
    yield Model('active-head-default', ('n',), goal=False, late=True)
    yield Model('both-inactive', ())
    yield Model('both-active', ('n', 'z'))
    yield Model('secondary-both-branches', ('n',), special='.SECONDARY: all\n')
    yield Model('notintermediate-both-branches', ('n',), special='.NOTINTERMEDIATE: all\n')
    yield Model('active-head-local', ('n',), declaration=TARGET_SOURCE)
    yield Model('inactive-head-local', ('z',), declaration=TARGET_SOURCE)
    yield Model('active-tail-pattern-recipe', ('n',), pattern_tail=True)
    yield Model('single-inferred-branch-startup-print', ('n',), single_pattern=True)
    yield Model('single-inferred-branch-recipe-print', ('n',), single_pattern=True, recipe_print=True)


def describe(model):
    local = 'scope' if model.declaration else ''
    witnesses = {f'seen-{name}': f'values[{name}][{name}][{name}][{name}][gate][{local}]\n'
                 for name in model.newer}
    if model.pattern_tail:
        witnesses['seen-z'] = f'values[n][n z][n z][n][gate][{local}]\n'
    return {**asdict(model), 'makefile': model.makefile, 'flags': model.flags,
            'records': [model.expected_record(ordinal, name)
                        for ordinal, name in enumerate(('n',) if model.single_pattern else ('n', 'z'))],
            'input_mtimes': {'Makefile': FIXED, **{name: CURRENT if name == 'all' else NEW if name in model.newer else FIXED for name in FILE_INPUTS}},
            'witnesses': witnesses}


def fixture(model, path):
    path.mkdir()
    (path / 'Makefile').write_text(model.makefile)
    for name in FILE_INPUTS:
        (path / name).write_text(name + '\n')
    return h.Case(path, model.id, 'database.double-colon-branch-metadata', 'required', model.flags, 15, {}, {})


def check_stdout(plan, output, start, end, directory):
    fields = list(CLOCK.finditer(output))
    if [field[1] for field in fields] != [b'Make data base, printed on', b'Finished Make data base on']:
        raise h.HarnessError('expected a complete database dump')
    a, b = (db.parse_clock(field[2]) for field in fields)
    if not start <= a <= b <= end:
        raise h.HarnessError('unexpected positioned clock fields')
    if plan['pattern_tail']:
        device, inode = directory
        identity = f'# . (device {device}, inode {inode}):'.encode()
        if output.count(identity) != 1 or output.count(b'(device ') != 1:
            raise h.HarnessError('implicit search directory differs from independent stat')
        output = output.replace(identity, b'# . <checked identity>:', 1)
    elif b'(device ' in output:
        raise h.HarnessError('unexpected physical directory fields')
    header, footer = b'\n# Files\n\n', b'\n# files hash-table stats:\n'
    if output.count(header) != 1 or output.count(footer) != 1 or output.index(header) > output.index(footer):
        raise h.HarnessError('invalid file database section')
    section = output[output.index(header) + len(header):output.index(footer)]
    bodies = list(re.finditer(rb'(?m)^all::[^\n]*\n(?:[^\n]+\n)*?\t[^\n]*$', section))
    if len(bodies) != len(plan['records']):
        raise h.HarnessError(f'expected {len(plan["records"])} database branch records, observed {bodies!r}')
    if plan['single_pattern'] and re.search(rb'(?m)^\.SUFFIXES:', section):
        raise h.HarnessError('a selected implicit stem must not introduce a suffix file record')
    for record, body in zip(plan['records'], bodies, strict=True):
        lines = body[0].decode().splitlines()
        metadata = (lines[0], *(line for line in lines[1:] if line.startswith('#  ') and not line.startswith('#  recipe')))
        expected = (record['rule'], *record['metadata'])
        if metadata != expected:
            raise h.HarnessError(f'{record["rule"]} metadata differs: expected {expected!r}, observed {metadata!r}')
        if tuple(lines[-2:]) != record['recipe']:
            raise h.HarnessError(f'{record["rule"]} recipe and source differ: {lines[-2:]!r}')
        auto = tuple((name.decode(), value.decode()) for name, value in AUTOMATIC.findall(body[0]))
        if auto != record['automatic']:
            raise h.HarnessError(f'{record["rule"]} automatic bindings differ: {auto!r}')
        table = [line for line in lines if line.startswith(('# variable set hash-table stats:', '# Load='))]
        if table != (['# variable set hash-table stats:', record['stats']] if record['stats'] else []):
            raise h.HarnessError(f'{record["rule"]} local hash-table fields differ: {table!r}')
        opening = section[:body.start()].rsplit(b'\n\n', 1)[-1]
        actual_declaration = tuple(opening.decode().splitlines())
        if actual_declaration != record['declaration']:
            raise h.HarnessError(f'{record["rule"]} target-specific declaration differs: {actual_declaration!r}')
    return CLOCK.sub(lambda field: b'# ' + field[1] + b' <checked libc ctime>', output)


def run(plan, case, runtime, engine):
    h.reset_workspace(case, runtime, engine)
    work = runtime / 'work'
    for name, stamp in plan['input_mtimes'].items():
        os.utime(work / name, ns=(stamp, stamp))
    roots = {name: runtime / name for name in ('work', 'home', 'tmp')}
    expected = h.snapshot(roots)
    expected_times = dict(plan['input_mtimes'])
    for name, value in plan['witnesses'].items():
        content = value.encode()
        expected['work/' + name] = {'type': 'file', 'mode': 0o644, 'size': len(content),
                                   'sha256': hashlib.sha256(content).hexdigest(), 'content': base64.b64encode(content).decode()}
        expected_times[name] = WITNESS
    argv = [str(runtime / 'bin/make'), *case.args]
    directory_before = work.stat()
    start = db.libc_time()
    out, err, status, limit, leftovers = h.capture(argv, work, h.clean_environment(runtime), case.timeout)
    end = db.libc_time()
    filesystem = h.snapshot(roots)
    times = {path.name: path.stat().st_mtime_ns for path in work.iterdir() if path.is_file()}
    result = {'engine': str(engine), 'argv': argv, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(), 'status': status,
              'limit': limit, 'leftovers': leftovers, 'filesystem': filesystem, 'expected_filesystem': expected,
              'physical_mtime_ns': times, 'expected_mtime_ns': expected_times, 'libc_window': [start, end]}
    try:
        if (status, err, limit, leftovers, filesystem, times) != (0, b'', None, False, expected, expected_times):
            raise h.HarnessError('unexpected status, stderr, process bound or physical file effect')
        directory_after = work.stat()
        identity = directory_before.st_dev, directory_before.st_ino
        if identity != (directory_after.st_dev, directory_after.st_ino):
            raise h.HarnessError('workspace directory changed during database capture')
        result['workspace_identity'] = identity
        result['comparison_sha256'] = hashlib.sha256(check_stdout(plan, out, start, end, identity)).hexdigest()
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
        parser.error('unknown branch-metadata case')
    os.umask(0o022)
    oracle = options.oracle.absolute()
    profile = reference.verify(oracle, options.oracle_profile or oracle.parent.parent / 'profile.json')
    engines = [oracle, oracle, *(path.resolve() for path in options.candidate)]
    rows = []
    with tempfile.TemporaryDirectory(prefix='rmake-db-double-colon-') as scratch:
        path = Path(scratch)
        runtime = path / 'runtime'
        runtime.mkdir()
        for model in selected:
            plan = describe(model)
            case = fixture(model, path / model.id)
            observations = [run(plan, case, runtime, engine) for engine in engines]
            signature = observations[0].get('comparison_sha256')
            oracle_passed = signature is not None and observations[1].get('comparison_sha256') == signature
            passed = oracle_passed and all(entry.get('comparison_sha256') == signature for entry in observations)
            rows.append({'model': plan, 'oracle_passed': oracle_passed, 'passed': passed, 'observations': observations})
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
