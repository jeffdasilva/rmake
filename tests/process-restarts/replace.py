# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import os
from pathlib import Path
name=Path('../bin/make')
assert name.is_symlink()
os.replace('replacement.sh',name)
