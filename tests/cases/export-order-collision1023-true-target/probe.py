# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import json,os
from pathlib import Path
names=["C001257", "C001424", "C003311", "C004064", "C006459", "C007846", "C007850", "C007905", "C008114", "C008652", "C010019", "C011388", "C013698", "C013735", "C015489", "C016600", "C019109", "C019525", "C020483", "C022033", "C022097", "C023486", "C024192", "C024545", "C024907", "C025267", "C025667", "C025745", "C026381", "C028230", "C030073", "C031836", "C032291", "C035406", "C037964", "C039149", "C040152", "C040796", "C043965", "C045396"]
print('VALUES:'+json.dumps({n:os.environ.get(n) for n in names},sort_keys=True,separators=(',',':')))
path=Path('journal')
print('JOURNAL:'+(path.read_bytes().hex() if path.exists() else 'absent'))
