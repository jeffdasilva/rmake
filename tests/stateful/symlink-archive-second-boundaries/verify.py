# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
from pathlib import Path
phase = int(Path('phase').read_text())
date = 22 if phase == 2 else 24
assert Path('lib.a').read_bytes() == Path(f'expected-{date}.a').read_bytes()
print(f'date=16000000{date}')
