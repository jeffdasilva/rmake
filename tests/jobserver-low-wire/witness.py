# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

import errno
import fcntl
import json
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
    parent = Path(f"/proc/{os.getppid()}")
    path = parent / "fd" / str(descriptor)
    try:
        metadata = path.stat()
    except FileNotFoundError:
        return "retired", {"state": "closed"}
    target = str(path.readlink())
    if (metadata.st_dev, metadata.st_ino) == identity:
        return "original", {"state": "original", "target": target}
    raw = {"state": "replacement", "target": target}
    # The parent's wait handle for this recipe can reuse the retired slot.
    if target == "anon_inode:[pidfd]":
        info = (parent / "fdinfo" / str(descriptor)).read_text()
        fields = dict(line.split(":", 1) for line in info.splitlines() if ":" in line)
        flags = fields.get("flags", "").strip()
        pid = fields.get("Pid", "").strip()
        recipe_pid = os.getpid()
        raw.update(flags=flags, pid=pid, recipe_pid=recipe_pid)
        if flags and int(flags, 8) & os.O_CLOEXEC and pid == str(recipe_pid):
            raw["state"] = "recipe-pidfd"
            return "retired", raw
    return "replacement", raw


def inspect():
    auth = Path("wire").read_text().split()[0]
    wire = [int(fd) for fd in auth.split(",")]
    identity_file = Path("wire-identity")
    identity = tuple(map(int, identity_file.read_text().split(",")))
    identity_file.unlink()
    Path("seen-child").write_text(" ".join(describe(fd) for fd in (0, 1, 2)) + "\n")
    Path("seen-wire").write_text(" ".join(describe(fd) for fd in wire) + "\n")
    states = []
    raw_wires = {}
    for fd in wire:
        state, raw = parent_wire_state(fd, identity)
        states.append(f"parent{fd}={state}")
        raw_wires[fd] = raw
    Path("seen-parent").write_text(" ".join(states) + "\n")
    Path("seen-parent-raw.json").write_text(json.dumps(raw_wires) + "\n")


if sys.argv[1:] == ["inspect"]:
    inspect()
elif sys.argv[1] == "submake":
    exec_submake(sys.argv[2])
else:
    if sys.argv[1] != "open":
        for fd in sys.argv[1].split(","):
            os.close(int(fd))
    os.execv(sys.argv[2], sys.argv[2:])
