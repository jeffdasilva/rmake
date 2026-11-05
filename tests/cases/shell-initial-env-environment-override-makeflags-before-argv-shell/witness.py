# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

import os
from pathlib import Path
import sys

phase = sys.argv[1]
with Path("seen").open("a") as journal:
    journal.write(phase + "\n")
    if phase in ("argv", "fallback"):
        journal.write(phase + "-makeflags=" + os.environ.get("MAKEFLAGS", "<unset>") + "\n")
