#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original bounded GNU/rmake signal campaign with controlled delivery phases."""
from __future__ import annotations
import argparse
import array
import fcntl
import dataclasses
import json
import os
from pathlib import Path
import selectors
import signal
import subprocess
import tempfile
import time
import conformance as harness
import oracle

ROOT = Path(__file__).resolve().parents[1]
SIGNALS = (signal.SIGHUP, signal.SIGINT, signal.SIGQUIT, signal.SIGPIPE,
           signal.SIGTERM, signal.SIGXCPU, signal.SIGXFSZ)
NAMES = {signal.SIGHUP: b'Hangup', signal.SIGINT: b'Interrupt',
         signal.SIGQUIT: b'Quit', signal.SIGPIPE: b'Broken pipe',
         signal.SIGTERM: b'Terminated', signal.SIGXCPU: b'CPU time limit exceeded',
         signal.SIGXFSZ: b'File size limit exceeded'}
PROTECTED = b'Original signal campaign protected bytes.\n'
HELPER = b'''import ctypes
import os
from pathlib import Path
import signal
import sys
import time
assert ctypes.CDLL(None).prctl(4, 0, 0, 0, 0) == 0, 'disable kernel core files'
received = 0
def record(number, frame):
    global received
    received = number
    Path('received.signal').write_text(str(number))
for number in (1, 2, 3, 13, 15, 24, 25):
    signal.signal(number, record)
import re
pool = set()
if len(sys.argv) > 2:
    match = re.search(r'--jobserver-auth=(\\d+),(\\d+)', os.environ.get('MAKEFLAGS', ''))
    if match:
        pool = set(map(int, match.groups()))
        identities = [os.fstat(fd) for fd in pool]
        assert len(identities) == 2 and identities[0].st_ino == identities[1].st_ino and identities[0].st_dev == identities[1].st_dev
for descriptor in os.listdir('/proc/self/fd'):
    if int(descriptor) > 2 and int(descriptor) not in pool:
        try:
            os.fstat(int(descriptor))
        except OSError:
            continue
        raise RuntimeError('unexpected inherited runtime descriptor')
kind = sys.argv[1]
if kind == 'directory':
    Path('out').mkdir()
elif kind == 'archive':
    Path('lib.a').write_bytes(b'!<arch>\\n' + b'm.o/'.ljust(16) + b'1700000001'.ljust(12) + b'0'.ljust(6) * 2 + b'644'.ljust(8) + b'0'.ljust(10) + b'`\\n')
elif kind != 'unchanged':
    Path('out').write_text('changed\\n')
    if kind == 'same-mtime':
        os.utime('out', ns=(1700000000000000000, 1700000000000000000))
if kind == 'grouped':
    Path('peer').write_text('peer\\n')
Path('pid.tmp').write_text(str(os.getpid()))
Path('pid.tmp').rename('ready.pid')
deadline = time.monotonic() + 4
while not Path('release').exists():
    if time.monotonic() > deadline:
        raise RuntimeError('signal controller did not release live child')
    time.sleep(.002)
if received:
    signal.signal(received, signal.SIG_DFL)
    os.kill(os.getpid(), received)
'''

PAIR_HELPER = b'''import ctypes
import os
from pathlib import Path
import signal
import sys
import time
name = sys.argv[1]
received = 0
def record(number, frame):
    global received
    received = number
    Path(name + '.signal').write_text(str(number))
assert ctypes.CDLL(None).prctl(4, 0, 0, 0, 0) == 0, 'disable kernel core files'
for number in (1, 2, 3, 13, 15, 24, 25):
    signal.signal(number, record)
Path(name).write_text(name + '\\n')
Path(name + '.pid.tmp').write_text(str(os.getpid()))
Path(name + '.pid.tmp').rename(name + '.pid')
deadline = time.monotonic() + 4
while not Path(name + '.release').exists():
    if time.monotonic() > deadline:
        raise RuntimeError('parallel signal controller did not release child')
    time.sleep(.002)
if received:
    signal.signal(received, signal.SIG_DFL)
    os.kill(os.getpid(), received)
'''

@dataclasses.dataclass(frozen=True)
class Cell:
    number: int
    delivery: str
    jobs: int = 1
    style: str = 'pipe'
    kind: str = 'changed'
    ignored: bool = False
    mode: str = ''
    second: int = 0
    unstaged: bool = False

    @property
    def id(self):
        return f'{signal.Signals(self.number).name.lower()}-{self.delivery}-j{self.jobs}-{self.style}-{self.kind}' + ('-inherited-ignore' if self.ignored else '') + ('-mode-' + self.mode if self.mode else '') + ('-then-' + signal.Signals(self.second).name.lower() if self.second else '')

    @property
    def source(self):
        if self.kind == 'pair':
            return b'.PHONY: all FORCE\nall: a b\nFORCE:\na: FORCE\n\t@python3 helper.py a\n\t@printf tail > a.tail\nb: FORCE\n\t@python3 helper.py b\n\t@printf tail > b.tail\n'
        lines = ['.PHONY: all FORCE', 'all: out', 'FORCE:']
        if self.kind == 'oneshell': lines.append('.ONESHELL:')
        if self.kind == 'load': lines[1] = 'all: out b c'
        if self.kind == 'token-wait': lines[1] = 'all: out b'
        if self.kind == 'precious': lines.append('.PRECIOUS: out')
        if self.kind == 'phony': lines.append('.PHONY: out')
        if self.kind == 'intermediate':
            lines[1] = 'all: built .WAIT out'
            lines += ['.INTERMEDIATE: mid', 'built: mid', 'mid:;@printf mid > mid']
        lines.append('out peer &: FORCE' if self.kind == 'grouped' else 'out: FORCE')
        lines.append('\t' + ('+' if self.mode else '') + ('-@' if self.kind == 'ignore-recipe' else '@') + ('exec ' if self.kind == 'oneshell' else '') + 'python3 helper.py ' + self.kind + (' ' + self.mode if self.mode else ''))
        if self.kind != 'last': lines.append('\t@printf tail > tail')
        if self.kind == 'token-wait':
            lines += ['b:', '\t@$(file >queued-b)printf b > b']
        if self.kind == 'load':
            lines += ['b:', '\t@$(file >queued-b)printf b > b', 'c:', '\t@$(file >queued-c)printf c > c']
        if self.kind == 'archive':
            lines[1] = 'all: lib.a(m.o)'
            lines = [line.replace('out: FORCE', 'lib.a(m.o): FORCE') for line in lines]
        return ('\n'.join(lines) + '\n').encode()

    @property
    def line(self):
        return next(i for i, line in enumerate(self.source.splitlines(), 1) if b'python3 helper.py' in line)

    @property
    def deletes(self):
        changed_regular = self.kind not in ('precious', 'phony', 'unchanged', 'same-mtime', 'directory', 'archive')
        if self.ignored: return False
        if self.delivery == 'child': return changed_regular and self.kind != 'ignore-recipe'
        if self.number in (1, 2, 3, 15): return changed_regular
        return changed_regular and (self.delivery == 'group' or self.kind not in ('last', 'oneshell'))

    @property
    def expected(self):
        if self.second:
            deletion = b"make: *** Deleting file 'out'\n"
            waiting = b'make: *** Waiting for unfinished jobs....\n'
            first_deletes = self.number in (1, 2, 3, 15)
            second_deletes = self.second in (1, 2, 3, 15)
            error = deletion if first_deletes else waiting
            if first_deletes and not second_deletes: error += waiting
            if not first_deletes and second_deletes: error += deletion
            if self.second == 15:
                error += f'make: *** [Makefile:{self.line}: out] Terminated\n'.encode()
            if not first_deletes and not second_deletes: error += deletion
            return b'', error, 1 if self.second == 3 else -self.second
        if self.kind == 'pair':
            a = b'make: *** [Makefile:5: a] ' + NAMES[self.number] + b'\n'
            b = b'make: *** [Makefile:8: b] ' + NAMES[self.number] + b'\n'
            da = b"make: *** Deleting file 'a'\n"
            db = b"make: *** Deleting file 'b'\n"
            waiting = b'make: *** Waiting for unfinished jobs....\n'
            status = 2 if self.delivery == 'child' else 1 if self.number == 3 else -self.number
            if self.delivery == 'child': error = a + da + waiting
            elif self.number in (1, 2, 3, 15):
                error = db + da + (a + b if self.delivery == 'group' or self.number == 15 else b'')
            elif self.delivery == 'group': error = waiting + a + da + b + db
            else: error = waiting + da + db
            return (b'python3 helper.py ' + self.kind.encode() + b' n\n' if self.mode == 'n' else b''), error, status
        if self.ignored:
            return b'', b'', 0
        status = (2 if self.delivery == 'child' and self.kind != 'ignore-recipe' else 0) if self.delivery == 'child' else (1 if self.number == 3 else -self.number)
        deletion = b"make: *** Deleting file 'out'\n" if self.deletes else b''
        if self.kind == 'archive':
            deletion = b"make: *** Archive member 'lib.a(m.o)' may be bogus; not deleted\n"
        if self.deletes and self.kind == 'grouped':
            deletion += b"make: *** [out] Deleting file 'peer'\n"
        child_error = b''
        if self.delivery != 'parent' or self.number == 15:
            child_error = ((b'make: ' if self.kind == 'ignore-recipe' else b'make: *** ') +
                           f'[Makefile:{self.line}: {"lib.a(m.o)" if self.kind == "archive" else "out"}] '.encode() + NAMES[self.number] +
                           (b' (ignored)' if self.kind == 'ignore-recipe' else b'') + b'\n')
        if self.delivery == 'child':
            error = child_error + deletion
        elif self.number in (1, 2, 3, 15):
            error = deletion + child_error
        else:
            error = b'make: *** Waiting for unfinished jobs....\n' + child_error + deletion
        if self.kind == 'intermediate':
            error += b"make: *** Deleting intermediate file 'mid'\n" if self.delivery != 'child' else b''
        return (b'python3 helper.py ' + self.kind.encode() + b' n\n' if self.mode == 'n' else b''), error, status


def inventory():
    cells = [Cell(number, delivery, jobs, style)
             for number in SIGNALS for delivery in ('parent', 'group', 'child')
             for jobs, style in ((1, 'pipe'), (2, 'pipe'), (2, 'fifo'))]
    for kind in ('precious', 'phony', 'unchanged', 'same-mtime', 'directory',
                 'symlink', 'grouped', 'intermediate', 'ignore-recipe', 'last'):
        for number in (signal.SIGINT, signal.SIGTERM, signal.SIGPIPE):
            for delivery in ('parent', 'group', 'child'):
                # GNU's normal child-only failure reaches ordinary rm formatting.
                # The intermediate profile tests actual invocation interruption.
                if kind == 'intermediate' and delivery == 'child': continue
                cells.append(Cell(number, delivery, 2, 'pipe', kind))
    cells += [Cell(number, 'parent', 2, 'pipe', ignored=True) for number in SIGNALS]
    cells += [Cell(number, delivery, 2, style, 'pair') for number in SIGNALS
              for delivery in ('parent', 'group', 'child') for style in ('pipe', 'fifo')]
    cells += [Cell(number, delivery, 2, 'pipe', mode=mode) for number in SIGNALS
              for delivery in ('parent', 'group', 'child') for mode in ('n', 'q', 't')]
    cells += [Cell(number, delivery, 2, 'pipe', 'oneshell') for number in SIGNALS
              for delivery in ('parent', 'group', 'child')]
    cells += [Cell(number, delivery, 4, 'pipe', 'load') for number in SIGNALS
              for delivery in ('parent', 'group', 'child')]
    cells += [Cell(first, 'parent', jobs, second=second)
              for first in (signal.SIGINT, signal.SIGPIPE)
              for second in (signal.SIGHUP, signal.SIGTERM, signal.SIGXCPU) for jobs in (1, 2)]
    cells += [Cell(number, delivery, 2, 'pipe', 'archive') for number in SIGNALS
              for delivery in ('parent', 'group', 'child')]
    return tuple(cells)


def materialize(cell, path):
    path.mkdir()
    (path / 'Makefile').write_bytes(cell.source)
    helper = PAIR_HELPER if cell.kind == 'pair' else HELPER
    if cell.unstaged: helper = helper.replace(b'signal.signal(number, record)', b'signal.signal(number, signal.SIG_DFL)')
    (path / 'helper.py').write_bytes(helper)
    (path / 'protected').write_bytes(PROTECTED)
    if cell.kind == 'archive':
        (path / 'lib.a').write_bytes(b'!<arch>\n' + b'm.o/'.ljust(16) + b'1700000000'.ljust(12) + b'0'.ljust(6) * 2 + b'644'.ljust(8) + b'0'.ljust(10) + b'`\n')
    if cell.kind in ('unchanged', 'same-mtime', 'symlink'):
        target = path / ('referent' if cell.kind == 'symlink' else 'out')
        target.write_bytes(b'original\n')
        os.utime(target, ns=(1700000000000000000, 1700000000000000000))
        if cell.kind == 'symlink': (path / 'out').symlink_to('referent')


def caught(process, number):
    try:
        row = next(line for line in Path(f'/proc/{process}/status').read_text().splitlines() if line.startswith('SigCgt:'))
        return bool(int(row.split()[1], 16) & (1 << (number - 1)))
    except (OSError, StopIteration):
        return False


def capture(cell, root, authentication=None, descriptors=(), pool_reader=None):
    def limits():
        harness.child_limits(6)
        for number in SIGNALS: signal.signal(number, signal.SIG_IGN if cell.ignored and number == cell.number else signal.SIG_DFL)
    command = [str(root / 'bin/make'), '-rR', '--no-print-directory',
               '-j' + str(cell.jobs), '--jobserver-style=' + cell.style]
    if cell.mode: command.append('-' + cell.mode)
    if cell.kind == 'load': command.append('-l0')
    environment = harness.clean_environment(root)
    if authentication is not None:
        command.remove('-j' + str(cell.jobs))
        environment['MAKEFLAGS'] = 'rR -j --jobserver-auth=' + authentication + ' --no-print-directory'
    process = subprocess.Popen(command, cwd=root / 'work', env=environment,
                               stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               start_new_session=True, close_fds=True, pass_fds=descriptors, preexec_fn=limits)
    selector = selectors.DefaultSelector()
    buffers = {'stdout': bytearray(), 'stderr': bytearray()}
    for name, stream in (('stdout', process.stdout), ('stderr', process.stderr)):
        os.set_blocking(stream.fileno(), False)
        selector.register(stream, selectors.EVENT_READ, name)
    deadline = time.monotonic() + 6
    limit = None
    leftovers = False
    sent = released = False
    second_sent = False
    pair_children = {}
    pair_released_a = pair_released_b = False
    events = []
    work = root / 'work'
    cleanup_deadline = None
    last_count = 0.0
    try:
        while selector.get_map() or process.poll() is None:
            now = time.monotonic()
            if limit is None and now >= deadline: limit = 'timeout'
            if now - last_count >= .1:
                last_count = now
                if harness.group_size(process.pid) > harness.MAX_CHILDREN: limit = 'child-count'
            if cell.kind == 'pair' and not sent and all((work / (name + '.pid')).exists() for name in 'ab') and process.poll() is None:
                pair_children = {name: int((work / (name + '.pid')).read_text()) for name in 'ab'}
                for name in 'ab': (work / (name + '.pid')).unlink()
                (work / 'ready').write_bytes(b'ready\n')
                events += ['ready', 'sent']
                if cell.delivery == 'group': os.killpg(process.pid, cell.number)
                else: os.kill(pair_children['a'] if cell.delivery == 'child' else process.pid, cell.number)
                sent = True
            if cell.kind == 'pair' and sent:
                if not pair_released_a:
                    ack = True
                    if cell.delivery == 'group' or (cell.delivery == 'parent' and cell.number == 15):
                        ack &= all((work / (name + '.signal')).exists() for name in 'ab')
                    elif cell.delivery == 'child': ack &= (work / 'a.signal').exists()
                    if cell.delivery != 'child':
                        ack &= not caught(process.pid, cell.number)
                        if cell.number in (1, 2, 3, 15):
                            ack &= all(not (work / name).exists() for name in 'ab')
                        else: ack &= b'Waiting for unfinished jobs....\n' in buffers['stderr']
                    if ack:
                        (work / 'a.release').write_bytes(b'release\n')
                        events += ['acknowledged', 'release-a']
                        pair_released_a = True
                if pair_released_a and not pair_released_b and not Path('/proc', str(pair_children['a'])).exists():
                    if cell.delivery != 'child' or b'Waiting for unfinished jobs....\n' in buffers['stderr']:
                        (work / 'b.release').write_bytes(b'release\n')
                        events.append('release-b')
                        pair_released_b = True
            load_ready = cell.kind not in ('load', 'token-wait') or ((work / ('queued-c' if cell.kind == 'load' else 'queued-b')).exists() and (pool_reader is None or available(pool_reader) == 0))
            if cell.kind != 'pair' and not sent and (work / 'ready.pid').exists() and process.poll() is None and load_ready:
                child = int((work / 'ready.pid').read_text())
                (work / 'ready.pid').unlink()
                (work / 'ready').write_bytes(b'ready\n')
                events.append('ready')
                if cell.delivery == 'group': os.killpg(process.pid, cell.number)
                else: os.kill(child if cell.delivery == 'child' else process.pid, cell.number)
                sent = True
                events.append('sent')
            if cell.kind != 'pair' and sent and not released and cell.delivery == 'parent' and cell.number != 15:
                acknowledged = cell.ignored or not caught(process.pid, cell.number)
                if cell.number in (1, 2, 3) and cell.deletes:
                    acknowledged &= not os.path.lexists(work / 'out')
                elif cell.number not in (1, 2, 3) and not cell.ignored:
                    acknowledged &= b'Waiting for unfinished jobs....\n' in buffers['stderr']
                if acknowledged and not second_sent:
                    events.append('acknowledged')
                    if cell.second:
                        os.kill(process.pid, cell.second)
                        events.append('second-sent')
                        second_sent = True
                    else:
                        (work / 'release').write_bytes(b'release\n')
                        released = True
            if second_sent and not released and not caught(process.pid, cell.second) and (cell.second != 15 or ((work / 'received.signal').exists() and not os.path.lexists(work / 'out'))):
                (work / 'release').write_bytes(b'release\n')
                released = True
            if cell.kind != 'pair' and sent and not released and not cell.second and not cell.ignored and (cell.delivery in ('group', 'child') or cell.number == 15):
                ack = (work / 'received.signal').exists()
                if cell.delivery != 'child':
                    ack &= not caught(process.pid, cell.number)
                    if cell.number in (1, 2, 3, 15) and cell.deletes:
                        ack &= not os.path.lexists(work / 'out')
                    elif cell.number not in (1, 2, 3, 15):
                        ack &= b'Waiting for unfinished jobs....\n' in buffers['stderr']
                if ack:
                    events.append('acknowledged')
                    (work / 'release').write_bytes(b'release\n')
                    released = True
            # An inherited ignored TERM must also be released; it is not forwarded.
            if sent and cell.ignored and not released:
                (work / 'release').write_bytes(b'release\n')
                events.append('acknowledged')
                released = True
            if process.poll() is not None and cleanup_deadline is None:
                leftovers = harness.process_group_exists(process.pid)
                harness.kill_group(process.pid)
                cleanup_deadline = now + .5
            if limit is not None and cleanup_deadline is None:
                harness.kill_group(process.pid)
                cleanup_deadline = now + .5
            if cleanup_deadline is not None and now >= cleanup_deadline: break
            for key, _ in selector.select(.005):
                chunk = os.read(key.fd, 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    key.fileobj.close()
                    continue
                room = harness.MAX_OUTPUT - sum(map(len, buffers.values()))
                buffers[key.data].extend(chunk[:max(0, room)])
                if len(chunk) > room: limit = 'output'
        harness.kill_group(process.pid)
        status = process.wait(timeout=1)
    finally:
        harness.kill_group(process.pid)
        for key in list(selector.get_map().values()): key.fileobj.close()
        selector.close()
        if process.poll() is None: process.wait(timeout=1)
    files = harness.snapshot({name: root / name for name in ('work', 'home', 'tmp')})
    observation = harness.Observation(bytes(buffers['stdout']), bytes(buffers['stderr']), status, limit, files, leftovers)
    return observation, events


def anchors(cell, observation, events, root):
    errors = []
    if (observation.stdout, observation.stderr, observation.returncode) != cell.expected:
        errors.append('independent exact stream/status anchors')
    if observation.limit or observation.leftover_processes: errors.append('process containment')
    if events[:2] != ['ready', 'sent']: errors.append('ready before controlled delivery')
    if cell.delivery == 'parent' and cell.number != 15 and 'acknowledged' not in events:
        errors.append('shutdown acknowledged before releasing child')
    work = root / 'work'
    if cell.kind == 'pair':
        if (work / 'a').exists() or (work / 'a.tail').exists(): errors.append('failed/cancelled owner cleanup')
        if (work / 'b').exists() != (cell.delivery == 'child'): errors.append('parallel peer cleanup')
        if (work / 'b.tail').exists() != (cell.delivery == 'child'): errors.append('ordinary vs fatal peer continuation')
        if events[-2:] != ['release-a', 'release-b']: errors.append('controlled child reap order')
        if not (work / 'protected').is_file() or (work / 'protected').read_bytes() != PROTECTED: errors.append('protected file')
        if any((root / 'tmp').iterdir()): errors.append('temporary resources')
        return errors
    if cell.kind in ('load', 'token-wait') and any((work / name).exists() for name in 'bc'): errors.append('load-waiter cancellation')
    if cell.kind != 'archive' and os.path.lexists(work / 'out') == cell.deletes: errors.append('target deletion policy')
    tail_expected = cell.ignored or (cell.delivery == 'child' and cell.kind == 'ignore-recipe')
    if (work / 'tail').exists() != tail_expected: errors.append('continuation cancellation')
    if cell.kind == 'archive' and (not (work / 'lib.a').is_file() or (work / 'lib.a').read_bytes()[24:36] != b'1700000001  '): errors.append('archive preservation and timestamp')
    if not (work / 'protected').is_file() or (work / 'protected').read_bytes() != PROTECTED: errors.append('protected file')
    if cell.kind in ('unchanged', 'same-mtime') and (not (work / 'out').is_file() or (work / 'out').stat().st_mtime_ns != 1700000000000000000): errors.append('fixed target timestamp')
    if cell.kind == 'intermediate' and (work / 'mid').exists(): errors.append('intermediate cleanup')
    if any((root / 'tmp').iterdir()): errors.append('temporary resources')
    return errors


def execute(cell, root, fixture, executable, authentication=None, descriptors=(), pool_reader=None):
    case = harness.Case(fixture, cell.id, 'runtime.signals', 'required', (), 6, {}, {})
    harness.reset_workspace(case, root, executable)
    observation, events = capture(cell, root, authentication, descriptors, pool_reader)
    return observation, events, anchors(cell, observation, events, root)


def available(descriptor):
    count = array.array('i', [0])
    fcntl.ioctl(descriptor, 0x541B, count, True)  # Linux FIONREAD, no token consumed.
    return count[0]


class ExternalPool:
    """Test-owned shared descriptors. Peeking never changes client FD flags."""
    def __init__(self, root, style, tokens=b'+*'):
        self.path = None
        self.descriptors = ()
        self.handles = []
        self.identity = None
        self.tokens = tokens
        if style == 'fifo':
            self.path = root / 'borrowed.fifo'
            os.mkfifo(self.path, 0o600)
            self.reader = self.writer = os.open(self.path, os.O_RDWR | os.O_NONBLOCK | os.O_CLOEXEC)
            self.handles.append(self.reader)
            metadata = self.path.stat()
            self.identity = metadata.st_dev, metadata.st_ino
            self.authentication = 'fifo:' + str(self.path)
        else:
            reader, self.writer = os.pipe2(os.O_CLOEXEC)
            self.handles += [reader, self.writer]
            self.reader = os.open(f'/proc/self/fd/{reader}', os.O_RDONLY | os.O_NONBLOCK | os.O_CLOEXEC)
            self.handles.append(self.reader)
            self.descriptors = reader, self.writer
            self.authentication = f'{reader},{self.writer}'
        self.refill()

    def refill(self):
        os.write(self.writer, self.tokens)

    def returned(self):
        try: return os.read(self.reader, 64)
        except BlockingIOError: return b''

    def intact(self):
        if self.path is None: return True
        try:
            metadata = self.path.stat()
            return self.identity == (metadata.st_dev, metadata.st_ino)
        except OSError: return False

    def close(self):
        for descriptor in self.handles: os.close(descriptor)
        if self.path is not None and self.intact(): self.path.unlink(missing_ok=True)


def borrowed_probe(cell, root, fixture, reference, candidate, self_test):
    pool = ExternalPool(root, cell.style, b'' if cell.kind == 'token-wait' else b'+*')
    try:
        expected, expected_events, errors = execute(cell, root, fixture, reference, pool.authentication, pool.descriptors, pool.reader)
        reference_tokens = pool.returned()
        if reference_tokens != (b'*' if cell.kind == 'pair' else b''): errors.append('GNU 4.4.1 fatal borrowed-token loss control changed')
        if not pool.intact(): errors.append('reference removed borrowed resource')
        pool.refill()
        observed, events, observed_errors = execute(cell, root, fixture, candidate, pool.authentication, pool.descriptors, pool.reader)
        returned = pool.returned()
        required_tokens = (b'*' if cell.kind == 'pair' else b'') if self_test else pool.tokens
        if sorted(returned) != sorted(required_tokens): errors.append('borrowed token safety recovery')
        if not pool.intact(): errors.append('candidate removed borrowed resource')
        errors += observed_errors + harness.differences(expected, observed)
        return dict(case='borrowed-' + cell.id, errors=errors, safety_recovery=True,
                    oracle=dataclasses.asdict(expected), candidate=dataclasses.asdict(observed),
                    oracle_events=expected_events, candidate_events=events,
                    oracle_tokens=reference_tokens, candidate_tokens=returned,
                    token_contract='GNU fatal-token loss is recorded; rmake must return every original token byte.')
    finally:
        pool.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=oracle.PREFIX / 'bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--self-test', action='store_true')
    parser.add_argument('--case', action='append', default=[])
    parser.add_argument('--repeat', type=int, default=3)
    parser.add_argument('--unstaged', action='store_true', help='retain original simultaneous group-exit research; requires one base group case')
    parser.add_argument('--artifacts', type=Path, default=ROOT / 'artifacts/signals')
    args = parser.parse_args()
    if bool(args.candidate) == args.self_test: parser.error('choose candidate or self-test')
    if not 1 <= args.repeat <= 100: parser.error('repeat must be 1-100')
    cells = inventory()
    if set(args.case) - {c.id for c in cells}: parser.error('unknown case')
    if args.unstaged and (len(args.case) != 1 or not any(c.id == args.case[0] and c.delivery == 'group' and c.kind == 'changed' and not c.mode and not c.ignored and not c.second for c in cells)):
        parser.error('unstaged research requires one base group-delivery case')
    profile = oracle.verify(args.oracle.resolve())
    candidate = args.oracle.resolve() if args.self_test else args.candidate.resolve()
    identity = harness.executable_identity(candidate)
    args.artifacts.mkdir(parents=True, exist_ok=True)
    destination = Path(tempfile.mkdtemp(prefix='signals-', dir=args.artifacts.resolve()))
    results = []
    for cell in cells:
        if args.case and cell.id not in args.case: continue
        if args.unstaged: cell = dataclasses.replace(cell, unstaged=True)
        fixture = destination / ('input-' + cell.id)
        materialize(cell, fixture)
        for repeat in range(args.repeat):
            root = destination / 'run'
            root.mkdir(exist_ok=True)
            expected, expected_events, expected_errors = execute(cell, root, fixture, args.oracle.resolve())
            observed, events, errors = execute(cell, root, fixture, candidate)
            errors += expected_errors + harness.differences(expected, observed)
            row = dict(case=cell.id, repeat=repeat, errors=errors,
                       oracle=dataclasses.asdict(expected), candidate=dataclasses.asdict(observed),
                       oracle_events=expected_events, candidate_events=events)
            results.append(row)
            print('FAIL' if errors else 'PASS', cell.id, repeat, errors, flush=True)
    if not args.case:
        for number in (signal.SIGINT, signal.SIGTERM, signal.SIGPIPE):
            for delivery in ('parent', 'group'):
                for style in ('fifo', 'pipe'):
                    for kind in ('pair', 'load', 'token-wait'):
                        cell = Cell(number, delivery, 4 if kind == 'load' else 2, style, kind)
                        fixture = destination / ('borrowed-input-' + cell.id)
                        materialize(cell, fixture)
                        for repeat in range(args.repeat):
                            row = borrowed_probe(cell, destination / 'run', fixture, args.oracle.resolve(), candidate, args.self_test)
                            row['repeat'] = repeat
                            results.append(row)
                            print('FAIL' if row['errors'] else 'PASS', row['case'], repeat, row['errors'], flush=True)
    report = dict(oracle_profile=profile, candidate_identity=identity, unstaged_research=args.unstaged,
                  runner_sha256=harness.sha256_file(Path(__file__)),
                  helper_sha256=__import__('hashlib').sha256(HELPER + PAIR_HELPER).hexdigest(), results=results,
                  limitations=['Trusted fixtures and Linux procfs, not a security sandbox.',
                               'Signal dispositions and ready/deletion/output acknowledgments control delivery; no stream normalization.'])
    (destination / 'report.json').write_text(json.dumps(report, default=lambda x: {'hex': x.hex()} if isinstance(x, bytes) else str(x), indent=2) + '\n')
    print('Report:', destination / 'report.json')
    return int(any(row['errors'] for row in results))

if __name__ == '__main__':
    raise SystemExit(main())
