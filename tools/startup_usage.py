#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original strict usage, phase, and filesystem comparisons with GNU Make."""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
import os
from pathlib import Path
import tempfile

import conformance as h
import locale_data
import oracle

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/startup-usage/Makefile'
CLI = b'SIDE:=$(file >seen,cli-effect)'
INHERITED = b'SIDE:=$$(file\\ >seen,inherited-effect)'
SHORT = b'{argv}: invalid option -- \'z\'\n'
LONG = b"{argv}: unrecognized option '--absent'\n"
JOBS = b"make: the '-j' option requires a positive integer argument\n"
USAGE = b'Usage: make [options] [target] ...\nOptions:\n'


@dataclass(frozen=True)
class Model:
    id: str
    args: tuple[bytes, ...] = ()
    errors: bytes = b''
    makeflags: bytes = b''
    gnumakeflags: bytes = b''
    status: int = 2
    usage: bool = True
    version: bool = False
    seen: bytes | None = None
    goal: bytes | None = None
    french: bool = False


def empty(option):
    return b"make: the '" + option + b"' option requires a non-empty string argument\n"


def models():
    for letter in b'CfIEWo':
        opt = b'-' + bytes([letter])
        yield Model('empty-short-' + opt[1:].decode(), (opt, b''), empty(opt))
    for name, opt in ((b'--directory', b'-C'), (b'--makefile', b'-f'),
                      (b'--include-dir', b'-I'), (b'--eval', b'-E'),
                      (b'--assume-new', b'-W'), (b'--assume-old', b'-o')):
        yield Model('empty-' + name[2:].decode(), (name + b'=',), empty(opt))
    yield Model('missing-short', (b'-f',), b"{argv}: option requires an argument -- 'f'\n")
    yield Model('missing-long', (b'--include-dir',), b"{argv}: option '--include-dir' requires an argument\n")
    yield Model('unknown-short', (b'-z',), SHORT)
    yield Model('unknown-long', (b'--absent',), LONG)
    yield Model('unknown-raw-long', (b'--\xff',), b"{argv}: unrecognized option '--\xff'\n")
    yield Model('ambiguous-long', (b'--w',),
                b"{argv}: option '--w' is ambiguous; possibilities: '--what-if' '--warn-undefined-variables'\n")
    yield Model('unexpected-long-value', (b'--quiet=yes',),
                b"{argv}: option '--quiet' doesn't allow an argument\n")
    yield Model('zero-jobs', (b'-j0',), JOBS)
    yield Model('empty-long-jobs', (b'--jobs=',), JOBS)
    yield Model('empty-separated-jobs', (b'-j', b''), JOBS)
    yield Model('unknown-output-sync', (b'-Ounsupported',),
                b"make: *** unknown output-sync type 'unsupported'.  Stop.\n", usage=False)
    yield Model('unknown-debug', (b'--debug=unknown',),
                b"make: *** unknown debug level specification 'unknown'.  Stop.\n", usage=False)
    yield Model('invalid-then-assignment', (b'-z', CLI), SHORT, seen=b'cli-effect\n')
    yield Model('assignment-then-invalid', (CLI, b'-z'), SHORT, seen=b'cli-effect\n')
    yield Model('two-invalid-with-assignment', (b'-z', CLI, b'--absent'), SHORT + LONG, seen=b'cli-effect\n')
    yield Model('empty-directory-then-assignment', (b'-C', b'', CLI), empty(b'-C'), seen=b'cli-effect\n')
    yield Model('invalid-then-help', (b'-z', b'--help'), SHORT)
    yield Model('help-then-invalid', (b'--help', b'-z'), SHORT)
    yield Model('invalid-with-version', (b'--version', b'-C', b''), empty(b'-C'), version=True)
    yield Model('invalid-with-explicit-output-sync', (b'-Otarget', b'-z', CLI), SHORT, seen=b'cli-effect\n')
    yield Model('invalid-with-inherited-output-sync', (b'-z', CLI), SHORT,
                makeflags=b'-Otarget', seen=b'cli-effect\n')
    yield Model('clustered-invalid-letters', (b'-zZY', CLI), SHORT +
                b"{argv}: invalid option -- 'Z'\n{argv}: invalid option -- 'Y'\n", seen=b'cli-effect\n')
    yield Model('failed-inherited-source-still-defines', (CLI,), SHORT.replace(b'{argv}', b''),
                makeflags=b'-z ' + INHERITED, seen=b'inherited-effect\n')
    yield Model('failed-gnu-source-stops-following-sources', (CLI,), SHORT.replace(b'{argv}', b''),
                makeflags=INHERITED, gnumakeflags=b'-z')
    yield Model('deferred-eval-skipped', (b'-z', b'-E', b'$(file >seen,deferred)'), SHORT)
    yield Model('literal-option-looking-goal', (b'--', b'-z'), status=0, usage=False, goal=b'-z')
    yield Model('explicit-dot-directory', (b'-C', b'.///'), status=0, usage=False, goal=b'all')
    yield Model('fr-empty-directory', (b'-C', b''), french=True)
    yield Model('fr-unknown-option', (b'-z',), SHORT, french=True)


def observe(model, binary, invoke, reset, locale, engine):
    reset.mkdir()
    work, home, temporary = (reset / name for name in ('work', 'home', 'tmp'))
    for directory in (work, home, temporary):
        directory.mkdir()
    source = FIXTURE.read_bytes()
    (work / 'Makefile').write_bytes(source)
    roots = {'work': work, 'home': home, 'tmp': temporary}
    before = h.snapshot(roots)
    invoke.unlink(missing_ok=True)
    invoke.symlink_to(binary)
    env = oracle.clean_environment(home) | {
        'MAKEFLAGS': model.makeflags, 'GNUMAKEFLAGS': model.gnumakeflags,
        'TMPDIR': str(temporary),
    }
    if model.french:
        env |= locale['environment'] | {'LC_ALL': 'fr_FR.UTF-8'}
    out, err, status, limit, children = h.capture([str(invoke), '-rR', '--no-print-directory', *model.args],
                                                  work, env, 10)
    expected_out = b'opened\ngoal|' + model.goal + b'\n' if model.goal else b''
    if model.version:
        lines = out.split(b'\n')
        if engine == 'candidate':
            stdout_ok = len(lines) == 6 and lines[0].startswith(b'rmake ') and lines[1].startswith(
                b'GNU Make 4.4.1 compatibility target ') and lines[-2:] == [b'', b'']
        else:
            stdout_ok = lines[0] == b'GNU Make 4.4.1' and lines[-2:] == [b'', b'']
    else:
        stdout_ok = out == expected_out
    expected_err = model.errors.replace(b'{argv}', os.fsencode(invoke))
    if model.usage:
        if model.french:
            # This body must be localized. GNU2 below also compares every byte.
            stderr_ok = err.startswith(expected_err) and b'Options' in err and USAGE not in err
        else:
            stderr_ok = err.startswith(expected_err + USAGE) and err.endswith(
                b'\nThis program built for x86_64-pc-linux-gnu\nReport bugs to <bug-make@gnu.org>\n')
    else:
        stderr_ok = err == expected_err
    present = (work / 'seen').read_bytes() if (work / 'seen').is_file() else None
    after = h.snapshot(roots)
    stable_files = dict(after)
    stable_files.pop('work/seen', None)
    conditions = (stdout_ok, stderr_ok, (status, limit, children) == (model.status, None, False),
                  present == model.seen, stable_files == before)
    return {'stdout_hex': out.hex(), 'stderr_hex': err.hex(), 'status': status, 'limit': limit,
            'leftover_processes': children, 'files': after,
            'model_errors': [i for i, valid in enumerate(conditions) if not valid]}


def compare(model, observations):
    if any(row['model_errors'] for row in observations):
        return False
    if observations[0] != observations[1]:
        return False
    # Only the separately verified version banner can differ.
    if model.version:
        return all(observations[0][key] == observations[2][key]
                   for key in observations[0] if key != 'stdout_hex')
    return observations[0] == observations[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=oracle.PREFIX / 'bin/make')
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--locale-root', type=Path, default=ROOT / '.tools/locale-data')
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--case')
    args = parser.parse_args()
    selected = [model for model in models() if args.case is None or args.case == model.id]
    if not selected:
        parser.error(f'unknown startup usage case: {args.case}')
    profile = oracle.verify(args.oracle)
    locale = locale_data.provision(args.locale_root.resolve()) if any(model.french for model in selected) else None
    rows = []
    previous_umask = os.umask(0o022)
    try:
        with tempfile.TemporaryDirectory(prefix='rmake-startup-usage-') as directory:
            root = Path(directory)
            reset = root / 'reset'
            for model in selected:
                observations = []
                for name, binary in (('gnu-first', args.oracle), ('gnu-second', args.oracle),
                                     ('candidate', args.candidate)):
                    try:
                        observations.append(observe(model, binary.absolute(), root / 'make', reset, locale, name))
                    finally:
                        h.remove_tree(reset)
                passed = compare(model, observations)
                print('PASS' if passed else 'FAIL', model.id,
                      [row['model_errors'] for row in observations], flush=True)
                rows.append({'id': model.id, 'pass': passed, 'observations': observations})
    finally:
        os.umask(previous_umask)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    passed = sum(row['pass'] for row in rows)
    args.report.write_text(json.dumps({'cases': len(rows), 'passed': passed, 'results': rows,
                                      'oracle_profile': profile['profile_id'],
                                      'candidate_sha256': h.sha256_file(args.candidate)}, indent=2) + '\n')
    print(f'Startup usage gate: {passed}/{len(rows)}', flush=True)
    return int(passed != len(rows))


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (h.HarnessError, oracle.OracleError, OSError) as error:
        raise SystemExit(f'startup usage gate: {error}') from error
