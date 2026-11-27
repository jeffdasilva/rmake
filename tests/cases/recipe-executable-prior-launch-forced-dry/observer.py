# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile


PRIOR_ECHO = b'./prior\n'
NEXT_ECHO = b'./next\n'
PRIOR_OUT = b'prior-out\n'
PRIOR_ERR = b'prior-err\n'
LAUNCH_ERROR = b'make[1]: ./next: No such file or directory\n'
PRIOR_ERROR = b'make[1]: ./prior: No such file or directory\n'
RECIPE_ERROR = b'make[1]: *** [worker.mk:3: all] Error 127\n'
IGNORED_ERROR = b'make[1]: [worker.mk:3: all] Error 127 (ignored)\n'
ORDINARY_COMMAND = b'printf "ordinary\\n" > ran\n'
CONTINUATION_COMMAND = b'printf "continued\\n"; printf "effect\\n" > ran\n'
CONTINUATION = CONTINUATION_COMMAND + b'continued\n'
FORCED_STDOUT = PRIOR_ECHO + PRIOR_OUT + NEXT_ECHO
FORCED_MERGED = PRIOR_ECHO + PRIOR_OUT + PRIOR_ERR + NEXT_ECHO
DIRECT_FATAL = LAUNCH_ERROR + PRIOR_ERROR + RECIPE_ERROR
DIRECT_IGNORED = LAUNCH_ERROR + PRIOR_ERROR + IGNORED_ERROR
RECURSIVE = ('-Orecurse',)

cases = (
    ('forced missing', (), b'+@', b'+@', True, 2,
     FORCED_STDOUT, PRIOR_ERR + DIRECT_FATAL, FORCED_MERGED + DIRECT_FATAL),
    ('healthy prior control', (), b'+@', b'+@', False, 2,
     FORCED_STDOUT, PRIOR_ERR + LAUNCH_ERROR + RECIPE_ERROR,
     FORCED_MERGED + LAUNCH_ERROR + RECIPE_ERROR),
    ('skipped second line control', (), b'+@', b'@', True, 0,
     FORCED_STDOUT + ORDINARY_COMMAND, PRIOR_ERR, FORCED_MERGED + ORDINARY_COMMAND),
    ('ordinary dry run control', (), b'@', b'@', True, 0,
     PRIOR_ECHO + NEXT_ECHO + ORDINARY_COMMAND, b'',
     PRIOR_ECHO + NEXT_ECHO + ORDINARY_COMMAND),
    ('direct ignored', (), b'+@', b'+-@', True, 0,
     FORCED_STDOUT + CONTINUATION, PRIOR_ERR + DIRECT_IGNORED,
     FORCED_MERGED + DIRECT_IGNORED + CONTINUATION),
    ('direct ignored healthy control', (), b'+@', b'+-@', False, 0,
     FORCED_STDOUT + CONTINUATION, PRIOR_ERR + LAUNCH_ERROR + IGNORED_ERROR,
     FORCED_MERGED + LAUNCH_ERROR + IGNORED_ERROR + CONTINUATION),
    ('recursive fatal', RECURSIVE, b'+@', b'+@', True, 2,
     FORCED_STDOUT, PRIOR_ERROR + PRIOR_ERR + LAUNCH_ERROR + RECIPE_ERROR,
     PRIOR_ERROR + FORCED_MERGED + LAUNCH_ERROR + RECIPE_ERROR),
    ('recursive fatal healthy control', RECURSIVE, b'+@', b'+@', False, 2,
     FORCED_STDOUT, PRIOR_ERR + LAUNCH_ERROR + RECIPE_ERROR,
     FORCED_MERGED + LAUNCH_ERROR + RECIPE_ERROR),
    ('recursive ignored', RECURSIVE, b'+@', b'+-@', True, 0,
     FORCED_STDOUT + CONTINUATION, PRIOR_ERROR + PRIOR_ERR + LAUNCH_ERROR + IGNORED_ERROR,
     PRIOR_ERROR + FORCED_MERGED + LAUNCH_ERROR + IGNORED_ERROR + CONTINUATION),
    ('recursive ignored healthy control', RECURSIVE, b'+@', b'+-@', False, 0,
     FORCED_STDOUT + CONTINUATION, PRIOR_ERR + LAUNCH_ERROR + IGNORED_ERROR,
     FORCED_MERGED + LAUNCH_ERROR + IGNORED_ERROR + CONTINUATION),
    ('recursive skipped second line control', RECURSIVE, b'+@', b'@', True, 0,
     FORCED_STDOUT + ORDINARY_COMMAND, PRIOR_ERR, FORCED_MERGED + ORDINARY_COMMAND),
    ('serial target mode', ('-Otarget',), b'+@', b'+@', True, 2,
     FORCED_STDOUT, PRIOR_ERR + DIRECT_FATAL, FORCED_MERGED + DIRECT_FATAL),
    ('serial line mode', ('-Oline',), b'+@', b'+@', True, 2,
     FORCED_STDOUT, PRIOR_ERR + DIRECT_FATAL, FORCED_MERGED + DIRECT_FATAL),
)

failures = []
for name, options, first, second, removes, status, stdout, stderr, combined in cases:
    ignored = second == b'+-@'
    tail = b'+@' + CONTINUATION_COMMAND if ignored else b'@' + ORDINARY_COMMAND
    for merged in (False, True):
        capture = 'merged' if merged else 'split'
        with tempfile.TemporaryDirectory(dir='.') as scratch:
            work = Path(scratch)
            tmp = work / 'tmp'
            tmp.mkdir()
            prior = work / 'prior'
            script = b'#!/bin/sh\nprintf "prior-out\\n"\nprintf "prior-err\\n" >&2\n'
            if removes:
                script += b'rm -- "$0"\n'
            script += b'exit 0\n'
            prior.write_bytes(script)
            prior.chmod(0o755)
            worker = work / 'worker.mk'
            source = b'all:\n\t' + first + b'./prior\n\t' + second + b'./next\n\t' + tail
            worker.write_bytes(source)
            result = subprocess.run(
                [sys.argv[1], '-f', 'worker.mk', '-n', *options, 'all'],
                cwd=work, env=dict(os.environ, TMPDIR=str(tmp.resolve())),
                stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT if merged else subprocess.PIPE,
                timeout=2, umask=0o022,
            )
            if result.returncode not in (0, 2):
                raise SystemExit(f'unexpected make outcome in {name} {capture}: {result!r}')
            expected = {'tmp', 'worker.mk'}
            files_match = not any(tmp.iterdir()) and worker.read_bytes() == source
            if not (removes and first == b'+@'):
                expected.add('prior')
                files_match &= (prior.is_file() and prior.read_bytes() == script
                                and stat.S_IMODE(prior.stat().st_mode) == 0o755)
            if ignored:
                expected.add('ran')
                witness = work / 'ran'
                files_match &= (witness.is_file() and witness.read_bytes() == b'effect\n'
                                and stat.S_IMODE(witness.stat().st_mode) == 0o644)
            files_match &= {path.name for path in work.iterdir()} == expected
            expected_streams = (combined, b'') if merged else (stdout, stderr)
            if (result.returncode != status or (result.stdout, result.stderr or b'') != expected_streams
                    or not files_match):
                failures.append(f'{name} {capture}: {result!r}, effects_match={files_match}')
            else:
                print(f'verified {name} {capture}')

if failures:
    print('\n'.join(failures), file=sys.stderr)
    raise SystemExit(1)
