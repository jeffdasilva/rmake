# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Observe FIFO identity, borrowed live capacity, and bounded startup effects."""
import array
import fcntl
import os
from pathlib import Path
import resource
import stat
import sys
import termios
import time

mode, limit, seed = sys.argv[1:4]
directory = Path(os.environ['TMPDIR'])
keeper = None
inherited = None
events = []
if mode == 'borrowed':
    path = directory / 'borrowed.fifo'
    os.mkfifo(path, 0o600)
    keeper = os.open(path, os.O_RDWR | os.O_NONBLOCK)
    os.write(keeper, bytes.fromhex(seed))
    os.environ['MAKEFLAGS'] = ' -j --jobserver-auth=fifo:' + str(path)
elif mode == 'inherited-owned':
    inherited = os.open('inherited.txt', os.O_RDONLY)
    if inherited != 3:
        raise SystemExit('launcher did not receive the declared descriptor')
    os.set_inheritable(inherited, True)
pid = os.fork()
if pid == 0:
    resource.setrlimit(resource.RLIMIT_NOFILE, (int(limit), int(limit)))
    os.execv(sys.argv[4], sys.argv[4:])
if mode != 'borrowed':
    path = directory / f'GMfifo{pid}'


def available():
    value = array.array('i', [0])
    fcntl.ioctl(keeper, termios.FIONREAD, value, True)
    return value[0]


status = None
end = time.monotonic() + 5
while time.monotonic() < end:
    done, code = os.waitpid(pid, os.WNOHANG)
    if done:
        status = code
        break
    if keeper is not None:
        if not events and Path('b.held').exists():
            events.append(f'bytes-before-b-release={available()}')
            Path('b.release').touch(exist_ok=False)
        if len(events) == 1 and available() == 1:
            events.append('one-byte-before-a-release')
            Path('a.release').touch(exist_ok=False)
    time.sleep(.002)
if status is None:
    os.kill(pid, 9)
    os.waitpid(pid, 0)
    raise SystemExit('FIFO resource witness expired')

try:
    metadata = path.lstat()
except FileNotFoundError:
    metadata = None
    events.append('pool-path=absent')
else:
    events.append(f'pool-path={"fifo" if stat.S_ISFIFO(metadata.st_mode) else "other"}'
                  f':{stat.S_IMODE(metadata.st_mode):04o}')
if keeper is not None:
    original = os.fstat(keeper)
    same = metadata is not None and os.path.samestat(original, metadata)
    events.append(f'pool-identity={"same" if same else "changed"}')
    count = available()
    events.append(f'final-count={count}')
    events.append('returned=' + (os.read(keeper, count).hex() if count else ''))
    os.close(keeper)
if inherited is not None:
    os.close(inherited)
Path('observer').write_text('\n'.join(events) + '\n')
raise SystemExit(os.waitstatus_to_exitcode(status))
