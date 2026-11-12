# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

import os
from pathlib import Path
import sys
import time


def process_start(pid):
    try:
        fields = Path('/proc', str(pid), 'stat').read_bytes().rsplit(b')', 1)[1].split()
    except (FileNotFoundError, ProcessLookupError):
        return None
    return fields[19]


def wait_for_reap(name):
    record = Path(name)
    deadline = time.monotonic() + 4
    identity = None
    while time.monotonic() < deadline:
        if identity is None and record.is_file():
            pid, start = record.read_bytes().split()
            identity = (int(pid), start)
        if identity is not None and process_start(identity[0]) != identity[1]:
            record.unlink()
            return
        time.sleep(.002)
    raise RuntimeError('include worker was not reaped before the deadline')


def record_process(name):
    record = Path(name)
    pid = os.getpid()
    start = process_start(pid)
    if start is None:
        raise RuntimeError('could not read the include worker process identity')
    temporary = record.with_suffix('.tmp')
    temporary.write_bytes(str(pid).encode() + b' ' + start + b'\n')
    temporary.replace(record)


if __name__ == '__main__':
    args = sys.argv[1:]
    if args == ['write-right-tail']:
        record_process('right-tail.pid')
        with Path('seen').open('ab') as stream:
            stream.write(b'tail\n')
    else:
        if args == ['after-right']:
            wait_for_reap('right.pid')
        elif args == ['after-right-tail']:
            wait_for_reap('right-tail.pid')
        else:
            record_process('named.pid')
        Path('named.gen').write_bytes(b'NAMED := loaded\n')
