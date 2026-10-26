# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import os
from pathlib import Path
phase = int(Path('phase').read_text())
seconds, fraction = [(19, 999999999), (20, 999999999), (21, 0), (22, 999999999), (23, 0)][phase]
ns = (1600000000 + seconds) * 10**9 + fraction
os.utime('one.o', ns=(ns, ns), follow_symlinks=False)
os.utime('input', ns=(1600000000000000000,) * 2)
