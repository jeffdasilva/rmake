# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

import errno
import fcntl
import os
from pathlib import Path
import stat
import sys


KINDS = {stat.S_IFREG: "file", stat.S_IFIFO: "pipe", stat.S_IFCHR: "device"}


def describe(fd):
    try:
        metadata = os.fstat(fd)
    except OSError as error:
        if error.errno == errno.EBADF:
            return "closed"
        raise
    kind = KINDS.get(stat.S_IFMT(metadata.st_mode), "other")
    flags = fcntl.fcntl(fd, fcntl.F_GETFL)
    return f"{kind}/access={flags & os.O_ACCMODE}/append={int(bool(flags & os.O_APPEND))}"


def exec_submake(executable):
    auth, = [argument.removeprefix("--jobserver-auth=")
             for argument in os.environ["MAKEFLAGS"].split()
             if argument.startswith("--jobserver-auth=")]
    read, write = map(int, auth.split(","))
    reader, writer = (os.fstat(fd) for fd in (read, write))
    if not (stat.S_ISFIFO(reader.st_mode) and stat.S_ISFIFO(writer.st_mode)):
        raise RuntimeError("wire descriptors must be pipes")
    if (reader.st_dev, reader.st_ino) != (writer.st_dev, writer.st_ino):
        raise RuntimeError("wire descriptors must identify the same pipe")
    access = [fcntl.fcntl(fd, fcntl.F_GETFL) & os.O_ACCMODE for fd in (read, write)]
    if access != [os.O_RDONLY, os.O_WRONLY]:
        raise RuntimeError("wire directions must be read and write")
    Path("wire").write_text(f"{read},{write} same-pipe read-only write-only\n")
    Path("wire-identity").write_text(f"{reader.st_dev},{reader.st_ino}\n")
    os.execv(executable, [executable, "-f", "Child.mk", "child"])


def parent_wire_state(descriptor, identity):
    path = Path(f"/proc/{os.getppid()}/fd/{descriptor}")
    try:
        metadata = path.stat()
    except FileNotFoundError:
        return "closed"
    return "original" if (metadata.st_dev, metadata.st_ino) == identity else "replacement"


def inspect():
    auth = Path("wire").read_text().split()[0]
    wire = [int(fd) for fd in auth.split(",")]
    identity_file = Path("wire-identity")
    identity = tuple(map(int, identity_file.read_text().split(",")))
    identity_file.unlink()
    Path("seen-child").write_text(" ".join(describe(fd) for fd in (0, 1, 2)) + "\n")
    Path("seen-wire").write_text(" ".join(describe(fd) for fd in wire) + "\n")
    states = [f"parent{fd}={parent_wire_state(fd, identity)}" for fd in wire]
    Path("seen-parent").write_text(" ".join(states) + "\n")


if sys.argv[1:] == ["inspect"]:
    inspect()
elif sys.argv[1] == "submake":
    exec_submake(sys.argv[2])
else:
    if sys.argv[1] != "open":
        for fd in sys.argv[1].split(","):
            os.close(int(fd))
    os.execv(sys.argv[2], sys.argv[2:])
