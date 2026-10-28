#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original controlled interruption of expansion, capture and blocking effects."""
from __future__ import annotations
import argparse
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
          'main-read', 'stdin-read', 'expansion')

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

    # GNU's stdin-Makefile temporary is observer state outside the target tree.
    # Preserve its names as evidence; the candidate must release owned temps.
    if cell.phase == 'stdin-read':
        extra = root/'stdin-temporaries';extra.mkdir(exist_ok=True)
        for path in extra.iterdir():path.unlink()
        environment['TMPDIR'] = str(extra)
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
        if not errors: os.kill(process.pid, cell.number)
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
    if require_reaping and temporary_names: errors.append('candidate leaked owned stdin temporary')
    return observed, errors, before_cleanup, temporary_names


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
            baseline,base_errors,base_live,base_temporaries=execute(cell,root,fixture,reference,False)
            observed,errors,live,temporaries=execute(cell,root,fixture,candidate,not args.self_test)
            errors+=base_errors+harness.differences(baseline,observed)
            rows.append({'case':cell.id,'repeat':repeat,'errors':errors,'oracle_live_after_exit':base_live,
                         'candidate_live_after_exit':live,'oracle_stdin_temporaries':base_temporaries,'candidate_stdin_temporaries':temporaries,'oracle':dataclasses.asdict(baseline),'candidate':dataclasses.asdict(observed)})
            print('FAIL' if errors else 'PASS',cell.id,repeat,errors,flush=True)
    report={'oracle_profile':profile,'candidate_identity':identity,'runner_sha256':harness.sha256_file(Path(__file__)),
            'results':rows,'safety':'GNU may orphan an unregistered shell-function child. The controller records and terminates that exact pidfd. The candidate must reap its owned capture child before exit. Streams, status and filesystem remain exact.'}
    (destination/'report.json').write_text(json.dumps(report,default=lambda x:{'hex':x.hex()} if isinstance(x,bytes) else str(x),indent=2)+'\n')
    print('Report:',destination/'report.json')
    return int(any(row['errors'] for row in rows))

if __name__=='__main__':raise SystemExit(main())
