# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import os
from pathlib import Path
home=Path.cwd().parent/"home"
assert Path(os.environ["HOME"]).resolve()==home.resolve()
(home/"input").write_bytes(b"input\n")
Path("expected-home").write_bytes(os.fsencode(home)+b"/input")
