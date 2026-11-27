# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
from typing import NamedTuple


PRIOR_ECHO = b'./prior\n'
NEXT_ECHO = b'./next\n'
PRIOR_OUT = b'prior-out\n'
PRIOR_ERR = b'prior-err\n'
LAUNCH_ERROR = b'make[1]: ./next: No such file or directory\n'
PRIOR_ERROR = b'make[1]: ./prior: No such file or directory\n'
RECIPE_ERROR = b'make[1]: *** [worker.mk:3: all] Error 127\n'
IGNORED_ERROR = b'make[1]: [worker.mk:3: all] Error 127 (ignored)\n'
IGNORED_CHILD_ERROR = b'make[1]: [worker.mk:3: all] Error 1 (ignored)\n'
ORDINARY_COMMAND = b'printf "ordinary\\n" > ran\n'
CONTINUATION_COMMAND = b'printf "continued\\n"; printf "effect\\n" > ran\n'
CONTINUED = b'continued\n'
CONTINUATION = CONTINUATION_COMMAND + CONTINUED
FORCED_STDOUT = PRIOR_ECHO + PRIOR_OUT + NEXT_ECHO
FORCED_MERGED = PRIOR_ECHO + PRIOR_OUT + PRIOR_ERR + NEXT_ECHO
DIRECT_FATAL = LAUNCH_ERROR + PRIOR_ERROR + RECIPE_ERROR
DIRECT_IGNORED = LAUNCH_ERROR + PRIOR_ERROR + IGNORED_ERROR
NEXT_OUT = b'next-out\n'
NEXT_ERR = b'next-err\n'
NEXT_SCRIPT = (
    b'#!/bin/sh\nprintf "next-out\\n"\nprintf "next-err\\n" >&2\n'
    b'printf "effect\\n" > next-ran\nexit 1\n'
)
DRY = ('-n',)
RECURSIVE = ('-n', '-Orecurse')
QUERY = ('-q',)
QUERY_RECURSIVE = ('-q', '-Orecurse')


class Case(NamedTuple):
    name: str
    options: tuple[str, ...]
    first_prefix: bytes
    second_prefix: bytes
    remove_prior: bool
    status: int
    stdout: bytes
    stderr: bytes
    combined: bytes
    prior_status: int = 0
    next_executable: bool = False


cases = [
    Case('forced missing', DRY, b'+@', b'+@', True, 2,
         FORCED_STDOUT, PRIOR_ERR + DIRECT_FATAL, FORCED_MERGED + DIRECT_FATAL),
    Case('healthy prior control', DRY, b'+@', b'+@', False, 2,
         FORCED_STDOUT, PRIOR_ERR + LAUNCH_ERROR + RECIPE_ERROR,
         FORCED_MERGED + LAUNCH_ERROR + RECIPE_ERROR),
    Case('skipped second line control', DRY, b'+@', b'@', True, 0,
         FORCED_STDOUT + ORDINARY_COMMAND, PRIOR_ERR, FORCED_MERGED + ORDINARY_COMMAND),
    Case('ordinary dry run control', DRY, b'@', b'@', True, 0,
         PRIOR_ECHO + NEXT_ECHO + ORDINARY_COMMAND, b'',
         PRIOR_ECHO + NEXT_ECHO + ORDINARY_COMMAND),
    Case('direct ignored', DRY, b'+@', b'+-@', True, 0,
         FORCED_STDOUT + CONTINUATION, PRIOR_ERR + DIRECT_IGNORED,
         FORCED_MERGED + DIRECT_IGNORED + CONTINUATION),
    Case('direct ignored healthy control', DRY, b'+@', b'+-@', False, 0,
         FORCED_STDOUT + CONTINUATION, PRIOR_ERR + LAUNCH_ERROR + IGNORED_ERROR,
         FORCED_MERGED + LAUNCH_ERROR + IGNORED_ERROR + CONTINUATION),
    Case('recursive fatal', RECURSIVE, b'+@', b'+@', True, 2,
         FORCED_STDOUT, PRIOR_ERROR + PRIOR_ERR + LAUNCH_ERROR + RECIPE_ERROR,
         PRIOR_ERROR + FORCED_MERGED + LAUNCH_ERROR + RECIPE_ERROR),
    Case('recursive fatal healthy control', RECURSIVE, b'+@', b'+@', False, 2,
         FORCED_STDOUT, PRIOR_ERR + LAUNCH_ERROR + RECIPE_ERROR,
         FORCED_MERGED + LAUNCH_ERROR + RECIPE_ERROR),
    Case('recursive ignored', RECURSIVE, b'+@', b'+-@', True, 0,
         FORCED_STDOUT + CONTINUATION, PRIOR_ERROR + PRIOR_ERR + LAUNCH_ERROR + IGNORED_ERROR,
         PRIOR_ERROR + FORCED_MERGED + LAUNCH_ERROR + IGNORED_ERROR + CONTINUATION),
    Case('recursive ignored healthy control', RECURSIVE, b'+@', b'+-@', False, 0,
         FORCED_STDOUT + CONTINUATION, PRIOR_ERR + LAUNCH_ERROR + IGNORED_ERROR,
         FORCED_MERGED + LAUNCH_ERROR + IGNORED_ERROR + CONTINUATION),
    Case('recursive skipped second line control', RECURSIVE, b'+@', b'@', True, 0,
         FORCED_STDOUT + ORDINARY_COMMAND, PRIOR_ERR, FORCED_MERGED + ORDINARY_COMMAND),
    Case('serial target mode', ('-n', '-Otarget'), b'+@', b'+@', True, 2,
         FORCED_STDOUT, PRIOR_ERR + DIRECT_FATAL, FORCED_MERGED + DIRECT_FATAL),
    Case('serial line mode', ('-n', '-Oline'), b'+@', b'+@', True, 2,
         FORCED_STDOUT, PRIOR_ERR + DIRECT_FATAL, FORCED_MERGED + DIRECT_FATAL),
    Case('query forced missing', QUERY, b'+@', b'+@', True, 2,
         PRIOR_OUT, PRIOR_ERR + DIRECT_FATAL, PRIOR_OUT + PRIOR_ERR + DIRECT_FATAL),
    Case('query healthy prior control', QUERY, b'+@', b'+@', False, 2,
         PRIOR_OUT, PRIOR_ERR + LAUNCH_ERROR + RECIPE_ERROR,
         PRIOR_OUT + PRIOR_ERR + LAUNCH_ERROR + RECIPE_ERROR),
    Case('query first child exit 1', QUERY, b'+@', b'+@', True, 1,
         PRIOR_OUT, PRIOR_ERR, PRIOR_OUT + PRIOR_ERR, prior_status=1),
    Case('query next child exit 1', QUERY, b'+@', b'+@', True, 1,
         PRIOR_OUT + NEXT_OUT, PRIOR_ERR + NEXT_ERR,
         PRIOR_OUT + PRIOR_ERR + NEXT_OUT + NEXT_ERR, next_executable=True),
    Case('query skipped second line', QUERY, b'+@', b'@', True, 1,
         PRIOR_OUT, PRIOR_ERR, PRIOR_OUT + PRIOR_ERR),
    Case('query direct ignored', QUERY, b'+@', b'+-@', True, 0,
         PRIOR_OUT + CONTINUED, PRIOR_ERR + DIRECT_IGNORED,
         PRIOR_OUT + PRIOR_ERR + DIRECT_IGNORED + CONTINUED),
    Case('query direct ignored healthy control', QUERY, b'+@', b'+-@', False, 0,
         PRIOR_OUT + CONTINUED, PRIOR_ERR + LAUNCH_ERROR + IGNORED_ERROR,
         PRIOR_OUT + PRIOR_ERR + LAUNCH_ERROR + IGNORED_ERROR + CONTINUED),
    Case('query recursive fatal', QUERY_RECURSIVE, b'+@', b'+@', True, 2,
         PRIOR_OUT, PRIOR_ERROR + PRIOR_ERR + LAUNCH_ERROR + RECIPE_ERROR,
         PRIOR_ERROR + PRIOR_OUT + PRIOR_ERR + LAUNCH_ERROR + RECIPE_ERROR),
    Case('query recursive fatal healthy control', QUERY_RECURSIVE, b'+@', b'+@', False, 2,
         PRIOR_OUT, PRIOR_ERR + LAUNCH_ERROR + RECIPE_ERROR,
         PRIOR_OUT + PRIOR_ERR + LAUNCH_ERROR + RECIPE_ERROR),
    Case('query recursive ignored', QUERY_RECURSIVE, b'+@', b'+-@', True, 0,
         PRIOR_OUT + CONTINUED, PRIOR_ERROR + PRIOR_ERR + LAUNCH_ERROR + IGNORED_ERROR,
         PRIOR_ERROR + PRIOR_OUT + PRIOR_ERR + LAUNCH_ERROR + IGNORED_ERROR + CONTINUED),
    Case('query recursive ignored healthy control', QUERY_RECURSIVE, b'+@', b'+-@', False, 0,
         PRIOR_OUT + CONTINUED, PRIOR_ERR + LAUNCH_ERROR + IGNORED_ERROR,
         PRIOR_OUT + PRIOR_ERR + LAUNCH_ERROR + IGNORED_ERROR + CONTINUED),
    Case('query recursive first child exit 1', QUERY_RECURSIVE, b'+@', b'+@', True, 1,
         PRIOR_OUT, PRIOR_ERR, PRIOR_OUT + PRIOR_ERR, prior_status=1),
    Case('query recursive next child exit 1', QUERY_RECURSIVE, b'+@', b'+@', True, 1,
         PRIOR_OUT + NEXT_OUT, PRIOR_ERR + NEXT_ERR,
         PRIOR_OUT + PRIOR_ERR + NEXT_OUT + NEXT_ERR, next_executable=True),
    Case('query recursive ignored child exit 1', QUERY_RECURSIVE, b'+@', b'+-@', True, 0,
         PRIOR_OUT + NEXT_OUT + CONTINUED, PRIOR_ERR + NEXT_ERR + IGNORED_CHILD_ERROR,
         PRIOR_OUT + PRIOR_ERR + NEXT_OUT + NEXT_ERR + IGNORED_CHILD_ERROR + CONTINUED,
         next_executable=True),
    Case('query recursive skipped second line', QUERY_RECURSIVE, b'+@', b'@', True, 1,
         PRIOR_OUT, PRIOR_ERR, PRIOR_OUT + PRIOR_ERR),
]

# Forced lines have the same exact streams under an owned pipe. Target sync
# uses the direct streams, and Recurse keeps the prior diagnostic global.
serial_cases = {case.name: case for case in cases}
pipe_cases = (
    ('dry direct fatal', 'forced missing', ()),
    ('query direct fatal', 'query forced missing', ()),
    ('dry direct ignored', 'direct ignored', ()),
    ('query direct ignored', 'query direct ignored', ()),
    ('dry target fatal', 'forced missing', ('-Otarget',)),
    ('query target fatal', 'query forced missing', ('-Otarget',)),
    ('dry target ignored', 'direct ignored', ('-Otarget',)),
    ('query target ignored', 'query direct ignored', ('-Otarget',)),
    ('dry recursive fatal', 'recursive fatal', ()),
    ('query recursive fatal', 'query recursive fatal', ()),
    ('dry recursive ignored', 'recursive ignored', ()),
    ('query recursive ignored', 'query recursive ignored', ()),
    ('dry target healthy control', 'healthy prior control', ('-Otarget',)),
    ('query recursive healthy control', 'query recursive fatal healthy control', ()),
    ('query first child exit 1', 'query first child exit 1', ()),
    ('query next child exit 1', 'query next child exit 1', ()),
    ('dry skipped second line', 'skipped second line control', ()),
    ('query skipped second line', 'query skipped second line', ()),
)
for label, template, extra_options in pipe_cases:
    case = serial_cases[template]
    cases.append(case._replace(
        name=f'pipe {label}',
        options=('-j2', '--jobserver-style=pipe', *case.options, *extra_options),
    ))

failures = []
for case in cases:
    ignored = case.second_prefix == b'+-@'
    if ignored:
        tail = b'+@' + CONTINUATION_COMMAND
    elif '-q' in case.options:
        # The forced tail records any failure to stop at the query verdict.
        tail = b'+@' + ORDINARY_COMMAND
    else:
        tail = b'@' + ORDINARY_COMMAND
    for merged in (False, True):
        capture = 'merged' if merged else 'split'
        with tempfile.TemporaryDirectory(dir='.') as scratch:
            work = Path(scratch)
            tmp = work / 'tmp'
            tmp.mkdir()
            prior = work / 'prior'
            script = b'#!/bin/sh\nprintf "prior-out\\n"\nprintf "prior-err\\n" >&2\n'
            if case.remove_prior:
                script += b'rm -- "$0"\n'
            script += f'exit {case.prior_status}\n'.encode('ascii')
            prior.write_bytes(script)
            prior.chmod(0o755)
            next_path = work / 'next'
            if case.next_executable:
                next_path.write_bytes(NEXT_SCRIPT)
                next_path.chmod(0o755)
            worker = work / 'worker.mk'
            source = (
                b'all:\n\t' + case.first_prefix + b'./prior\n\t'
                + case.second_prefix + b'./next\n\t' + tail
            )
            worker.write_bytes(source)
            result = subprocess.run(
                [sys.argv[1], '-f', 'worker.mk', *case.options, 'all'],
                cwd=work, env=dict(os.environ, TMPDIR=str(tmp.resolve())),
                stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT if merged else subprocess.PIPE,
                timeout=2, umask=0o022,
            )
            if result.returncode not in (0, 1, 2):
                raise SystemExit(f'unexpected make outcome in {case.name} {capture}: {result!r}')
            expected = {'tmp', 'worker.mk'}
            files_match = not any(tmp.iterdir()) and worker.read_bytes() == source
            if not (case.remove_prior and case.first_prefix == b'+@'):
                expected.add('prior')
                files_match &= (prior.is_file() and prior.read_bytes() == script
                                and stat.S_IMODE(prior.stat().st_mode) == 0o755)
            if ignored:
                expected.add('ran')
                witness = work / 'ran'
                files_match &= (witness.is_file() and witness.read_bytes() == b'effect\n'
                                and stat.S_IMODE(witness.stat().st_mode) == 0o644)
            if case.next_executable:
                expected.update({'next', 'next-ran'})
                witness = work / 'next-ran'
                files_match &= (next_path.is_file() and next_path.read_bytes() == NEXT_SCRIPT
                                and stat.S_IMODE(next_path.stat().st_mode) == 0o755
                                and witness.is_file() and witness.read_bytes() == b'effect\n'
                                and stat.S_IMODE(witness.stat().st_mode) == 0o644)
            files_match &= {path.name for path in work.iterdir()} == expected
            expected_streams = (case.combined, b'') if merged else (case.stdout, case.stderr)
            if (result.returncode != case.status
                    or (result.stdout, result.stderr or b'') != expected_streams
                    or not files_match):
                failures.append(f'{case.name} {capture}: {result!r}, effects_match={files_match}')
            else:
                print(f'verified {case.name} {capture}')

if failures:
    print('\n'.join(failures), file=sys.stderr)
    raise SystemExit(1)
