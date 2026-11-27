# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Launch with controlled fd 0 or record the child's actual input descriptor."""
import errno
import os
from pathlib import Path
import sys


def inspect(phase):
    try:
        actual = os.fstat(0)
    except OSError as error:
        if error.errno != errno.EBADF:
            raise
        observed = "closed"
    else:
        expected = Path("input").stat()
        observed = "input" if os.path.samestat(actual, expected) else "other"
    with Path("seen").open("a") as witness:
        witness.write(f"{phase}={observed}\n")


action, *arguments = sys.argv[1:]
if action == "inspect":
    inspect(*arguments)
else:
    if action == "closed":
        os.close(0)
    elif action == "inherited":
        with Path("input").open("rb") as source:
            os.dup2(source.fileno(), 0, inheritable=True)
    else:
        raise SystemExit(f"unknown stdin action {action}")
    os.execv(arguments[0], arguments)
