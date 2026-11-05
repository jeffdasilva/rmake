#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Observe a real capture endpoint while its direct writer remains alive."""
import os
from pathlib import Path
import sys
import time


def mark(value):
    with open('seen', 'ab') as journal:
        journal.write(value + b'\n')


def wait_for(predicate):
    deadline = time.monotonic() + 2
    while not predicate():
        if time.monotonic() >= deadline:
            return False
        time.sleep(.005)
    return True


def readers(parent, pipe):
    descriptors = Path('/proc', str(parent), 'fd')
    matching = []
    for descriptor in descriptors.iterdir():
        try:
            if os.readlink(descriptor) != pipe:
                continue
            metadata = Path('/proc', str(parent), 'fdinfo', descriptor.name).read_bytes()
        except FileNotFoundError:
            continue
        flags = next(line.split()[1] for line in metadata.splitlines() if line.startswith(b'flags:'))
        if int(flags, 8) & os.O_ACCMODE == os.O_RDONLY:
            matching.append(descriptor.name)
    return matching


def close_writer():
    parent = os.getppid()
    if Path('/proc', str(parent), 'comm').read_bytes() != b'make\n':
        mark(b'child:not-direct')
        os._exit(61)
    pipe = os.readlink('/proc/self/fd/1')
    if not pipe.startswith('pipe:[') or len(readers(parent, pipe)) != 1:
        mark(b'child:missing-unique-reader')
        os._exit(62)
    mark(b'child:saw-reader')
    os.write(1, b'left\nright\n')
    os.close(1)
    mark(b'child:writer-closed')
    if not wait_for(lambda: not readers(parent, pipe)):
        mark(b'child:retained-reader')
        os._exit(27)
    if os.getppid() != parent:
        mark(b'child:parent-changed')
        os._exit(63)
    mark(b'child:reader-closed')
    os._exit(7)


def descendant_writer():
    parent = os.getpid()
    os.write(1, b'first\n')
    mark(b'child:main-writer')
    if os.fork() != 0:
        os._exit(9)
    # Kernel reparenting proves original child exit before the late write,
    # independently of whether the engine has reaped that child yet.
    if not wait_for(lambda: os.getppid() != parent):
        mark(b'child:original-writer-still-live')
        os._exit(64)
    os.write(1, b'late\n')
    mark(b'child:descendant-wrote')
    os._exit(0)


if sys.argv[1:] == ['eof']:
    close_writer()
elif sys.argv[1:] == ['descendant']:
    descendant_writer()
else:
    os._exit(65)
