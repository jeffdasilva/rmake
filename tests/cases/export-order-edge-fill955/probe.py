# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import json,os
from pathlib import Path
names=["EDGE001449", "EDGE001761", "EDGE001874", "EDGE002405", "EDGE002658", "EDGE002882", "EDGE004223", "EDGE004283", "EDGE004596", "EDGE005402", "EDGE005538", "EDGE005980", "EDGE006708", "EDGE006869", "EDGE006967", "EDGE006981", "EDGE007650", "EDGE008629", "EDGE010193", "EDGE012266", "EDGE014063", "EDGE015835", "EDGE016435", "EDGE016747", "EDGE019998", "EDGE020625", "EDGE021593", "EDGE022720", "EDGE022883", "EDGE023683", "EDGE023794", "EDGE024124"]
print('VALUES:'+json.dumps({n:os.environ.get(n) for n in names},sort_keys=True,separators=(',',':')))
path=Path('journal')
print('JOURNAL:'+(path.read_bytes().hex() if path.exists() else 'absent'))
