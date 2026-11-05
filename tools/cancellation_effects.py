#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original controlled interruption of expansion, capture and blocking effects."""
from __future__ import annotations
import argparse
import hashlib
import re
import stat
import dataclasses
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import tempfile
import time
import conformance as harness
import oracle

ROOT = Path(__file__).resolve().parents[1]
SIGNALS = (1, 2, 3, 13, 15, 24, 25)
HELPER = b'''import os
from pathlib import Path
import time
ready=Path(os.environ['RMAKE_TEST_READY'])
ready.with_suffix('.tmp').write_text(str(os.getpid()))
ready.with_suffix('.tmp').rename(ready)
while True:time.sleep(1)
'''
PHASES = ('initial-makeflags-shell', 'initial-gnumakeflags-shell', 'makeflags-shell', 'gnumakeflags-shell', 'cli-shell', 'cli-inherited-shell', 'cli-tilde-shell', 'reader-shell', 'reader-assignment', 'include-shell',
          'recipe-shell', 'export-shell', 'file-read', 'file-write', 'include-read',
          'main-read', 'stdin-read', 'expansion', 'metadata-shell', 'metadata-eval-shell')

METADATA_DIRECTORIES = {
    'metadata-shell': 'metadata-$(shell python3 helper.py)$(file >forbidden)',
    'metadata-eval-shell': 'metadata-$(eval X:=$(shell python3 helper.py))$(file >forbidden)',
}

@dataclasses.dataclass(frozen=True)
class Cell:
    phase: str
    number: int
    mode: str = 'none'

    @property
    def id(self): return f'{self.phase}-{signal.Signals(self.number).name.lower()}-{self.mode}'

    @property
    def source(self):
        before = '$(info before)\n'
        after = '\n$(file >forbidden)\n.PHONY: all\nall:;@true\n'
        if self.phase.startswith('initial-'): return (before + after).encode()
        if self.phase in METADATA_DIRECTORIES:
            # Decode literal function bytes first; evaluate only after stat has
            # selected the physical directory for .INCLUDE_DIRS construction.
            directory = METADATA_DIRECTORIES[self.phase].replace('$', '$$').replace(' ', '\\ ')
            return (before + 'MAKEFLAGS = -I' + directory + after).encode()
        body = {
            'makeflags-shell': 'MAKEFLAGS = $(shell python3 helper.py)',
            'gnumakeflags-shell': 'GNUMAKEFLAGS = $(shell python3 helper.py)',
            'cli-shell': '', 'cli-inherited-shell': '', 'cli-tilde-shell': '',
            'reader-shell': 'X := $(shell python3 helper.py)',
            'reader-assignment': 'X != python3 helper.py',
            'include-shell': 'include $(shell python3 helper.py)',
            'recipe-shell': '.PHONY: all\nall:;@$(shell python3 helper.py)$(file >forbidden)true',
            'export-shell': 'export X = $(shell python3 helper.py)\n.PHONY: all\nall:;@true',
            'file-read': 'X := $(file <fifo)',
            'file-write': '$(file >fifo,' + 'x' * 131072 + ')',
            'include-read': 'include fifo',
            'main-read': '', 'stdin-read': '',
            'expansion': 'VALUES := $(shell seq 1 100000)\n$(file >ready)\nX := $(foreach v,$(VALUES),$(words a b c d e f g)$(subst a,b,aaa))',
        }[self.phase]
        if self.phase == 'gnumakeflags-shell':
            # Replay happens after reading: the forbidden effect must be in a
            # later recipe, rather than an already-completed reader line.
            return (before + body + '\n.PHONY: all\nall:;@$(file >forbidden)true\n').encode()
        if self.phase in ('recipe-shell', 'export-shell'): return (before + body + '\n').encode()
        return (before + body + after).encode()

    @property
    def arguments(self):
        args = ['-rR', '--no-print-directory', '-j2', '--jobserver-style=pipe', '-O' + self.mode]
        if self.phase in ('cli-shell', 'cli-inherited-shell'): args += ['SHELL=/bin/sh', 'X:=$(shell python3 helper.py)', 'AFTER:=$(file >forbidden)']
        if self.phase == 'cli-tilde-shell': args += ['SHELL=/bin/sh', 'HOME=$(shell python3 helper.py)', '-I~/include', 'AFTER:=$(file >forbidden)']
        if self.phase == 'main-read': args += ['-f', 'fifo']
        if self.phase == 'stdin-read': args += ['-f', '-']
        return args


def inventory():
    return tuple(Cell(phase, number, mode) for phase in PHASES for number in SIGNALS for mode in ('none', 'target', 'recurse'))


def materialize(cell, fixture):
    fixture.mkdir()
    (fixture / 'Makefile').write_bytes(cell.source)
    (fixture / 'helper.py').write_bytes(HELPER)
    if cell.phase in METADATA_DIRECTORIES:
        (fixture / METADATA_DIRECTORIES[cell.phase]).mkdir()


def live_group(group):
    for entry in Path('/proc').iterdir():
        if not entry.name.isdigit(): continue
        try: fields = (entry / 'stat').read_text().rsplit(')', 1)[1].split()
        except (OSError, IndexError): continue
        if int(fields[2]) == group and fields[0] != 'Z': return True
    return False


def anchors(cell, observed):
    before = not cell.phase.startswith('initial-') and not cell.phase.startswith('cli-') and cell.phase not in ('main-read', 'stdin-read')
    if cell.mode == 'target' and cell.phase not in ('recipe-shell', 'export-shell'): before = False
    expected = b'before\n' if before else b''
    errors = []
    if observed.stdout != expected: errors.append('independent output phase anchor')
    if observed.returncode != (1 if cell.number == 3 else -cell.number): errors.append('fatal wait status')
    if observed.stderr: errors.append('unexpected fatal diagnostic')
    if observed.limit or observed.leftover_processes: errors.append('process containment')
    if 'work/forbidden' in observed.filesystem: errors.append('effect after interruption')
    return errors



def capture_stdin_source(directory, pid):
    """Bind the one declared source while a pidfd proves the reader is alive."""
    descriptor = os.pidfd_open(pid)
    try:
        if select.select([descriptor], [], [], 0)[0]:
            raise ValueError('stdin reader exited before source observation')
        entries = list(directory.iterdir())
        if len(entries) != 1 or re.fullmatch(r'Gm[A-Za-z0-9]{6}', entries[0].name) is None:
            raise ValueError('expected exactly one declared stdin source')
        source = entries[0]
        with os.fdopen(os.open(source, os.O_RDONLY | os.O_NOFOLLOW), 'rb') as stream:
            before = os.fstat(stream.fileno())
            content = stream.read(8193)
            after = os.fstat(stream.fileno())
        if not stat.S_ISREG(before.st_mode):
            raise ValueError('stdin source is not a regular file')
        def stable(info):
            return info.st_dev, info.st_ino, info.st_mode, info.st_size, info.st_mtime_ns
        if stable(before) != stable(after) or select.select([descriptor], [], [], 0)[0]:
            raise ValueError('stdin source changed or reader exited during observation')
        return {'path_hex': os.fsencode(source).hex(), 'device': before.st_dev, 'inode': before.st_ino,
                'mode': stat.S_IMODE(before.st_mode), 'size': before.st_size, 'content_hex': content.hex(),
                'captured_before_signal': True}
    finally:
        os.close(descriptor)


def inspect_stdin_source(directory, captured, expected_static, observation):
    actual = harness.snapshot({'owned': directory})
    expected = {'owned': {'type': 'directory', 'mode': 0o755}}
    errors = []; final = None
    if captured is None:
        errors.append('missing independently captured pre-signal stdin source')
    else:
        path = Path(os.fsdecode(bytes.fromhex(captured['path_hex'])))
        valid = path.parent == directory and re.fullmatch(r'Gm[A-Za-z0-9]{6}', path.name) is not None
        if not valid: errors.append('stdin source outside exact declared namespace')
        if (captured.get('captured_before_signal') is not True or captured['mode'] != 0o600
                or captured['size'] != 0 or captured['content_hex'] != ''):
            errors.append('independent empty0600 pre-signal stdin source')
        if valid:
            expected['owned/' + path.name] = {'type': 'file', 'mode': 0o600, 'size': 0,
                                              'sha256': hashlib.sha256(b'').hexdigest(), 'content': ''}
            try:
                info = path.lstat()
                final = {'device': info.st_dev, 'inode': info.st_ino, 'mode': stat.S_IMODE(info.st_mode),
                         'regular': stat.S_ISREG(info.st_mode)}
                if final != {'device': captured['device'], 'inode': captured['inode'], 'mode': 0o600, 'regular': True}:
                    errors.append('final stdin source physical identity changed')
            except FileNotFoundError:
                errors.append('observed unpublished stdin source did not remain')
    if actual != expected: errors.append('complete owned-stdin directory filesystem')
    if observation.filesystem != expected_static: errors.append('independent complete stdin work/home/tmp filesystem')
    return {'errors': errors, 'captured_before_signal': captured, 'final_identity': final,
            'filesystem': actual, 'expected_filesystem': expected, 'expected_static_filesystem': expected_static}


def execute(cell, root, fixture, binary, require_reaping):
    case = harness.Case(fixture, cell.id, 'runtime.cancellation', 'required', tuple(cell.arguments), 6, {}, {})
    harness.reset_workspace(case, root, binary)
    ready = root / 'capture.ready'; ready.unlink(missing_ok=True)
    environment = harness.clean_environment(root, {'RMAKE_TEST_READY': str(ready)})
    if cell.phase == 'cli-inherited-shell': environment['MAKEFLAGS'] = '-O' + cell.mode
    if cell.phase.startswith('initial-'):
        environment['SHELL'] = '/bin/sh'
        variable = 'GNUMAKEFLAGS' if cell.phase == 'initial-gnumakeflags-shell' else 'MAKEFLAGS'
        environment[variable] = '$(shell python3 helper.py)$(file >forbidden)'

    # Both engines retain the unpublished stdin source on these early signals.
    # Its single independently captured identity and entire final directory are
    # mandatory; this is not an allowance for arbitrary candidate temporaries.
    if cell.phase == 'stdin-read':
        extra = root/'stdin-temporaries';extra.mkdir(exist_ok=True)
        for path in extra.iterdir():path.unlink()
        environment['TMPDIR'] = str(extra)
    expected_static = harness.snapshot({name: root/name for name in ('work', 'home', 'tmp')}) if cell.phase == 'stdin-read' else None
    stdin_source = None
    descriptor = None
    if cell.phase in ('file-read', 'file-write', 'include-read', 'main-read'):
        os.mkfifo(root/'work/fifo', 0o600)
    if (root / 'work/fifo').exists():
        descriptor = os.open(root / 'work/fifo', os.O_RDWR | os.O_NONBLOCK)
    process = subprocess.Popen([str(root / 'bin/make'), *case.args], cwd=root/'work', env=environment,
        stdin=subprocess.PIPE if cell.phase == 'stdin-read' else subprocess.DEVNULL,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True,
        preexec_fn=lambda: harness.child_limits(6))
    captured = None
    errors = []
    limit = None
    before_cleanup = False
    try:
        deadline = time.monotonic() + 4
        while True:
            if 'shell' in cell.phase or cell.phase == 'reader-assignment':
                staged = ready.exists()
                if staged:
                    captured = os.pidfd_open(int(ready.read_text()))
            elif cell.phase == 'expansion': staged = (root/'work/ready').exists()
            else:
                try: channel = Path('/proc', str(process.pid), 'wchan').read_text()
                except OSError: channel = ''
                staged = 'pipe' in channel or 'partner' in channel
            if staged: break
            if process.poll() is not None or time.monotonic() > deadline:
                errors.append('effect phase was not reached'); break
            time.sleep(.002)
        if not errors:
            if cell.phase == 'stdin-read':
                stdin_source = capture_stdin_source(root/'stdin-temporaries', process.pid)
            os.kill(process.pid, cell.number)
        deadline = time.monotonic() + 2
        while process.poll() is None and time.monotonic() < deadline: time.sleep(.002)
        if process.poll() is None:
            limit = 'shutdown-timeout'; harness.kill_group(process.pid)
        before_cleanup = live_group(process.pid)
        if require_reaping and before_cleanup: errors.append('candidate left a live owned capture child')
        if captured is not None and not select.select([captured], [], [], 0)[0]:
            signal.pidfd_send_signal(captured, signal.SIGKILL)
        out, err = process.communicate(timeout=1)
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        errors.append('controller: ' + str(error)); limit = 'controller'
        harness.kill_group(process.pid)
        out, err = process.communicate(timeout=1)
    finally:
        harness.kill_group(process.pid)
        if captured is not None: os.close(captured)
        if descriptor is not None: os.close(descriptor)
    observed = harness.Observation(out, err, process.returncode, limit,
        harness.snapshot({name: root/name for name in ('work', 'home', 'tmp')}), False)
    errors += anchors(cell, observed)
    temporary_names = sorted(path.name for path in (root/'stdin-temporaries').iterdir()) if cell.phase == 'stdin-read' else []
    stdin_evidence = None
    if cell.phase == 'stdin-read':
        stdin_evidence = inspect_stdin_source(root/'stdin-temporaries', stdin_source, expected_static, observed)
        errors += stdin_evidence['errors']
    return observed, errors, before_cleanup, temporary_names, stdin_evidence


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle',type=Path,default=oracle.PREFIX/'bin/make')
    parser.add_argument('--candidate',type=Path)
    parser.add_argument('--self-test',action='store_true')
    parser.add_argument('--repeat',type=int,default=3)
    parser.add_argument('--case',action='append',default=[])
    parser.add_argument('--artifacts',type=Path,default=ROOT/'artifacts/cancellation')
    args=parser.parse_args()
    if bool(args.candidate)==args.self_test:parser.error('choose candidate or self-test')
    if not 1<=args.repeat<=100:parser.error('repeat must be 1-100')
    reference=args.oracle.resolve();profile=oracle.verify(reference)
    candidate=reference if args.self_test else args.candidate.resolve()
    identity=harness.executable_identity(candidate)
    cells=inventory()
    if set(args.case)-{c.id for c in cells}:parser.error('unknown case')
    args.artifacts.mkdir(parents=True,exist_ok=True)
    destination=Path(tempfile.mkdtemp(prefix='effects-',dir=args.artifacts.resolve()))
    rows=[]
    for cell in cells:
        if args.case and cell.id not in args.case:continue
        fixture=destination/('input-'+cell.id);materialize(cell,fixture)
        for repeat in range(args.repeat):
            root=destination/'run';root.mkdir(exist_ok=True)
            baseline,base_errors,base_live,base_temporaries,base_stdin=execute(cell,root,fixture,reference,False)
            observed,errors,live,temporaries,stdin_evidence=execute(cell,root,fixture,candidate,not args.self_test)
            errors+=base_errors+harness.differences(baseline,observed)
            rows.append({'case':cell.id,'repeat':repeat,'errors':errors,'oracle_live_after_exit':base_live,
                         'candidate_live_after_exit':live,'oracle_stdin_temporaries':base_temporaries,'candidate_stdin_temporaries':temporaries,'oracle_stdin_source':base_stdin,'candidate_stdin_source':stdin_evidence,'oracle':dataclasses.asdict(baseline),'candidate':dataclasses.asdict(observed)})
            print('FAIL' if errors else 'PASS',cell.id,repeat,errors,flush=True)
    report={'oracle_profile':profile,'candidate_identity':identity,'runner_sha256':harness.sha256_file(Path(__file__)),
            'results':rows,'stdin_ownership':'The21 early-copy stdin cells require one live pre-signal host-observed empty0600 source and identical final physical identity/full bytes, with every other observer/fixture file fixed. Original memory-backed no-candidate-temporary scope is retained as historical evidence; no new exception credit.',
            'safety':'GNU may orphan an unregistered shell-function child. The controller records and terminates that exact pidfd. The candidate must reap its owned capture child before exit. Streams, status and filesystem remain exact.'}
    (destination/'report.json').write_text(json.dumps(report,default=lambda x:{'hex':x.hex()} if isinstance(x,bytes) else str(x),indent=2)+'\n')
    print('Report:',destination/'report.json')
    return int(any(row['errors'] for row in rows))

if __name__=='__main__':raise SystemExit(main())
