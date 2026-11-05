# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Independent owner of an original inherited jobserver pipe and token."""
import array
import fcntl
import os
from pathlib import Path
import resource
import signal
import sys
import termios
import time

limit, mode = int(sys.argv[1]), sys.argv[2]
reader, writer = os.pipe()
if (reader, writer) != (3, 4):
    raise SystemExit('fixture did not receive its declared wire numbers')
identity = os.fstat(reader)
os.write(writer, b'+' if mode == 'tokens' else b'Q')
child = os.fork()
if child == 0:
    for fd in (reader, writer):
        os.set_inheritable(fd, True)
    os.environ['MAKEFLAGS'] = ' -j --jobserver-auth=3,4'
    os.environ['RM_PIPE_DEV'] = str(identity.st_dev)
    os.environ['RM_PIPE_INO'] = str(identity.st_ino)
    resource.setrlimit(resource.RLIMIT_NOFILE, (limit, limit))
    os.execv(sys.argv[3], sys.argv[3:])

events = []
status = None
deadline = time.monotonic() + 4.5
while time.monotonic() < deadline:
    pid, code = os.waitpid(child, os.WNOHANG)
    if pid:
        status = code
        break
    if mode == 'tokens' and not events and all(Path(name + '.ready').exists() for name in 'ab'):
        count = array.array('i', [0])
        fcntl.ioctl(reader, termios.FIONREAD, count, True)
        events.append(f'live-token-bytes={count[0]}')
        Path('a.release').touch(exist_ok=False)
    if mode == 'tokens' and len(events) == 1 and Path('a.done').exists():
        events.append('a-done-before-b-release')
        Path('b.release').touch(exist_ok=False)
    time.sleep(.002)
if status is None:
    os.kill(child, signal.SIGKILL)
    os.waitpid(child, 0)
    raise SystemExit('fixture make did not finish before the local deadline')
read_flags = fcntl.fcntl(reader, fcntl.F_GETFL)
write_flags = fcntl.fcntl(writer, fcntl.F_GETFL)
events.append(f'shared-reader-nonblocking={int(bool(read_flags & os.O_NONBLOCK))}')
events.append(f'shared-writer-nonblocking={int(bool(write_flags & os.O_NONBLOCK))}')
os.close(writer)
events.append('returned=' + os.read(reader, 64).hex())
os.close(reader)
Path('observer').write_text('\n'.join(events) + '\n')
if os.WIFSIGNALED(status):
    number = os.WTERMSIG(status)
    signal.signal(number, signal.SIG_DFL)
    os.kill(os.getpid(), number)
raise SystemExit(os.waitstatus_to_exitcode(status))
