#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Exact GNU2 comparisons for empty switch values and delayed style validation."""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
import os
from pathlib import Path
import tempfile

import conformance as h
import oracle

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/runtime-option-values/Makefile'
ORDINARY = b'pre\npost|\ngoal\n'
JOBS = b"make: the '-j' option requires a positive integer argument\n"
USAGE = b'Usage: make [options] [target] ...\nOptions:\n'
JOB_PROBE = b'$(info jobs|$(filter -j%,$(MAKEFLAGS)))'
ORDERED = rb'BEFORE:=$$(warning\ before) --output-sync= AFTER:=$$(warning\ after)'


@dataclass(frozen=True)
class Model:
    id: str
    args: tuple[bytes, ...] = ()
    directives: bytes = b''
    makeflags: bytes = b''
    stdout: bytes = ORDINARY
    stderr: bytes = b''
    status: int = 0
    usage: bool = False


def empty(option):
    return b"make: the '" + option + b"' option requires a non-empty string argument\n"


def style(value):
    return b"make: *** unknown jobserver auth style '" + value + b"'.  Stop.\n"


def models():
    for name, opt in ((b'output-sync', b'-O'), (b'debug', b'--debug'),
                      (b'shuffle', b'--shuffle'), (b'jobserver-style', b'--jobserver-style'),
                      (b'jobserver-auth', b'--jobserver-auth'), (b'jobserver-fds', b'--jobserver-fds'),
                      (b'sync-mutex', b'--sync-mutex')):
        value = b'--' + name + b'='
        yield Model('argv-empty-' + name.decode(), (value,), stdout=b'', stderr=empty(opt), status=2, usage=True)
        yield Model('reader-empty-' + name.decode(), directives=b'MAKEFLAGS = ' + value, stderr=empty(opt))
    for opt in (b'--jobserver-style', b'--sync-mutex'):
        yield Model('separate-empty-' + opt[2:].decode(), (opt, b''), stdout=b'', stderr=empty(opt), status=2, usage=True)
    yield Model('jobs-leading-blank', (b'--jobs= 2',), JOB_PROBE, stdout=b'pre\njobs|-j2\npost|\ngoal\n')
    yield Model('jobs-plus', (b'-j+2',), JOB_PROBE, stdout=b'pre\njobs|-j2\npost|\ngoal\n')
    yield Model('jobs-trailing-blank', (b'--jobs=2 ',), stdout=b'', stderr=JOBS, status=2, usage=True)
    yield Model('jobs-overflow', (b'--jobs=4294967296', b'--help'), stdout=b'', stderr=JOBS, status=2, usage=True)
    yield Model('inactive-argv-style', (b'--jobserver-style=bogus',))
    yield Model('active-argv-style', (b'-j2', b'--jobserver-style=bogus'), stdout=b'pre\npost|\n', stderr=style(b'bogus'), status=2)
    yield Model('later-valid-style', (b'-j2', b'--jobserver-style=bogus', b'--jobserver-style=pipe'))
    yield Model('ordered-immediate', directives=b'MAKEFLAGS = ' + ORDERED,
                stderr=b'Makefile:4: before\n' + empty(b'-O') + b'Makefile:4: after\n')
    yield Model('ordered-passive', directives=b'GNUMAKEFLAGS = ' + ORDERED,
                stderr=b'make: before\n' + empty(b'-O') + b'make: after\n')
    yield Model('contextual-include', directives=(b'HOME = $(CURDIR)/includes\nMAKEFLAGS = -I~ '
                                                + ORDERED + b'\ninclude carried.mk'),
                stdout=b'pre\npost|included\ngoal\n',
                stderr=b'Makefile:5: before\n' + empty(b'-O') + b'Makefile:5: after\n')
    yield Model('notice-before-fatal', directives=b'MAKEFLAGS = --output-sync= --output-sync=bogus',
                stdout=b'pre\n', stderr=empty(b'-O') + b"make: *** unknown output-sync type 'bogus'.  Stop.\n", status=2)
    yield Model('two-ordered-notices', directives=rb'MAKEFLAGS = --output-sync= J:=$$(warning\ middle) --shuffle=',
                stderr=empty(b'-O') + b'Makefile:4: middle\n' + empty(b'--shuffle'))
    yield Model('active-reader-style', directives=b'MAKEFLAGS = -j2 --jobserver-style=bogus',
                stdout=b'pre\npost|\n', stderr=style(b'bogus'), status=2)
    yield Model('inactive-reader-style', directives=b'MAKEFLAGS = -j1 --jobserver-style=bogus')
    yield Model('ignored-empty-file-ages', directives=b'MAKEFLAGS = --old-file= --what-if= --directory= --file=')
    yield Model('reader-eval-empty', directives=b'MAKEFLAGS = --eval= -s', stderr=empty(b'-E'))
    yield Model('reader-job-zero', directives=b'MAKEFLAGS = --jobs=0 -s', stderr=JOBS)
    yield Model('inherited-empty-then-cli', (b'X:=$(file >seen,explicit)',), makeflags=b'--output-sync=',
                stdout=b'', stderr=empty(b'-O'), status=2, usage=True)
    yield Model('raw-invalid-style', (b'-j2', b'--jobserver-style=\xff'), stdout=b'pre\npost|\n',
                stderr=style(b'\xff'), status=2)


def run(model, binary, engine, reset):
    reset.mkdir()
    work, home, temporary = (reset / name for name in ('work', 'home', 'tmp'))
    for folder in (work, home, temporary, work / 'includes'):
        folder.mkdir()
    (work / 'Makefile').write_bytes(FIXTURE.read_bytes().replace(b'# CASE-DIRECTIVES', model.directives))
    (work / 'includes/carried.mk').write_bytes(b'X := included\n')
    files = {'work': work, 'home': home, 'tmp': temporary}
    before = h.snapshot(files)
    engine.unlink(missing_ok=True)
    engine.symlink_to(binary)
    environment = oracle.clean_environment(home) | {
        'TMPDIR': str(temporary), 'MAKEFLAGS': model.makeflags, 'GNUMAKEFLAGS': b'',
    }
    out, err, status, limit, children = h.capture([str(engine), '-rR', '--no-print-directory', *model.args],
                                                  work, environment, 10)
    after = h.snapshot(files)
    stderr_ok = err == model.stderr if not model.usage else (
        err.startswith(model.stderr + USAGE) and err.endswith(b'Report bugs to <bug-make@gnu.org>\n'))
    errors = [name for name, result in (
        ('stdout', out == model.stdout), ('stderr', stderr_ok), ('filesystem', before == after),
        ('status', (status, limit, children) == (model.status, None, False)),
    ) if not result]
    return {'stdout_hex': out.hex(), 'stderr_hex': err.hex(), 'status': status, 'limit': limit,
            'leftover_processes': children, 'filesystem': after, 'errors': errors}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=oracle.PREFIX / 'bin/make')
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--case')
    args = parser.parse_args()
    selected = [model for model in models() if args.case is None or args.case == model.id]
    if not selected:
        parser.error(f'unknown option value case: {args.case}')
    profile = oracle.verify(args.oracle)
    results = []
    previous_umask = os.umask(0o022)
    try:
        with tempfile.TemporaryDirectory(prefix='rmake-runtime-option-values-') as directory:
            root = Path(directory)
            for model in selected:
                rows = []
                reset = root / 'reset'
                for executable in (args.oracle, args.oracle, args.candidate):
                    try:
                        rows.append(run(model, executable.absolute(), root / 'make', reset))
                    finally:
                        h.remove_tree(reset)
                passed = not rows[0]['errors'] and rows[0] == rows[1] == rows[2]
                print('PASS' if passed else 'FAIL', model.id, [row['errors'] for row in rows], flush=True)
                results.append({'id': model.id, 'pass': passed, 'observations': rows})
    finally:
        os.umask(previous_umask)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    passed = sum(row['pass'] for row in results)
    args.report.write_text(json.dumps({'cases': len(results), 'passed': passed, 'results': results,
                                      'oracle_profile': profile['profile_id'],
                                      'candidate_sha256': h.sha256_file(args.candidate)}, indent=2) + '\n')
    print(f'Runtime option values gate: {passed}/{len(results)}', flush=True)
    return int(passed != len(results))


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (h.HarnessError, oracle.OracleError, OSError) as error:
        raise SystemExit(f'runtime option values gate: {error}') from error
