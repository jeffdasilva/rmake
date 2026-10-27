# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import json,os
from pathlib import Path
names=["Q0000", "Q0001", "Q0002", "Q0003", "Q0004", "Q0005", "Q0006", "Q0007", "Q0008", "Q0009", "Q0010", "Q0011", "Q0012", "Q0013", "Q0014", "Q0015", "Q0016", "Q0017", "Q0018", "Q0019", "Q0020", "Q0021", "Q0022", "Q0023", "Q0024", "Q0025", "Q0026", "Q0027", "Q0028", "Q0029", "Q0030", "Q0031", "Q0032", "Q0033", "Q0034", "Q0035", "Q0036", "Q0037", "Q0038", "Q0039"]
print('VALUES:'+json.dumps({n:os.environ.get(n) for n in names},sort_keys=True,separators=(',',':')))
path=Path('journal')
print('JOURNAL:'+(path.read_bytes().hex() if path.exists() else 'absent'))
