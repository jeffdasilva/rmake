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


def verify_pipe(reader, writer, access):
    if not (stat.S_ISFIFO(reader.st_mode) and stat.S_ISFIFO(writer.st_mode)):
        raise RuntimeError("wire descriptors must be pipes")
    if (reader.st_dev, reader.st_ino) != (writer.st_dev, writer.st_ino):
        raise RuntimeError("wire descriptors must identify the same pipe")
    if access != [os.O_RDONLY, os.O_WRONLY]:
        raise RuntimeError("wire directions must be read and write")


def exec_submake(executable):
    auth, = [argument.removeprefix("--jobserver-auth=")
             for argument in os.environ["MAKEFLAGS"].split()
             if argument.startswith("--jobserver-auth=")]
    read, write = map(int, auth.split(","))
    reader, writer = (os.fstat(fd) for fd in (read, write))
    access = [fcntl.fcntl(fd, fcntl.F_GETFL) & os.O_ACCMODE for fd in (read, write)]
    verify_pipe(reader, writer, access)
    Path("wire").write_text(f"{read},{write} same-pipe read-only write-only\n")
    Path("wire-identity").write_text(f"{reader.st_dev},{reader.st_ino}\n")
    os.execv(executable, [executable, "-f", "Child.mk", "child"])


def parent_wire_state(parent, descriptor, identity):
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


def inspect_active_fifo(parent, borrowed_identity, path):
    metadata = path.stat()
    identity = (metadata.st_dev, metadata.st_ino)
    mode = stat.S_IMODE(metadata.st_mode)
    if path.parent != Path(os.environ["TMPDIR"]) or not stat.S_ISFIFO(metadata.st_mode):
        raise RuntimeError("active jobserver must be a FIFO in TMPDIR")
    if mode != 0o600 or identity == borrowed_identity:
        raise RuntimeError("active FIFO must be private and distinct from the borrowed pipe")

    wire = []
    borrowed_present = False
    for entry in (parent / "fd").iterdir():
        try:
            endpoint = entry.stat()
        except FileNotFoundError:
            continue
        endpoint_identity = (endpoint.st_dev, endpoint.st_ino)
        if endpoint_identity == identity:
            wire.append(int(entry.name))
        if endpoint_identity == borrowed_identity:
            borrowed_present = True
    wire.sort()
    if len(wire) != 2:
        raise RuntimeError("parent must hold both active FIFO endpoints")
    borrowed_state = "present" if borrowed_present else "absent"
    states = " ".join(describe(fd) for fd in wire)
    Path("seen-active").write_text(
        f"fifo {wire[0]},{wire[1]} new mode={mode:04o} borrowed={borrowed_state} {states}\n"
    )
    return {"path": str(path), "device": metadata.st_dev, "inode": metadata.st_ino,
            "mode": f"{mode:04o}", "descriptors": wire, "borrowed_present": borrowed_present}


def inspect_active_wire(parent, borrowed_identity):
    auth = Path("child-auth").read_text().strip()
    if not auth:
        return
    if auth.startswith("--jobserver-auth=fifo:"):
        path = Path(auth.removeprefix("--jobserver-auth=fifo:"))
        return inspect_active_fifo(parent, borrowed_identity, path)
    wire = [int(fd) for fd in auth.removeprefix("--jobserver-auth=").split(",")]
    reader, writer = ((parent / "fd" / str(fd)).stat() for fd in wire)
    access = []
    for fd in wire:
        info = (parent / "fdinfo" / str(fd)).read_text().splitlines()
        flags, = [line.split()[1] for line in info if line.startswith("flags:")]
        access.append(int(flags, 8) & os.O_ACCMODE)
    verify_pipe(reader, writer, access)
    state = "original" if (reader.st_dev, reader.st_ino) == borrowed_identity else "new"
    states = " ".join(describe(fd) for fd in wire)
    Path("seen-active").write_text(f"{wire[0]},{wire[1]} {state} {states}\n")


def inspect_forced_output(parent, wire, borrowed_identity):
    raw = {}
    states = []
    for fd in sorted({1, 2, *wire}):
        metadata = os.fstat(fd)
        identity = (metadata.st_dev, metadata.st_ino)
        flags = fcntl.fcntl(fd, fcntl.F_GETFD)
        matches = []
        for entry in (parent / "fd").iterdir():
            try:
                endpoint = entry.stat()
            except FileNotFoundError:
                continue
            if (endpoint.st_dev, endpoint.st_ino) == identity:
                matches.append(int(entry.name))
        if identity == borrowed_identity:
            state = "wire"
        elif (matches and stat.S_ISREG(metadata.st_mode)
              and stat.S_IMODE(metadata.st_mode) == 0o600 and metadata.st_nlink == 0):
            state = "spool"
        else:
            state = "other"
        states.append(f"fd{fd}={state}/flags={flags}")
        raw[fd] = {"identity": identity, "parent_descriptors": sorted(matches),
                   "link": os.readlink(f"/proc/self/fd/{fd}"), "flags": flags, "state": state,
                   "mode": f"{stat.S_IMODE(metadata.st_mode):04o}", "link_count": metadata.st_nlink}
    if raw[1]["identity"] != raw[2]["identity"]:
        states.append("distinct-outputs")
    Path("seen-forced").write_text(" ".join(states) + "\n")
    return raw


def inspect(forced=False):
    if forced:
        flags = os.environ["MAKEFLAGS"].split()
        auth, = [flag for flag in flags if flag.startswith("--jobserver-auth=")]
        output, = [flag for flag in flags if flag.startswith("-O")]
        Path("child-auth").write_text(auth + "\n")
        Path("seen-output-mode").write_text(output + "\n")
    auth = Path("wire").read_text().split()[0]
    wire = [int(fd) for fd in auth.split(",")]
    identity_file = Path("wire-identity")
    identity = tuple(map(int, identity_file.read_text().split(",")))
    identity_file.unlink()
    Path("seen-child").write_text(" ".join(describe(fd) for fd in (0, 1, 2)) + "\n")
    Path("seen-wire").write_text(" ".join(describe(fd) for fd in wire) + "\n")
    parent = Path(f"/proc/{os.getppid()}")
    states = []
    raw_wires = {}
    for fd in wire:
        state, raw = parent_wire_state(parent, fd, identity)
        states.append(f"parent{fd}={state}")
        raw_wires[fd] = raw
    Path("seen-parent").write_text(" ".join(states) + "\n")
    active_fifo = inspect_active_wire(parent, identity)
    if active_fifo is not None:
        raw_wires["active_fifo"] = active_fifo
    if forced:
        raw_wires["forced_child"] = inspect_forced_output(parent, wire, identity)
    Path("seen-parent-raw.json").write_text(json.dumps(raw_wires) + "\n")


if sys.argv[1:] == ["inspect"]:
    inspect()
elif sys.argv[1:] == ["inspect-forced"]:
    inspect(forced=True)
elif sys.argv[1] == "submake":
    exec_submake(sys.argv[2])
else:
    if sys.argv[1] != "open":
        for fd in sys.argv[1].split(","):
            os.close(int(fd))
    os.execv(sys.argv[2], sys.argv[2:])
