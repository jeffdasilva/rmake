# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

import errno
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile


# Linux allows 32 base pages for one argument, including the terminator.
# The fixture covers base page sizes through 64 KiB without storing that input.
argument_size = os.sysconf('SC_PAGE_SIZE') * 32
if not 0 < argument_size <= 2 * 1024 * 1024:
    raise SystemExit('unsupported Linux page size for the argument-limit fixture')
argument = b'x' * argument_size
try:
    subprocess.run(
        [b'/usr/bin/true', argument], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        stdin=subprocess.DEVNULL, timeout=2,
    )
except OSError as error:
    if error.errno != errno.E2BIG:
        raise
else:
    raise SystemExit('the argument-limit fixture did not reach E2BIG')

filename_limit = os.pathconf('.', 'PC_NAME_MAX')
if not 0 < filename_limit <= 1024:
    raise SystemExit('unsupported filesystem component limit for the filename-limit fixture')
in_range = b'./' + b'x' * filename_limit
over_limit = in_range + b'x'
for path, expected_errno in ((over_limit, errno.ENAMETOOLONG), (in_range, errno.ENOENT)):
    try:
        os.stat(path)
    except OSError as error:
        if error.errno != expected_errno:
            raise
    else:
        raise SystemExit('the filename-limit fixture unexpectedly found the path')

PRIOR_ERROR = b'make[1]: ./prior: No such file or directory\n'
FATAL = b'make[1]: *** [worker.mk:3: all] Error 127\n'
IGNORED = b'make[1]: [worker.mk:3: all] Error 127 (ignored)\n'
PRIOR_OUT = b'prior-out\n'
PRIOR_ERR = b'prior-err\n'

cases = []
for limit, recipe_command, launch_error in (
    ('argument limit', b'/usr/bin/true ' + argument,
     b'make[1]: /usr/bin/true: Argument list too long\n'),
    ('filename limit', over_limit, b'make[1]: ' + over_limit + b': File name too long\n'),
):
    cases.extend((
        (f'{limit} direct', recipe_command, ('-j1', '-Onone'), True, False, False, 2,
         PRIOR_OUT, PRIOR_ERR + launch_error + PRIOR_ERROR + FATAL),
        (f'{limit} ignored', recipe_command, ('-j1', '-Onone'), True, True, True, 0,
         PRIOR_OUT + PRIOR_ERR + launch_error + PRIOR_ERROR + IGNORED + b'continued\n', b''),
        (f'{limit} parallel target', recipe_command, ('-j2', '-Otarget'), True, False, True, 2,
         PRIOR_ERROR + PRIOR_OUT + PRIOR_ERR + launch_error + FATAL, b''),
        (f'{limit} healthy prior control', recipe_command, ('-j1', '-Onone'), False, False, False, 2,
         PRIOR_OUT, PRIOR_ERR + launch_error + FATAL),
    ))
cases.append((
    'in-range missing filename control', in_range, ('-j1', '-Onone'), True, False, False, 2,
    PRIOR_OUT, PRIOR_ERR + b'make[1]: ' + in_range + b': No such file or directory\n' + PRIOR_ERROR + FATAL,
))

failures = []
for name, recipe_command, options, removed, ignored, merged, status, stdout, stderr in cases:
    with tempfile.TemporaryDirectory(dir='.') as scratch:
        work = Path(scratch)
        tmp = work/'tmp'
        tmp.mkdir()
        prior = work/'prior'
        script = b'#!/bin/sh\nprintf "prior-out\\n"\nprintf "prior-err\\n" >&2\n'
        if removed:
            script += b'rm -- "$0"\n'
        script += b'exit 0\n'
        prior.write_bytes(script)
        prior.chmod(0o755)
        worker = work/'worker.mk'
        modifier = b'-@' if ignored else b'@'
        worker.write_bytes(
            b'all:\n\t@./prior\n\t' + modifier + recipe_command +
            b'\n\t@printf "continued\\n"; printf "effect\\n" > ran\n'
        )
        command = [sys.argv[1], '-f', 'worker.mk', *options, 'all']
        result = subprocess.run(
            command, cwd=work, env=dict(os.environ, TMPDIR=str(tmp.resolve())),
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT if merged else subprocess.PIPE,
            timeout=2, umask=0o022,
        )
        if result.returncode not in (0, 2):
            raise SystemExit(f'unexpected make outcome in {name}: {result!r}')
        expected = {'tmp', 'worker.mk'}
        files_match = not any(tmp.iterdir())
        if not removed:
            expected.add('prior')
            files_match &= (prior.is_file() and prior.read_bytes() == script
                            and stat.S_IMODE(prior.stat().st_mode) == 0o755)
        if ignored:
            expected.add('ran')
            witness = work/'ran'
            files_match &= witness.is_file() and witness.read_bytes() == b'effect\n'
        files_match &= {path.name for path in work.iterdir()} == expected
        if (result.returncode != status or result.stdout != stdout
                or (result.stderr or b'') != stderr or not files_match):
            failures.append(f'{name}: {result!r}, effects_match={files_match}')
        else:
            print(f'verified {name}')

if failures:
    print('\n'.join(failures), file=sys.stderr)
    raise SystemExit(1)
