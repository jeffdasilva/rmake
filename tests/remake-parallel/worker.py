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

if mode == 'named-value':
    Path('named.gen').write_bytes(b'VALUE := rebuilt\n')
else:
    with Path('seen').open('ab') as stream:
        stream.write((name + '\n').encode())
if mode in ('include', 'include-fail'):
    output = Path(name + '.gen')
    output.write_bytes((name.upper() + ' := loaded\n').encode())
    os.utime(output, ns=(1_000_000_001_000_000_000, 1_000_000_001_000_000_000))
if mode in ('write-assumed-source', 'restore-assumed-source'):
    source = Path('b.mk')
    source.write_bytes(b'AFTER := settled\n')
    stamp = 1_000_000_002_000_000_000 if mode == 'write-assumed-source' else 1_000_000_000_000_000_000
    os.utime(source, ns=(stamp, stamp))
if mode in ('write-optional', 'write-optional-restart', 'write-fallback-restart'):
    optional = Path('Makefile' if mode == 'write-fallback-restart' else 'optional.gen')
    optional.write_bytes(b'OPTIONAL := new\n')
    os.utime(optional, ns=(1_000_000_004_000_000_000,) * 2)
    if mode.endswith('-restart'):
        required = Path(name + '.gen')
        required.write_bytes(b'# changed\n')
        os.utime(required, ns=(1_000_000_005_000_000_000,) * 2)
if mode == 'append-primary':
    primary = Path('Makefile')
    with primary.open('ab') as stream:
        stream.write(b'# remade\n')
    os.utime(primary, ns=(1_000_000_004_000_000_000,) * 2)
Path(name + '.done').write_bytes(b'done\n')
if mode in ('fail', 'include-fail'):
    print(name, flush=True)
    raise SystemExit(3 if name == 'left' else 4)
