# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import json,os
from pathlib import Path
names=["C000655", "C000685", "C002906", "C003102", "C003613", "C004183", "C007335", "C008010", "C009718", "C012126", "C015687", "C017796", "C019220", "C019317", "C019949", "C020172", "C020574", "C021704", "C022029", "C022139", "C023726", "C025470", "C026356", "C027146", "C027289", "C027357", "C027691", "C027843", "C027945", "C028661", "C028974", "C031074", "C032011", "C032430", "C035745", "C036091", "C037083", "C037212", "C038615", "C038740"]
print('VALUES:'+json.dumps({n:os.environ.get(n) for n in names},sort_keys=True,separators=(',',':')))
path=Path('journal')
print('JOURNAL:'+(path.read_bytes().hex() if path.exists() else 'absent'))
