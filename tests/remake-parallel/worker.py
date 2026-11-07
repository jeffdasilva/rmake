# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

import os
from pathlib import Path
import sys
import time

name, mode = sys.argv[1:]
ready = Path(name + '.started.tmp')
ready.write_bytes(b'started\n')
ready.rename(name + '.started')

deadline = time.monotonic() + 4
release = Path(name + '.release')
while not release.is_file():
    if time.monotonic() >= deadline:
        raise SystemExit(97)
    time.sleep(.002)
if release.read_bytes() != b'yes\n':
    raise SystemExit(96)

with Path('seen').open('ab') as stream:
    stream.write((name + '\n').encode())
if mode == 'include':
    output = Path(name + '.gen')
    output.write_bytes((name.upper() + ' := loaded\n').encode())
    os.utime(output, ns=(1_000_000_001_000_000_000, 1_000_000_001_000_000_000))
Path(name + '.done').write_bytes(b'done\n')
if mode == 'fail':
    print(name, flush=True)
    raise SystemExit(3 if name == 'left' else 4)
