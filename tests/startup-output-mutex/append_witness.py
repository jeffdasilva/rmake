# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Prepare inherited output slots and record parent and recipe flag ownership."""
import errno
import fcntl
import os
from pathlib import Path
import stat
import sys
import tempfile


def launch(mode, argv):
    closed = {"closed-stdout": (1,), "closed-stderr": (2,), "closed-both": (1, 2)}.get(mode)
    if closed is not None:
        for fd in closed:
            os.close(fd)
        os.execv(argv[0], argv)
    for fd in (1, 2):
        if mode == "regular":
            with tempfile.TemporaryFile(dir=".") as output:
                output.write(b"seed\n")
                output.seek(0)
                os.dup2(output.fileno(), fd)
        elif mode == "device":
            with open("/dev/null", "wb") as output:
                os.dup2(output.fileno(), fd)
        elif mode == "append-pipe":
            flags = fcntl.fcntl(fd, fcntl.F_GETFL)
            fcntl.fcntl(fd, fcntl.F_SETFL, flags | os.O_APPEND | os.O_NONBLOCK)
        elif mode != "pipe":
            raise SystemExit(f"unknown output mode {mode}")
    os.execv(argv[0], argv)


def describe(metadata, flags):
    kinds = {stat.S_IFREG: "file", stat.S_IFIFO: "pipe", stat.S_IFCHR: "device"}
    kind = kinds[stat.S_IFMT(metadata.st_mode)]
    return f"{kind} append={int(bool(flags & os.O_APPEND))} nonblock={int(bool(flags & os.O_NONBLOCK))}"


def inspect():
    parent = Path("/proc") / str(os.getppid())
    rows = []
    for fd in (1, 2):
        metadata = (parent / "fd" / str(fd)).stat()
        flags = next(line.split()[1] for line in (parent / "fdinfo" / str(fd)).read_text().splitlines()
                     if line.startswith("flags:"))
        rows.append(f"parent{fd} {describe(metadata, int(flags, 8))}")
        own = os.fstat(fd)
        rows.append(f"recipe{fd} {describe(own, fcntl.fcntl(fd, fcntl.F_GETFL))}")
        if stat.S_ISREG(own.st_mode) and os.path.samestat(own, metadata):
            os.lseek(fd, 0, os.SEEK_SET)
            os.write(fd, b"child\n")
            os.lseek(fd, 0, os.SEEK_SET)
            content = os.read(fd, 16)
            rows.append(f"file{fd} {content.hex()}")
    Path("seen").write_text("\n".join(rows) + "\n")


def inspect_recipe():
    rows = []
    for fd in (1, 2):
        try:
            description = describe(os.fstat(fd), fcntl.fcntl(fd, fcntl.F_GETFL))
        except OSError as error:
            if error.errno != errno.EBADF:
                raise
            description = "closed"
        rows.append(f"recipe{fd} {description}")
    Path("seen").write_text("\n".join(rows) + "\n")


if sys.argv[1:] == ["inspect"]:
    inspect()
elif sys.argv[1:] == ["inspect-recipe"]:
    inspect_recipe()
else:
    launch(sys.argv[1], sys.argv[2:])
