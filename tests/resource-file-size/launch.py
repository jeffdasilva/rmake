# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original capped-write launcher with a kernel source lifecycle witness."""
import ctypes
import os
from pathlib import Path
import resource
import signal
import struct
import sys

CREATE, DELETE, CLOSE_WRITE = 0x100, 0x200, 0x8
EVENT_NAMES = {CREATE: 'create', DELETE: 'delete', CLOSE_WRITE: 'close-write'}
EVENT = struct.Struct('=iIII')

mode, limit = sys.argv[1:3]
watch = None
if mode.startswith('stdin'):
    libc = ctypes.CDLL(None, use_errno=True)
    libc.inotify_init1.argtypes = [ctypes.c_int]
    libc.inotify_add_watch.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_uint32]
    watch = libc.inotify_init1(os.O_CLOEXEC | os.O_NONBLOCK)
    if watch < 0:
        raise OSError(ctypes.get_errno(), 'inotify_init1')
    watch_id = libc.inotify_add_watch(watch, os.fsencode(os.environ['TMPDIR']),
                                      CREATE | DELETE | CLOSE_WRITE)
    if watch_id < 0:
        raise OSError(ctypes.get_errno(), 'inotify_add_watch')

child = os.fork()
if child == 0:
    if mode.startswith('stdin'):
        source = os.open('input.mk', os.O_RDONLY)
        os.dup2(source, 0)
        os.close(source)
    signal.signal(signal.SIGXFSZ, signal.SIG_DFL if mode == 'stdin-signal' else signal.SIG_IGN)
    resource.setrlimit(resource.RLIMIT_FSIZE, (int(limit), int(limit)))
    os.execv(sys.argv[3], sys.argv[3:])

_pid, status = os.waitpid(child, 0)
if watch is not None:
    raw = bytearray()
    while True:
        try:
            raw.extend(os.read(watch, 8192))
        except BlockingIOError:
            break
        if len(raw) > 65536:
            raise SystemExit('unexpected temporary activity')
    os.close(watch)
    events = []
    offset = 0
    while offset < len(raw):
        identifier, mask, cookie, size = EVENT.unpack_from(raw, offset)
        offset += EVENT.size
        name = bytes(raw[offset:offset + size]).split(b'\0', 1)[0]
        offset += size
        if identifier != watch_id or cookie or mask not in EVENT_NAMES or not name:
            raise SystemExit('unexpected inotify event')
        events.append(EVENT_NAMES[mask].encode() + b' ' + name + b'\n')
    Path('observer').write_bytes(b''.join(events))

if os.WIFSIGNALED(status):
    number = os.WTERMSIG(status)
    signal.signal(number, signal.SIG_DFL)
    signal.pthread_sigmask(signal.SIG_UNBLOCK, {number})
    os.kill(os.getpid(), number)
raise SystemExit(os.waitstatus_to_exitcode(status))
