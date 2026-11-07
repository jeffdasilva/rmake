# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

from pathlib import Path
import sys
import time

name, mode = sys.argv[1:]
Path(name + '.started.tmp').write_bytes(b'started\n')
Path(name + '.started.tmp').rename(name + '.started')
release = Path(name + '.release')
deadline = time.monotonic() + 4
while not release.exists():
    if time.monotonic() >= deadline:
        raise SystemExit(97)
    time.sleep(0.003)
if release.read_bytes() != b'yes\n':
    raise SystemExit(96)
with Path('seen').open('ab') as stream:
    stream.write(name.encode() + b'\n')
Path(name + '.done').write_bytes(b'done\n')
if mode == 'fail':
    raise SystemExit(3)
for output in {'explicit': ('a', 'b'), 'implicit': ('a.out', 'a.peer'), 'right': ('tail',)}[mode]:
    Path(output).write_bytes(b'updated\n')
