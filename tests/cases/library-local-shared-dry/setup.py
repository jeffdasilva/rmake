# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
from pathlib import Path
import os
p=Path('libdemo.so');p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(b"original library input\n");os.utime(p,ns=(1600000000000000000,)*2)
