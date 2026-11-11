# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

import os
from pathlib import Path
import time


RECORD = Path('named.pid')


def process_start(pid):
    try:
        fields = Path('/proc', str(pid), 'stat').read_bytes().rsplit(b')', 1)[1].split()
    except (FileNotFoundError, ProcessLookupError):
        return None
    return fields[19]


def wait_for_named_reap():
    deadline = time.monotonic() + 4
    identity = None
    while time.monotonic() < deadline:
        if identity is None and RECORD.is_file():
            pid, start = RECORD.read_bytes().split()
            identity = (int(pid), start)
        if identity is not None and process_start(identity[0]) != identity[1]:
            RECORD.unlink()
            return
        time.sleep(.002)
    raise RuntimeError('named include was not reaped before the deadline')


if __name__ == '__main__':
    pid = os.getpid()
    start = process_start(pid)
    if start is None:
        raise RuntimeError('could not read the named include process identity')
    temporary = RECORD.with_suffix('.tmp')
    temporary.write_bytes(str(pid).encode() + b' ' + start + b'\n')
    temporary.replace(RECORD)
    Path('named.gen').write_bytes(b'NAMED := loaded\n')
