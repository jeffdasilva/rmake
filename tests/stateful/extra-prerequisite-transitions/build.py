# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
from pathlib import Path
import os
import sys
Path('out').write_bytes(b'original output\n')
stamp = int(Path('next-time').read_text())
os.utime('out', ns=(stamp, stamp))
print('built=[' + '|'.join(sys.argv[1:]) + ']')
