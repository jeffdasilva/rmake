# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Apply process limits only to this disposable executable image."""
import errno
import os
from pathlib import Path
import resource
import sys

nofile = int(sys.argv[1])
resource.setrlimit(resource.RLIMIT_NPROC, (0, 0))
try:
    child = os.fork()
except OSError as error:
    if error.errno != errno.EAGAIN:
        raise
else:
    if child == 0:
        os._exit(0)
    os.waitpid(child, 0)
    raise SystemExit('nproc witness: fork succeeded despite zero budget')

Path('pressure').write_text('nproc=0 fork=EAGAIN\n')
if nofile:
    resource.setrlimit(resource.RLIMIT_NOFILE, (nofile, nofile))
os.execv(sys.argv[2], sys.argv[2:])
