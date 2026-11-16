#!/usr/bin/python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

import os
from pathlib import Path
import sys
from time import monotonic, sleep

def wait_for_release(name):
    Path(f'{name}.started').write_bytes(b'ready\n')
    deadline = monotonic() + 4
    while not Path(f'{name}.release').is_file():
        if monotonic() >= deadline:
            raise SystemExit(97)
        sleep(.002)


if sys.argv[1:] == ['Makefile.sh']:
    wait_for_release('primary')
    Path('primary.done').write_bytes(b'done\n')
elif sys.argv[1:] in (['--child'], ['--leaf']):
    name = sys.argv[1][2:]
    wait_for_release(name)
    Path(f'{name}.mk').write_bytes(f'{name.upper()} := loaded\n'.encode())
    Path(f'{name}.done').write_bytes(b'done\n')
    raise SystemExit(0)

os.execv('/bin/cat', sys.argv)
