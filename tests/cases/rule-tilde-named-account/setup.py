# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import os
import pwd
from pathlib import Path
account=pwd.getpwuid(os.getuid())
Path("identity.mk").write_bytes(b"ACCOUNT := "+os.fsencode(account.pw_name)+b"\n")
Path("~"+account.pw_name).write_bytes(b"literal-control\n")
