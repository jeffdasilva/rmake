# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import os
from pathlib import Path
phase = int(Path('phase').read_text())
values = [(5, 3, 0), (20, 3, 0), (20, 35, 0), (0, 0, 50), (65, 70, 0), (85, 90, 0), (5, 3, 0)]
link, hop, leaf = values[phase]
destination = 'missing' if phase == 4 else 'input'
if os.readlink('hop') != destination:
    os.unlink('hop')
    os.symlink(destination, 'hop')
for name, seconds in [('link', link), ('hop', hop), ('input', leaf)]:
    os.utime(name, ns=((1600000000 + seconds) * 10**9,) * 2, follow_symlinks=False)
