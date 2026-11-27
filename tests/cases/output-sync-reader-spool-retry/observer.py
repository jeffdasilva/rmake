# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import tempfile


TEMP = (
    re.escape(b'make[1]: cannot create temporary file relative///Gm')
    + rb'[A-Za-z0-9]{6}'
    + re.escape(b': No such file or directory\n')
)
SUPPRESS = re.escape(b'make[1]: cannot open output-sync lock file, suppressing output-sync.\n')
FAILED_SPOOL = TEMP + SUPPRESS
FAILED_DIAGNOSTIC = FAILED_SPOOL + SUPPRESS
RECIPE = re.escape(b'recipe\n')
FATAL = re.escape(b'../worker.mk:2: *** E2.  Stop.\n')
INFO = re.escape(b'I1\n')

cases = (
    ('initial fatal', '$(error E1)\n', 'line', 1, True, False, 2,
     FAILED_DIAGNOSTIC, b''),
    ('preserved info and fatal retry', '$(info I1)\n$(error E2)\n',
     'line', 1, True, False, 2, FAILED_SPOOL + INFO + FAILED_DIAGNOSTIC, b''),
    ('warning retry and post-read replay', '$(info I1)\n$(warning W2)\n',
     'target', 2, False, False, 0, INFO + RECIPE,
     FAILED_SPOOL + FAILED_DIAGNOSTIC * 2),
    ('warning then a single info retry', '$(warning W1)\n$(info I2)\n$(info I3)\n',
     'line', 1, True, False, 0,
     FAILED_DIAGNOSTIC + FAILED_SPOOL + re.escape(b'I2\nI3\n') + RECIPE, b''),
    ('recurse control', '$(info I1)\n$(error E2)\n', 'recurse', 2,
     False, False, 2, INFO, FATAL),
    ('available directory control', '$(info I1)\n$(error E2)\n', 'line', 1,
     True, True, 2, INFO + FATAL, b''),
)

failures = []
for name, body, mode, jobs, merged, working_temp, status, stdout, stderr in cases:
    with tempfile.TemporaryDirectory(dir='.') as scratch:
        root = Path(scratch)
        (root / 'relative').mkdir()
        shifted = root / 'shifted'
        shifted.mkdir()
        if working_temp:
            (shifted / 'relative').mkdir()
        (root / 'worker.mk').write_text(
            body + "all: ; @printf 'recipe\\n'; printf 'effect\\n' > ran\n"
        )
        command = [sys.argv[1], '-C', 'shifted', '-f', '../worker.mk',
                   f'-j{jobs}', '--jobserver-style=pipe', f'-O{mode}', 'all']
        result = subprocess.run(
            command, cwd=root, env=dict(os.environ, MAKE_TMPDIR='relative///'),
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT if merged else subprocess.PIPE,
            timeout=2, umask=0o022,
        )
        if result.returncode < 0:
            raise SystemExit(f'unexpected make signal in {name}: {result!r}')
        witness = shifted / 'ran'
        if status == 0:
            files_match = (
                witness.is_file()
                and witness.read_bytes() == b'effect\n'
                and stat.S_IMODE(witness.stat().st_mode) == 0o600
                and sorted(p.name for p in shifted.iterdir()) == ['ran']
            )
        else:
            expected_files = ['relative'] if working_temp else []
            files_match = sorted(p.name for p in shifted.iterdir()) == expected_files
        if (result.returncode != status or not re.fullmatch(stdout, result.stdout)
                or not re.fullmatch(stderr, result.stderr or b'') or not files_match):
            failures.append(f'{name}: {result!r}, effects_match={files_match}')
        else:
            print(f'verified {name}')

if failures:
    print('\n'.join(failures), file=sys.stderr)
    raise SystemExit(1)
