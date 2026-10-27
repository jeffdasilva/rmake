# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import json,os
from pathlib import Path
names=["C000978", "C002094", "C002864", "C005141", "C005774", "C007676", "C008607", "C008921", "C009954", "C011157", "C011842", "C012681", "C014674", "C014956", "C015803", "C017423", "C018095", "C019885", "C019901", "C022259", "C022487", "C026682", "C027070", "C027101", "C027292", "C027338", "C028514", "C029881", "C031824", "C031990", "C032610", "C035712", "C036502", "C037162", "C038016", "C038392", "C038812", "C038893", "C039424", "C039879"]
print('VALUES:'+json.dumps({n:os.environ.get(n) for n in names},sort_keys=True,separators=(',',':')))
path=Path('journal')
print('JOURNAL:'+(path.read_bytes().hex() if path.exists() else 'absent'))
