# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import os,sys
with open(sys.argv[2],'rb') as source: data=source.read()
with open(sys.argv[1],'wb') as output: output.write(data)
stamp=os.stat(sys.argv[2]).st_mtime_ns+1000000000
os.utime(sys.argv[1],ns=(stamp,stamp))
print('built='+':'.join(sys.argv[1:]))
