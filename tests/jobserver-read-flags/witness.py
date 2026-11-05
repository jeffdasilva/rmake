# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

import array
import fcntl
import json
import os
from pathlib import Path
import resource
import stat
import sys
import termios


def descriptor(fd):
    info = os.fstat(fd)
    return {"fd": fd, "dev": info.st_dev, "ino": info.st_ino,
            "pipe": stat.S_ISFIFO(info.st_mode), "status_flags": fcntl.fcntl(fd, fcntl.F_GETFL),
            "descriptor_flags": fcntl.fcntl(fd, fcntl.F_GETFD)}


def count(fd):
    result = array.array("i", [0])
    fcntl.ioctl(fd, termios.FIONREAD, result, True)
    return result[0]


def save(name, data):
    Path(name).write_text(json.dumps(data, sort_keys=True) + "\n")


def exec_make(limit, executable, args):
    resource.setrlimit(resource.RLIMIT_NOFILE, (limit, limit))
    os.execv(executable, [executable, *args])


def launch(limit, executable, args):
    expected = tuple(map(int, os.environ["WIRE_SLOTS"].split(",")))
    protocol = os.pipe2(os.O_CLOEXEC)
    other = os.pipe2(os.O_CLOEXEC)
    original = (*protocol, *other)
    aliases = tuple(fcntl.fcntl(fd, fcntl.F_DUPFD_CLOEXEC, 32) for fd in original)
    os.write(protocol[1], b"Q")
    os.write(other[1], b"U")
    before = [descriptor(fd) for fd in (*original, *aliases)]
    counts_before = [count(protocol[0]), count(other[0])]
    child = os.fork()
    if child == 0:
        for source, target in zip(aliases, expected, strict=True):
            os.dup2(source, target, inheritable=True)
        for fd in (*original, *aliases):
            if fd not in expected:
                os.close(fd)
        exec_make(limit, executable, args)

    _, status = os.waitpid(child, 0)
    after = [descriptor(fd) for fd in (*original, *aliases)]
    counts_after = [count(protocol[0]), count(other[0])]
    token_bytes = [os.read(fd, 1).hex() if available else ""
                   for fd, available in zip((protocol[0], other[0]), counts_after, strict=True)]
    save("supervisor.json", {"engine_pid": child, "before": before, "after": after,
                             "counts_before": counts_before, "counts_after": counts_after,
                             "tokens": token_bytes})
    sys.exit(os.waitstatus_to_exitcode(status))


def recipe(value):
    wire = tuple(map(int, value.removeprefix("--jobserver-auth=").split(","))) if value else ()
    parent = os.getppid()
    observed = {"protocol": [descriptor(fd) for fd in wire],
                "parent": parent, "image": os.readlink(f"/proc/{parent}/exe"),
                "limit": list(resource.getrlimit(resource.RLIMIT_NOFILE))}
    if "WIRE_SLOTS" in os.environ:
        unrelated = tuple(map(int, os.environ["WIRE_SLOTS"].split(",")[2:]))
        observed["unrelated"] = [descriptor(fd) for fd in unrelated]
        save("recipe.json", observed)
        print("child")
    else:
        reader, writer = wire
        observed["token_count"] = count(reader)
        save("recipe.json", observed)
        for name, endpoint in zip(("reader", "writer"), observed["protocol"], strict=True):
            status = "nonblocking" if endpoint["status_flags"] & os.O_NONBLOCK else "blocking"
            print(f"{name}={status}")


if __name__ == "__main__":
    if sys.argv[1] == "launch":
        limit, executable, args = int(sys.argv[2]), sys.argv[3], sys.argv[4:]
        if "WIRE_SLOTS" in os.environ:
            launch(limit, executable, args)
        else:
            exec_make(limit, executable, args)
    else:
        recipe(sys.argv[1])
