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
    kinds = {stat.S_IFREG: "file", stat.S_IFIFO: "pipe", stat.S_IFCHR: "device"}
    for fd in (0, 1, 2):
        try:
            metadata = os.fstat(fd)
        except OSError as error:
            if error.errno != errno.EBADF:
                raise
            slots.append("closed")
            continue
        kind = kinds.get(stat.S_IFMT(metadata.st_mode), "other")
        flags = fcntl.fcntl(fd, fcntl.F_GETFL)
        slots.append(f"{kind}/access={flags & os.O_ACCMODE}/append={int(bool(flags & os.O_APPEND))}")
    Path("seen-child").write_text(" ".join(slots) + "\n")


if sys.argv[1:] == ["inspect"]:
    inspect()
else:
    if sys.argv[1] != "open":
        os.close(int(sys.argv[1]))
    os.execv(sys.argv[2], sys.argv[2:])
