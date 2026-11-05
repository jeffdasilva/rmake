# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Control one borrowed lease and observe bytes at independently held stages."""
import array
import fcntl
import os
from pathlib import Path
import resource
import sys
import termios
import time

mode = sys.argv[1]
seed = bytes.fromhex(sys.argv[2])
if len(seed) != 1:
    raise SystemExit('fixture requires exactly one original byte')
reader, writer = os.pipe()
stderr_reader, stderr_writer = os.pipe()
if (reader, writer) != (3, 4):
    raise SystemExit('fixture did not receive its declared pipe slots')
os.write(writer, seed)
child = os.fork()
if child == 0:
    os.close(stderr_reader)
    os.dup2(stderr_writer, 2)
    os.close(stderr_writer)
    for fd in (reader, writer):
        os.set_inheritable(fd, True)
    os.environ['MAKEFLAGS'] = ' -j --jobserver-auth=3,4'
    resource.setrlimit(resource.RLIMIT_NOFILE, (8, 8))
    os.execv(sys.argv[3], sys.argv[3:])
os.close(stderr_writer)
os.set_blocking(stderr_reader, False)
errors = bytearray()


def drain():
    while True:
        try:
            chunk = os.read(stderr_reader, 65536)
        except BlockingIOError:
            return
        if not chunk:
            return
        errors.extend(chunk)
        view = memoryview(chunk)
        while view:
            view = view[os.write(2, view):]


def available():
    count = array.array('i', [0])
    fcntl.ioctl(reader, termios.FIONREAD, count, True)
    return count[0]


events = []
status = None
deadline = time.monotonic() + 4.5
while time.monotonic() < deadline:
    drain()
    pid, code = os.waitpid(child, os.WNOHANG)
    if pid:
        status = code
        break
    if mode == 'implicit-first':
        if not events and Path('b.held').exists():
            events.append(f'bytes-before-a-release={available()}')
            Path('a.release').touch(exist_ok=False)
        if len(events) == 1 and available() == 1:
            events.append('one-byte-before-b-release')
            Path('b.release').touch(exist_ok=False)
    elif mode == 'no-child':
        if (not events and Path('a.ready').exists() and Path('b.phase').exists()
                and available() == 1):
            events.append('one-byte-after-config-before-a-release')
            Path('a.release').touch(exist_ok=False)
    elif mode in ('body-abort', 'config-abort', 'continuation-abort'):
        if mode == 'continuation-abort' and not events and Path('b.held').exists():
            events.append(f'bytes-before-b-release={available()}')
            Path('b.release').touch(exist_ok=False)
        expected = int(mode == 'continuation-abort')
        if (len(events) == expected and Path('a.ready').exists() and Path('b.phase').exists()
                and b'Waiting for unfinished jobs....' in errors):
            events.append(f'bytes-at-abort-wait={available()}')
            Path('a.release').touch(exist_ok=False)
    else:
        if not events and Path('b.held').exists():
            events.append(f'bytes-before-b-release={available()}')
            Path('b.release').touch(exist_ok=False)
        if len(events) == 1 and available() == 1:
            events.append('one-byte-before-a-release')
            Path('a.release').touch(exist_ok=False)
    time.sleep(.002)
if status is None:
    os.kill(child, 9)
    os.waitpid(child, 0)
    raise SystemExit('release controller expired')
drain()
os.close(stderr_reader)
count = available()
events.append(f'final-count={count}')
events.append('returned=' + (os.read(reader, count).hex() if count else ''))
os.close(writer)
os.close(reader)
Path('observer').write_text('\n'.join(events) + '\n')
raise SystemExit(os.waitstatus_to_exitcode(status))
