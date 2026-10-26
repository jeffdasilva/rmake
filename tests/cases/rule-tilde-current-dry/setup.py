# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import os
path=b'out'
os.makedirs(os.path.dirname(path) or b".",exist_ok=True)
with open(path,"wb") as stream: stream.write(b"input\n")
path=b'house/input'
os.makedirs(os.path.dirname(path) or b".",exist_ok=True)
with open(path,"wb") as stream: stream.write(b"input\n")
os.utime("house/input",ns=(1600000000000000000,)*2)
os.utime("out",ns=(1600000010000000000,)*2)
