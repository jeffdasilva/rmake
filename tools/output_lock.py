#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Controlled POSIX output-lock contention, release and fatal completion owners."""
from __future__ import annotations
import argparse
import dataclasses
import fcntl
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
import conformance as harness
import oracle

ROOT = Path(__file__).resolve().parents[1]
SIGNALS = (1, 2, 3, 13, 15, 24, 25)
PROTECTED = b'Original externally owned mutex bytes.\n'
WAIT = b'make: *** wait: No child processes.  Stop.\n'
NOTICE = b'make: *** Waiting for unfinished jobs....\n'

@dataclasses.dataclass(frozen=True)
class Cell:
    mode: str
    number: int = 0
    phase: str = 'recipe'
    changed: bool = False
    jobs: int = 2

    @property
    def id(self):
        return f'{self.phase}-{self.mode}-j{self.jobs}-' + (signal.Signals(self.number).name.lower() if self.number else 'release') + ('-changed' if self.changed else '-phony')

    @property
    def source(self):
        if self.phase == 'reader':
            return b'$(info locked-output)\n$(file >ready)\n.PHONY: all\nall:;@true\n'
        return (('' if self.changed else '.PHONY: all\n') + "all:;@printf 'locked-output\\n'; touch ready" + (' all' if self.changed else '') + '\n').encode()

    def expected(self):
        if not self.number: return b'locked-output\n', b'', 0
        if self.phase == 'reader': return b'', b'', 1 if self.number == 3 else -self.number
        deleting = self.number in (1, 2, 3, 15)
        err = WAIT + NOTICE + WAIT if deleting else NOTICE + WAIT + WAIT
        if deleting and self.changed: err = b"make: *** Deleting file 'all'\n" + err
        return b'', err, 2


def inventory():
    cells = []
    for mode, jobs in [('line', 2), ('target', 2), ('recurse', 2), ('recurse', 1)]:
        cells.append(Cell(mode, jobs=jobs))
        cells += [Cell(mode, number, changed=changed, jobs=jobs) for number in SIGNALS for changed in (False, True)]
    for mode in ('line', 'target'):
        cells.append(Cell(mode, phase='reader'))
        cells += [Cell(mode, number, phase='reader') for number in SIGNALS]
    return tuple(cells)


def anchors(cell, observed):
    errors = []
    if (observed.stdout, observed.stderr, observed.returncode) != cell.expected(): errors.append('independent lock/owner/exit anchor')
    if observed.limit or observed.leftover_processes: errors.append('process containment')
    present = cell.changed and (not cell.number or cell.number not in (1, 2, 3, 15))
    if ('work/all' in observed.filesystem) != present: errors.append('changed target lifecycle')
    import base64
    if observed.filesystem.get('work/mutex', {}).get('content') != base64.b64encode(PROTECTED).decode(): errors.append('borrowed mutex preservation')
    return errors


def execute(cell, root, fixture, binary):
    case = harness.Case(fixture, cell.id, 'runtime.cancellation', 'required', (), 5, {}, {})
    harness.reset_workspace(case, root, binary)
    args = ['-rR', '--no-print-directory', '-j'+str(cell.jobs), '--jobserver-style=pipe', '-O'+cell.mode, '--sync-mutex=fnm:mutex']
    errors = []; limit = None
    with (root/'work/mutex').open('r+b') as lock:
        fcntl.lockf(lock, fcntl.LOCK_EX, 1)
        process = subprocess.Popen([str(root/'bin/make'), *args], cwd=root/'work', env=harness.clean_environment(root),
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True,
            preexec_fn=lambda: harness.child_limits(5))
        try:
            deadline = time.monotonic()+3
            while True:
                try: channel = Path('/proc', str(process.pid), 'wchan').read_text()
                except OSError: channel = ''
                if (root/'work/ready').exists() and 'fcntl' in channel: break
                if process.poll() is not None or time.monotonic()>deadline:
                    errors.append('POSIX lock contention phase was not reached'); break
                time.sleep(.002)
            if not errors:
                if cell.number: os.kill(process.pid, cell.number)
                else: fcntl.lockf(lock, fcntl.LOCK_UN, 1)
            else: harness.kill_group(process.pid)
            out, err = process.communicate(timeout=2)
        except subprocess.TimeoutExpired:
            limit = 'shutdown-timeout'; harness.kill_group(process.pid)
            out, err = process.communicate(timeout=1)
        finally: harness.kill_group(process.pid)
    observation = harness.Observation(out, err, process.returncode, limit,
        harness.snapshot({name:root/name for name in ('work','home','tmp')}), False)
    return observation, errors + anchors(cell, observation)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle',type=Path,default=oracle.PREFIX/'bin/make')
    parser.add_argument('--candidate',type=Path)
    parser.add_argument('--self-test',action='store_true')
    parser.add_argument('--repeat',type=int,default=3)
    parser.add_argument('--case',action='append',default=[])
    parser.add_argument('--artifacts',type=Path,default=ROOT/'artifacts/output-lock')
    args=parser.parse_args()
    if bool(args.candidate)==args.self_test:parser.error('choose candidate or self-test')
    if not 1<=args.repeat<=100:parser.error('repeat must be 1-100')
    reference=args.oracle.resolve();profile=oracle.verify(reference)
    candidate=reference if args.self_test else args.candidate.resolve();identity=harness.executable_identity(candidate)
    cells=inventory()
    if set(args.case)-{c.id for c in cells}:parser.error('unknown case')
    args.artifacts.mkdir(parents=True,exist_ok=True)
    destination=Path(tempfile.mkdtemp(prefix='lock-',dir=args.artifacts.resolve()))
    rows=[]
    for cell in cells:
        if args.case and cell.id not in args.case:continue
        fixture=destination/('input-'+cell.id);fixture.mkdir()
        (fixture/'Makefile').write_bytes(cell.source);(fixture/'mutex').write_bytes(PROTECTED)
        for repeat in range(args.repeat):
            root=destination/'run';root.mkdir(exist_ok=True)
            baseline,base_errors=execute(cell,root,fixture,reference)
            observed,errors=execute(cell,root,fixture,candidate)
            errors+=base_errors+harness.differences(baseline,observed)
            rows.append({'case':cell.id,'repeat':repeat,'errors':errors,'oracle':dataclasses.asdict(baseline),'candidate':dataclasses.asdict(observed)})
            print('FAIL' if errors else 'PASS',cell.id,repeat,errors,flush=True)
    report={'oracle_profile':profile,'candidate_identity':identity,'runner_sha256':harness.sha256_file(Path(__file__)),
            'results':rows,'limitations':['Trusted fixtures; Linux procfs fcntl wait-channel observation. All output/status/filesystem observations exact.']}
    (destination/'report.json').write_text(json.dumps(report,default=lambda x:{'hex':x.hex()} if isinstance(x,bytes) else str(x),indent=2)+'\n')
    print('Report:',destination/'report.json')
    return int(any(row['errors'] for row in rows))

if __name__=='__main__':raise SystemExit(main())
