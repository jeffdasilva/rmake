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
OUTPUT_MODE = b'--output-sync=unsupported'
OUTPUT_MODE_ERROR = b"make: *** unknown output-sync type 'unsupported'.  Stop.\n"
FR_OUTPUT_MODE_ERROR = "make: *** type de output-sync «\u00a0unsupported\u00a0» inconnu. Arrêt.\n".encode()
DEBUG_MODE_ERROR = b"make: *** unknown debug level specification 'unknown'.  Stop.\n"
FR_DEBUG_MODE_ERROR = "make: *** niveau de débogage inconnu «\u00a0unknown\u00a0». Arrêt.\n".encode()
MUTEX_OPEN_ERROR = b'make: *** cannot open output sync mutex missing/lock: No such file or directory.  Stop.\n'


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
    yield Model('unknown-output-sync', (b'-Ounsupported',), OUTPUT_MODE_ERROR, usage=False)
    yield Model('output-mode-error-version-before', (b'--version', CLI, OUTPUT_MODE),
                OUTPUT_MODE_ERROR, usage=False, version=True, seen=b'cli-effect\n')
    yield Model('fr-output-mode-error-version-after', (OUTPUT_MODE, CLI, b'--version'),
                FR_OUTPUT_MODE_ERROR, usage=False, version=True, seen=b'cli-effect\n', french=True)
    yield Model('output-mode-error-without-version', (OUTPUT_MODE, CLI),
                OUTPUT_MODE_ERROR, usage=False, seen=b'cli-effect\n')
    yield Model('output-mode-error-overridden-before-version',
                (OUTPUT_MODE, b'--output-sync=none', b'--version', CLI),
                status=0, usage=False, version=True, seen=b'cli-effect\n')
    yield Model('inherited-output-mode-error-before-explicit-version', (b'--version', CLI),
                OUTPUT_MODE_ERROR, makeflags=OUTPUT_MODE, usage=False)
    gnu_first = rb'FIRST:=$$(file\ >>seen,gnu-first)'
    gnu_last = rb'LAST:=$$(file\ >>seen,gnu-last)'
    make_first = rb'FIRST:=$$(file\ >>seen,make-first)'
    make_last = rb'LAST:=$$(file\ >>seen,make-last)'
    next_source = b'$(file >>seen,next-source)'
    valid_mode = b'--output-sync=none'
    mutex = b'--sync-mutex=fnm:missing/lock'
    invalid_debug = b'--debug=unknown'
    yield Model('inherited-gnu-output-mode-version', (CLI,), OUTPUT_MODE_ERROR,
                gnumakeflags=b' '.join((b'--version', gnu_first, OUTPUT_MODE, gnu_last)),
                makeflags=b' '.join((next_source, invalid_debug)),
                usage=False, version=True, seen=b'gnu-first\ngnu-last\n')
    yield Model('fr-inherited-make-output-mode-version', (CLI,), FR_OUTPUT_MODE_ERROR,
                gnumakeflags=gnu_first,
                makeflags=b' '.join((OUTPUT_MODE, make_first, make_last, b'--version')),
                usage=False, version=True, seen=b'gnu-first\nmake-first\nmake-last\n', french=True)
    yield Model('inherited-version-then-make-output-mode', (CLI,), OUTPUT_MODE_ERROR,
                gnumakeflags=b' '.join((b'--version', gnu_first)),
                makeflags=b' '.join((make_first, OUTPUT_MODE, make_last)),
                usage=False, version=True, seen=b'gnu-first\nmake-first\nmake-last\n')
    yield Model('inherited-gnu-output-mode-before-later-version', (CLI,), OUTPUT_MODE_ERROR,
                gnumakeflags=b' '.join((gnu_first, OUTPUT_MODE, gnu_last)),
                makeflags=b' '.join((next_source, b'--version', valid_mode)),
                usage=False, seen=b'gnu-first\ngnu-last\n')
    yield Model('inherited-gnu-output-mode-version-with-mutex', (CLI,), OUTPUT_MODE_ERROR,
                gnumakeflags=b' '.join((gnu_first, b'--version', mutex, OUTPUT_MODE, gnu_last)),
                makeflags=next_source, usage=False, version=True, seen=b'gnu-first\ngnu-last\n')
    yield Model('fr-inherited-make-output-mode-version-with-mutex', (CLI,), FR_OUTPUT_MODE_ERROR,
                gnumakeflags=gnu_first,
                makeflags=b' '.join((make_first, b'--version', mutex, OUTPUT_MODE, make_last)),
                usage=False, version=True, seen=b'gnu-first\nmake-first\nmake-last\n', french=True)
    yield Model('inherited-gnu-output-mode-overridden-within-source',
                (b'SIDE:=$(file >>seen,cli-effect)',),
                gnumakeflags=b' '.join((gnu_first, b'--version', OUTPUT_MODE, gnu_last, valid_mode)),
                makeflags=b' '.join((next_source, make_last)), status=0, usage=False, version=True,
                seen=b'gnu-first\ngnu-last\nnext-source\nmake-last\ncli-effect\n')
    yield Model('inherited-lexical-error-before-output-mode', (CLI,), SHORT.replace(b'{argv}', b''),
                gnumakeflags=b' '.join((gnu_first, b'--version', b'-z', OUTPUT_MODE, gnu_last)),
                makeflags=next_source, version=True, seen=b'gnu-first\ngnu-last\n')
    yield Model('inherited-debug-error-before-output-mode', (CLI,), DEBUG_MODE_ERROR,
                gnumakeflags=b' '.join((gnu_first, invalid_debug, OUTPUT_MODE, gnu_last)),
                makeflags=b' '.join((next_source, b'--version')),
                usage=False, seen=b'gnu-first\ngnu-last\n')
    yield Model('inherited-gnu-debug-version-before-output-mode', (CLI,), DEBUG_MODE_ERROR,
                gnumakeflags=b' '.join((b'--version', gnu_first, invalid_debug, OUTPUT_MODE, mutex, gnu_last)),
                makeflags=next_source, usage=False, version=True, seen=b'gnu-first\ngnu-last\n')
    yield Model('fr-inherited-make-debug-error-version-after', (CLI,), FR_DEBUG_MODE_ERROR,
                gnumakeflags=gnu_first,
                makeflags=b' '.join((make_first, invalid_debug, make_last, b'--version')),
                usage=False, version=True, seen=b'gnu-first\nmake-first\nmake-last\n', french=True)
    yield Model('inherited-version-then-make-debug-error', (CLI,), DEBUG_MODE_ERROR,
                gnumakeflags=b' '.join((b'--version', gnu_first, b'--debug=none')),
                makeflags=b' '.join((make_first, invalid_debug, make_last)),
                usage=False, version=True, seen=b'gnu-first\nmake-first\nmake-last\n')
    yield Model('inherited-lexical-error-before-debug', (CLI,), SHORT.replace(b'{argv}', b''),
                gnumakeflags=b' '.join((b'--version', gnu_first, invalid_debug, mutex, gnu_last, b'-z')),
                makeflags=next_source, version=True, seen=b'gnu-first\ngnu-last\n')
    yield Model('explicit-debug-error-version-with-inherited-effects',
                (invalid_debug, b'SIDE:=$(file >>seen,cli-effect)', b'--version'), DEBUG_MODE_ERROR,
                gnumakeflags=gnu_first, makeflags=b' '.join((b'--output-sync=target', make_first)),
                usage=False, version=True, seen=b'gnu-first\nmake-first\ncli-effect\n')
    target_mode = b'--output-sync=target'
    yield Model('inherited-gnu-mutex-open-error-version-before', (CLI,), MUTEX_OPEN_ERROR,
                gnumakeflags=b' '.join((b'--version', gnu_first, target_mode, mutex, gnu_last)),
                makeflags=b' '.join((next_source, invalid_debug)),
                usage=False, version=True, seen=b'gnu-first\ngnu-last\n')
    yield Model('inherited-make-mutex-open-error-version-after', (CLI,), MUTEX_OPEN_ERROR,
                gnumakeflags=gnu_first,
                makeflags=b' '.join((make_first, mutex, target_mode, make_last, b'--version')),
                usage=False, version=True, seen=b'gnu-first\nmake-first\nmake-last\n')
    yield Model('inherited-version-then-make-mutex-open-error', (CLI,), MUTEX_OPEN_ERROR,
                gnumakeflags=b' '.join((b'--version', gnu_first, target_mode)),
                makeflags=b' '.join((make_first, mutex, make_last)),
                usage=False, version=True, seen=b'gnu-first\nmake-first\nmake-last\n')
    yield Model('inherited-mutex-open-error-before-later-version', (b'--version', CLI), MUTEX_OPEN_ERROR,
                gnumakeflags=b' '.join((gnu_first, target_mode, mutex, gnu_last)),
                makeflags=b' '.join((next_source, b'--version')),
                usage=False, seen=b'gnu-first\ngnu-last\n')
    yield Model('explicit-mutex-open-error-version-with-inherited-effects',
                (mutex, b'SIDE:=$(file >>seen,cli-effect)', b'--version'), MUTEX_OPEN_ERROR,
                gnumakeflags=gnu_first, makeflags=b' '.join((target_mode, make_first)),
                usage=False, version=True, seen=b'gnu-first\nmake-first\ncli-effect\n')
    source_failure = b'$(file >>seen,source-first)$(error stop-source)$(file >>seen,forbidden)'
    source_error = b'make: *** stop-source.  Stop.\n'
    yield Model('inherited-version-before-source-expansion-error', (CLI,), source_error,
                gnumakeflags=b' '.join((b'$(file >>seen,gnu-expanded)', gnu_first, b'--version')),
                makeflags=source_failure, usage=False, version=True,
                seen=b'gnu-expanded\ngnu-first\nsource-first\n')
    yield Model('fr-inherited-version-before-source-expansion-error', (CLI,),
                'make: *** stop-source. Arrêt.\n'.encode(),
                gnumakeflags=b' '.join((b'--version', gnu_first, target_mode)),
                makeflags=source_failure, usage=False, version=True, french=True,
                seen=b'gnu-first\nsource-first\n')
    yield Model('inherited-help-before-source-expansion-error', (CLI,), source_error,
                gnumakeflags=b' '.join((gnu_first, b'--help')),
                makeflags=source_failure, usage=False, seen=b'gnu-first\nsource-first\n')
    yield Model('version-text-in-failing-makeflags-expansion', (b'--version', CLI), source_error,
                gnumakeflags=gnu_first, makeflags=b'--version ' + source_failure,
                usage=False, seen=b'gnu-first\nsource-first\n')
    yield Model('version-text-in-failing-gnumakeflags-expansion', (CLI,), source_error,
                gnumakeflags=b'--version ' + source_failure, makeflags=next_source,
                usage=False, seen=b'source-first\n')
    yield Model('unknown-debug', (invalid_debug,), DEBUG_MODE_ERROR, usage=False)
    yield Model('invalid-then-assignment', (b'-z', CLI), SHORT, seen=b'cli-effect\n')
    yield Model('assignment-then-invalid', (CLI, b'-z'), SHORT, seen=b'cli-effect\n')
    yield Model('two-invalid-with-assignment', (b'-z', CLI, b'--absent'), SHORT + LONG, seen=b'cli-effect\n')
    yield Model('empty-directory-then-assignment', (b'-C', b'', CLI), empty(b'-C'), seen=b'cli-effect\n')
    yield Model('empty-shuffle-then-assignment', (b'--shuffle=', CLI), empty(b'--shuffle'),
                seen=b'cli-effect\n')
    yield Model('empty-shuffle-prefix', (b'--shuf=',), empty(b'--shuffle'))
    yield Model('empty-shuffle-in-gnu-source', (CLI,), empty(b'--shuffle'), gnumakeflags=b'--shuffle=')
    yield Model('empty-shuffle-in-make-source', (CLI,), empty(b'--shuffle'), makeflags=b'--shuffle=')
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
    yield Model('inherited-tail-long-include', (CLI,), empty(b'-I'),
                makeflags=b'--include-dir ')
    yield Model('inherited-tail-short-include', (CLI,), empty(b'-I'),
                gnumakeflags=b'rRI\t', makeflags=b'--debug=unknown')
    fr_include = "make: l'option «\u00a0-I\u00a0» a besoin d'une chaîne non vide comme argument\n".encode()
    yield Model('inherited-tail-fr-include', (), fr_include, makeflags=b'--include-dir\t ',
                french=True)
    yield Model('inherited-tail-eval', (CLI,), empty(b'-E'), makeflags=b'--eval ')
    yield Model('inherited-tail-directory', (), empty(b'-C'), gnumakeflags=b'--directory ')
    yield Model('inherited-tail-no-blank-control',
                errors=b": option '--include-dir' requires an argument\n",
                makeflags=b'--include-dir')
    yield Model('inherited-tail-attached-empty-control', (), fr_include,
                makeflags=b'--include-dir=', french=True)
    yield Model('inherited-tail-escaped-blank-control', makeflags=b'--include-dir \\ ',
                status=0, usage=False, goal=b'all')
    yield Model('inherited-tail-no-operand-control', makeflags=b'-s ',
                status=0, usage=False, goal=b'all')
    yield Model('inherited-tail-blank-only-control', gnumakeflags=b' \t ',
                status=0, usage=False, goal=b'all')
    yield Model('deferred-eval-skipped', (b'-z', b'-E', b'$(file >seen,deferred)'), SHORT)
    yield Model('literal-option-looking-goal', (b'--', b'-z'), status=0, usage=False, goal=b'-z')
    yield Model('explicit-dot-directory', (b'-C', b'.///'), status=0, usage=False, goal=b'all')
    yield Model('fr-empty-directory', (b'-C', b''), french=True)
    yield Model('fr-unknown-option', (b'-z',), SHORT, french=True)


def observe(model, binary, invoke, reset, locale, engine, make_host):
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
    expected_out = b'opened\ngoal|' + model.goal + b'\n' if model.goal else b''
    if model.version:
        banner_env = env | {'MAKEFLAGS': b'', 'GNUMAKEFLAGS': b''}
        banner, banner_error, banner_status, banner_limit, banner_children = h.capture(
            [str(invoke), '--version'], work, banner_env, 10)
        lines = banner.split(b'\n')
        if engine == 'candidate':
            banner_ok = len(lines) == 5 and lines[0].startswith(b'rmake ') and lines[1].startswith(
                b'GNU Make 4.4.1 compatibility target ')
        else:
            banner_ok = lines[0] == b'GNU Make 4.4.1'
        if (not banner_ok or lines[-1] or not lines[-2] or banner_error
                or (banner_status, banner_limit, banner_children) != (0, None, False)
                or h.snapshot(roots) != before):
            raise h.HarnessError('invalid isolated version banner')
        expected_out = banner + (b'\n' if model.usage else b'')
    out, err, status, limit, children = h.capture([str(invoke), '-rR', '--no-print-directory', *model.args],
                                                  work, env, 10)
    stdout_ok = out == expected_out
    expected_err = model.errors.replace(b'{argv}', os.fsencode(invoke))
    if model.usage:
        if model.french:
            # This body must be localized. GNU2 below also compares every byte.
            stderr_ok = err.startswith(expected_err) and b'Options' in err and USAGE not in err
        else:
            stderr_ok = err.startswith(expected_err + USAGE) and err.endswith(
                b'\nThis program built for ' + make_host + b'\nReport bugs to <bug-make@gnu.org>\n')
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
    make_host = profile['observed']['variables']['MAKE_HOST'].encode('ascii')
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
                        observations.append(observe(
                            model, binary.absolute(), root / 'make', reset, locale, name, make_host))
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
