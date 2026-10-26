# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import os
path = b'out file'
os.makedirs(os.path.dirname(path) or b".", exist_ok=True)
with open(path,"wb") as stream: stream.write(b"input\n")
path = b'in file'
os.makedirs(os.path.dirname(path) or b".", exist_ok=True)
with open(path,"wb") as stream: stream.write(b"input\n")
os.utime(b"in file",ns=(1600000000000000000,)*2)
os.utime(b"out file",ns=(1600000010000000000,)*2)
