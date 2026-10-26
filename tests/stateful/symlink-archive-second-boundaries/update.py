# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import os
from pathlib import Path
phase = int(Path('phase').read_text())
date = 22 if phase == 2 else 24
Path('lib.a').write_bytes(Path(f'expected-{date}.a').read_bytes())
seconds = 30 if phase == 2 else 40
os.utime('lib.a', ns=((1600000000 + seconds) * 10**9,) * 2)
