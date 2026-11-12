# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

from pathlib import Path
import time


for _ in range(500):
    try:
        if Path('tail.release').read_bytes() == b'yes\n':
            Path('tail.gen').write_bytes(b'TAIL := loaded\n')
            break
    except FileNotFoundError:
        pass
    time.sleep(0.01)
else:
    raise SystemExit('tail release was not observed')
