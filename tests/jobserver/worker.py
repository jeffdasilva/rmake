# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import fcntl
from pathlib import Path
import sys
import time

with open('lock', 'a') as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    active = int(Path('active').read_text()) + (1 if sys.argv[1] == 'enter' else -1)
    Path('active').write_text(str(active))
    Path('maximum').write_text(str(max(active, int(Path('maximum').read_text()))))
if sys.argv[1] == 'enter':
    for _ in range(300):
        try:
            if int(Path('maximum').read_text() or '0') >= 3:
                break
        except ValueError:
            pass
        time.sleep(.01)
    else:
        sys.exit(9)
    time.sleep(.02)
else:
    Path(sys.argv[2]).write_text('complete\n')
