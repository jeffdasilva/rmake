#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Give the engine a deleted initial cwd or a -C destination kept by a dirfd."""
import os
from pathlib import Path
import sys

mode, root, program, *arguments = sys.argv[1:]
work = Path(root)
assert work.is_absolute() and Path.cwd() == work
assert mode in ("initial", "after-C") and Path(program).is_absolute()
deleted = work / ".deleted-cwd"
deleted.mkdir(mode=0o700)
if mode == "initial":
    os.chdir(deleted)
else:
    assert arguments.count("/proc/self/fd/197") == 1
    owner = os.open(deleted, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
    if owner == 197:
        os.set_inheritable(owner, True)
    else:
        os.dup2(owner, 197, inheritable=True)
        os.close(owner)
os.rmdir(deleted)
os.execve(program, [program, *arguments], os.environ)
