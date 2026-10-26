# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import os
path=b'-house/a'
os.makedirs(os.path.dirname(path) or b".",exist_ok=True)
with open(path,"wb") as stream: stream.write(b"input\n")
path=b'all-house/a'
os.makedirs(os.path.dirname(path) or b".",exist_ok=True)
with open(path,"wb") as stream: stream.write(b"input\n")
