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
ORDINARY_COMMAND = b'printf "ordinary\\n" > ran\n'

cases = (
    ('forced missing', b'+@', b'+@', True, 2,
     PRIOR_ECHO + PRIOR_OUT + NEXT_ECHO,
     PRIOR_ERR + LAUNCH_ERROR + PRIOR_ERROR + RECIPE_ERROR,
     PRIOR_ECHO + PRIOR_OUT + PRIOR_ERR + NEXT_ECHO + LAUNCH_ERROR + PRIOR_ERROR + RECIPE_ERROR),
    ('healthy prior control', b'+@', b'+@', False, 2,
     PRIOR_ECHO + PRIOR_OUT + NEXT_ECHO,
     PRIOR_ERR + LAUNCH_ERROR + RECIPE_ERROR,
     PRIOR_ECHO + PRIOR_OUT + PRIOR_ERR + NEXT_ECHO + LAUNCH_ERROR + RECIPE_ERROR),
    ('skipped second line control', b'+@', b'@', True, 0,
     PRIOR_ECHO + PRIOR_OUT + NEXT_ECHO + ORDINARY_COMMAND,
     PRIOR_ERR,
     PRIOR_ECHO + PRIOR_OUT + PRIOR_ERR + NEXT_ECHO + ORDINARY_COMMAND),
    ('ordinary dry run control', b'@', b'@', True, 0,
     PRIOR_ECHO + NEXT_ECHO + ORDINARY_COMMAND, b'',
     PRIOR_ECHO + NEXT_ECHO + ORDINARY_COMMAND),
)

failures = []
for name, first, second, removes, status, stdout, stderr, combined in cases:
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
            source = (
                b'all:\n\t' + first + b'./prior\n\t' + second + b'./next\n\t@'
                + ORDINARY_COMMAND
            )
            worker.write_bytes(source)
            result = subprocess.run(
                [sys.argv[1], '-f', 'worker.mk', '-n', 'all'],
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
