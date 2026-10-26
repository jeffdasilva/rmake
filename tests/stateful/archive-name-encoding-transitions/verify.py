# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
from pathlib import Path
import sys
actual = Path("lib.a").read_bytes()
assert actual == Path(sys.argv[1]).read_bytes(), "archive bytes changed outside the selected date"
print("archive bytes=ok")
