# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original parallel worker owns a real slot until the Guile worker completes."""
from pathlib import Path
import resource
import sys
import time

role = sys.argv[1]
if role not in ("alpha", "beta") or resource.getrlimit(resource.RLIMIT_NOFILE) != (64, 64):
    raise SystemExit(71)
temporary = Path(f"{role}.tmp")
temporary.write_text(f"{role}\n")
temporary.rename(f"{role}.ready")
done = Path("callback.done")
deadline = time.monotonic() + 3
while not done.is_file():
    if time.monotonic() >= deadline:
        raise SystemExit(72)
    time.sleep(0.002)
Path(f"{role}.result").write_bytes(done.read_bytes())
