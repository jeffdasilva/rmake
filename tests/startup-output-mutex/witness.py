# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

import os
from pathlib import Path
import sys


def matching_lock(pid, identity):
    proc = Path("/proc") / str(pid)
    result = []
    for descriptor in (proc / "fd").iterdir():
        try:
            metadata = descriptor.stat()
            if (metadata.st_dev, metadata.st_ino) != identity:
                continue
            fdinfo = dict(line.split(":", 1) for line in
                          (proc / "fdinfo" / descriptor.name).read_text().splitlines())
        except FileNotFoundError:
            continue
        result.append(int(fdinfo["flags"], 8))
    return result


info = Path("lock").stat()
identity = info.st_dev, info.st_ino
parent = matching_lock(os.getppid(), identity)
child = matching_lock(os.getpid(), identity)
ready = bool(parent) and all(
    flags & os.O_ACCMODE == os.O_WRONLY and flags & os.O_CLOEXEC for flags in parent
)
with Path("seen").open("a") as journal:
    journal.write(f"{sys.argv[1]}-open={'yes' if ready else 'no'} "
                  f"child-inherited={'yes' if child else 'no'}\n")
