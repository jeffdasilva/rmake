# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import json,os
from pathlib import Path
names=["C000618", "C000849", "C002005", "C004047", "C004351", "C004518", "C005576", "C006672", "C007362", "C007974", "C009593", "C009693", "C012094", "C013920", "C014046", "C014289", "C020545", "C022447", "C026756", "C026828", "C030112", "C032408", "C034126", "C035438", "C036399", "C036776", "C037652", "C038255", "C040144", "C043099", "C043167", "C043823", "C044542", "C044606", "C050298", "C051086", "C051181", "C051883", "C052268", "C052614"]
print('VALUES:'+json.dumps({n:os.environ.get(n) for n in names},sort_keys=True,separators=(',',':')))
path=Path('journal')
print('JOURNAL:'+(path.read_bytes().hex() if path.exists() else 'absent'))
