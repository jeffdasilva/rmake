# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

import os
from pathlib import Path
import sys
import time

ready = Path("ready")
draft = ready.with_suffix(".draft")
draft.write_text(f"{os.getpid()} {os.getppid()}\n")
draft.rename(ready)
deadline = time.monotonic() + 3
release = Path("release")
while not release.exists():
    if time.monotonic() >= deadline:
        raise TimeoutError("controller did not acknowledge the single recipe")
    time.sleep(0.005)
ready.unlink()
release.unlink()
Path("wire.ready").unlink(missing_ok=True)
print(f"auth=[{sys.argv[1]}]")
