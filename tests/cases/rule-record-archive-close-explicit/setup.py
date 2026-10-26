# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
from pathlib import Path
import sys
name=sys.argv[1].encode()
header=(name+b'/').ljust(16)+b'1600000000'.ljust(12)+b'0'.ljust(6)+b'0'.ljust(6)+b'100644'.ljust(8)+b'2'.ljust(10)+b'`\n'
Path('lib.a').write_bytes(b'!<arch>\n'+header+b'x\n')
