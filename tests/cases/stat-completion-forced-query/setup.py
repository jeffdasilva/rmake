# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
from pathlib import Path
import os
for name,seconds in [('out',1700000000),('child',1600000000),('a',1600000000),('b',1600000000)]:
 Path(name).write_bytes(b'old\n');os.utime(name,ns=(seconds*10**9,)*2)
