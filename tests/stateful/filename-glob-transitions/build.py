# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import os,sys
with open(sys.argv[1],'wb') as output: output.write((sys.argv[2]+'\n').encode())
stamp=max(entry.stat().st_mtime_ns for entry in os.scandir('inputs') if entry.name.endswith('.in'))+1000000000
os.utime(sys.argv[1],ns=(stamp,stamp))
print('built='+':'.join(sys.argv[1:]))
