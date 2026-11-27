# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

import errno
import fcntl
import os
from pathlib import Path
import stat
import sys


def inspect():
    slots = []
    for fd in (1, 2):
        try:
            metadata = os.fstat(fd)
        except OSError as error:
            if error.errno != errno.EBADF:
                raise
            slots.append("closed")
            continue
        kind = "file" if stat.S_ISREG(metadata.st_mode) else "pipe" if stat.S_ISFIFO(metadata.st_mode) else "other"
        flags = fcntl.fcntl(fd, fcntl.F_GETFL)
        slots.append(f"{kind}/access={flags & os.O_ACCMODE}/append={int(bool(flags & os.O_APPEND))}")
    Path("seen-child").write_text(" ".join(slots) + "\n")


if sys.argv[1:] == ["inspect"]:
    inspect()
else:
    closed_output = int(sys.argv[1])
    if closed_output:
        os.close(closed_output)
    os.execv(sys.argv[2], sys.argv[2:])
