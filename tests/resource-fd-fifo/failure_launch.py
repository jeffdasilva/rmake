#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Observe the exact child-owned FIFO path with no pre-existing occupant."""
import errno
import json
import os
from pathlib import Path
import resource
import stat
import sys

directory = Path(os.environ['TMPDIR'])
if list(directory.iterdir()):
    raise SystemExit('expected an empty private temporary directory')
for descriptor in (0, 1, 2):
    os.fstat(descriptor)
try:
    os.fstat(3)
except OSError as error:
    if error.errno != errno.EBADF:
        raise
else:
    raise SystemExit('unexpected preexisting descriptor 3')

owner = os.fork()
if owner == 0:
    resource.setrlimit(resource.RLIMIT_NOFILE, (4, 4))
    os.execv(sys.argv[1], sys.argv[1:])
waited, status = os.waitpid(owner, 0)
assert waited == owner
try:
    os.waitpid(owner, os.WNOHANG)
except ChildProcessError:
    reaped = True
else:
    reaped = False
path = directory / f'GMfifo{owner}'


def identity(metadata):
    return [metadata.st_dev, metadata.st_ino]


observation = {'owner_pid': owner, 'owner_uid': os.geteuid(), 'exact_child_reaped': reaped,
               'soft_limit': 4, 'hard_limit': 4, 'owner_wait_status': os.waitstatus_to_exitcode(status)}
try:
    original = path.lstat()
except FileNotFoundError:
    observation['path'] = None
else:
    observation['path'] = {
        'basename': path.name, 'identity': identity(original), 'fifo': stat.S_ISFIFO(original.st_mode),
        'mode': stat.S_IMODE(original.st_mode), 'uid': original.st_uid, 'gid': original.st_gid,
        'nlink': original.st_nlink, 'size': original.st_size,
    }
    # Never open a non-FIFO or follow a symlink, even in the disposable fixture.
    if stat.S_ISFIFO(original.st_mode):
        flags = os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC
        try:
            writer = os.open(path, os.O_WRONLY | flags)
        except OSError as error:
            observation['writer_probe_errno'] = error.errno
        else:
            observation['writer_probe_errno'] = 0
            os.close(writer)
        reader = os.open(path, os.O_RDONLY | flags)
        try:
            observation['reader_probe_identity'] = identity(os.fstat(reader))
            try:
                observation['reader_probe_data_hex'] = os.read(reader, 1).hex()
            except OSError as error:
                observation['reader_probe_errno'] = error.errno
        finally:
            os.close(reader)
        observation['post_probe_identity'] = identity(path.lstat())

Path('observer.json').write_text(json.dumps(observation, sort_keys=True) + '\n')
raise SystemExit(os.waitstatus_to_exitcode(status))
