# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""The recipe waits for the test's pipe barrier before exiting."""
from pathlib import Path
import sys
import time

started = Path('started.tmp')
started.write_bytes(b'started\n')
started.rename('started')
release = Path('release')
deadline = time.monotonic() + 4
while not release.is_file():
    if time.monotonic() >= deadline:
        raise SystemExit(97)
    time.sleep(.002)
if release.read_bytes() != b'yes\n':
    raise SystemExit(96)
Path('done').write_bytes(b'done\n')
raise SystemExit(int(sys.argv[1]))
