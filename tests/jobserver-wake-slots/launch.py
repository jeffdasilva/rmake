# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

import errno
import fcntl
import json
import os
from pathlib import Path
import sys

reader, writer = os.pipe2(os.O_CLOEXEC)
os.write(writer, b"Q")
identity = os.fstat(reader)
for target, source in ((16, reader), (17, writer), (18, reader), (19, writer)):
    try:
        fcntl.fcntl(target, fcntl.F_GETFD)
    except OSError as error:
        if error.errno != errno.EBADF:
            raise
    else:
        raise RuntimeError("prepared inherited slot already open")
    os.dup2(source, target, inheritable=True)
os.close(reader)
os.close(writer)
Path("wire.ready").write_text(json.dumps({"dev": identity.st_dev, "ino": identity.st_ino}) + "\n")
os.execv(sys.argv[1], sys.argv[1:])
