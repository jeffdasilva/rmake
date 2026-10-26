# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import os
with open(b"lib\xffdemo.a","wb") as f:f.write(b"input\n")
os.utime(b"lib\xffdemo.a",ns=(1600000000000000000,)*2)
