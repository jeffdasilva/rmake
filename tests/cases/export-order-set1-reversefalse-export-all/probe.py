# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import json,os
from pathlib import Path
names=["X", "Y", "Z"]
print('VALUES:'+json.dumps({n:os.environ.get(n) for n in names},sort_keys=True,separators=(',',':')))
path=Path('journal')
print('JOURNAL:'+(path.read_bytes().hex() if path.exists() else 'absent'))
