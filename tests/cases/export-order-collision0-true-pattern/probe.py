# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import json,os
from pathlib import Path
names=["C000062", "C002999", "C004855", "C006140", "C006182", "C006507", "C007398", "C009760", "C011044", "C011607", "C011640", "C012726", "C012919", "C012942", "C013005", "C013536", "C013605", "C015877", "C016031", "C017139", "C017785", "C021601", "C021728", "C021769", "C022366", "C024978", "C025234", "C025381", "C026101", "C027745", "C029056", "C029344", "C029414", "C030258", "C030978", "C031554", "C031983", "C032215", "C034245", "C037722"]
print('VALUES:'+json.dumps({n:os.environ.get(n) for n in names},sort_keys=True,separators=(',',':')))
path=Path('journal')
print('JOURNAL:'+(path.read_bytes().hex() if path.exists() else 'absent'))
