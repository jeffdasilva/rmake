#!/usr/bin/python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

import os
from pathlib import Path
import sys
from time import monotonic, sleep

if sys.argv[1:] == ['Makefile.sh']:
    Path('primary.started').write_bytes(b'ready\n')
    deadline = monotonic() + 4
    while not Path('primary.release').is_file():
        if monotonic() >= deadline:
            raise SystemExit(97)
        sleep(.002)
    Path('primary.done').write_bytes(b'done\n')

os.execv('/bin/cat', sys.argv)
