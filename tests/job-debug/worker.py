# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original bounded worker: host verifies identity before acknowledging it."""
import os
from pathlib import Path
import re
import select
import sys
import time

name, mode, code = sys.argv[1], sys.argv[2], int(sys.argv[3])
deadline = time.monotonic() + 3


def wait(predicate):
    while not predicate():
        if time.monotonic() >= deadline:
            raise SystemExit(97)
        time.sleep(.002)


def identity(pid):
    try:
        fields = Path('/proc', str(pid), 'stat').read_bytes().rsplit(b') ', 1)[1].split()
        return int(fields[1]), int(fields[19])
    except (FileNotFoundError, ProcessLookupError):
        return None


def seen(phase):
    fd = os.open('seen', os.O_WRONLY | os.O_APPEND)
    try:
        os.write(fd, (phase + '-' + name + '\n').encode())
    finally:
        os.close(fd)


if name == 'second' and mode in ('pair', 'loan'):
    wait(lambda: Path('first.started').exists())
parent, start = identity(os.getpid())
temporary = Path(name + '.identity.tmp')
temporary.write_text(f'pid={os.getpid()} ppid={parent} start={start} target={name}\n')
temporary.rename(name + '.identity')
wait(lambda: Path(name + '.verified').read_bytes() == b'yes\n'
     if Path(name + '.verified').exists() else False)
seen('START')
Path(name + '.started').write_bytes(b'yes\n')
if name == 'first' and mode in ('pair', 'loan'):
    wait(lambda: Path('second.started').exists())
if name == 'second' and mode in ('pair', 'loan'):
    wait(lambda: Path('first.done').exists())
    fields = Path('first.identity').read_text().split()
    first = int(fields[0].split('=')[1])
    first_start = int(fields[2].split('=')[1])
    wait(lambda: identity(first) != (parent, first_start))
    wait(lambda: Path('finish-second').exists())
    if mode == 'loan':
        match = re.search(r'(?:^| )--jobserver-auth=([^ ]+)', os.environ['MAKEFLAGS'])
        if match is None:
            raise SystemExit(95)
        auth = match[1]
        fifo = auth.startswith('fifo:')
        if fifo:
            reader = writer = os.open(auth[5:], os.O_RDWR | os.O_NONBLOCK)
        else:
            reader, writer = map(int, auth.split(','))
        try:
            remaining = max(0, deadline - time.monotonic())
            ready, _, _ = select.select([reader], [], [], remaining)
            if not ready:
                raise SystemExit(96)
            token = os.read(reader, 1)
            if len(token) != 1 or os.write(writer, token) != 1:
                raise SystemExit(94)
            Path('borrowed-token').write_bytes(b'borrowed=' + token + b' returned=' + token + b'\n')
        finally:
            if fifo:
                os.close(reader)
seen('END')
Path(name + '.done').write_bytes(b'yes\n')
raise SystemExit(code)
