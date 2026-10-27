#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original inherited-PTY metadata, stdin and output-diversion matrix."""
from __future__ import annotations
import argparse
import dataclasses
import json
import os
from pathlib import Path
import pty
import select
import subprocess
import tempfile
import time
import conformance as harness
import oracle

ROOT = Path(__file__).resolve().parents[1]
HELPER = b'''import os
for fd,name in [(0,'stdin'),(1,'stdout'),(2,'stderr')]:print(name+'-tty='+str(int(os.isatty(fd))),flush=True)
for name in ['MAKE_TERMOUT','MAKE_TERMERR']:print(name+'=['+os.environ.get(name,'unset')+']',flush=True)
os.write(2,b'child-stderr\\n')
'''

@dataclasses.dataclass(frozen=True)
class Cell:
    mode: str
    jobs: int
    forced: bool
    merged: bool
    imported: bool

    @property
    def id(self): return f'{self.mode}-j{self.jobs}-' + ('forced' if self.forced else 'ordinary') + ('-merged' if self.merged else '-separate') + ('-imported' if self.imported else '-default')

    @property
    def source(self):
        return ("$(info termout-origin=$(origin MAKE_TERMOUT))\n$(info termerr-origin=$(origin MAKE_TERMERR))\n.PHONY: all\nall:;" + ('+@' if self.forced else '@') + 'python3 helper.py\n').encode()

    def expected(self, terminal):
        synchronized = self.mode == 'recurse' or (self.mode in ('line','target') and self.jobs != 1 and not self.forced)
        origin_out = 'environment' if self.imported else 'default'
        origin_err = 'environment' if self.imported else 'default' if self.merged else 'undefined'
        out = f'termout-origin={origin_out}\ntermerr-origin={origin_err}\nstdin-tty=1\nstdout-tty={int(not synchronized)}\nstderr-tty={int(self.merged and not synchronized)}\nMAKE_TERMOUT=[{terminal if not self.imported else ""}]\nMAKE_TERMERR=[{("" if self.imported else terminal) if self.merged or self.imported else "unset"}]\n'.encode()
        if self.merged: out += b'child-stderr\n'
        return out.replace(b'\n',b'\r\n'), b'' if self.merged else b'child-stderr\n'


def inventory():
    return tuple(Cell(mode,jobs,forced,merged,imported) for mode in ('none','line','target','recurse')
                 for jobs in (1,2) for forced in (False,True) for merged in (False,True) for imported in (False,True))


def execute(cell,root,fixture,binary,master,slave):
    case=harness.Case(fixture,cell.id,'runtime.stdin','required',(),5,{}, {})
    harness.reset_workspace(case,root,binary)
    environment=harness.clean_environment(root,{'MAKE_TERMOUT':'','MAKE_TERMERR':''} if cell.imported else {})
    args=['-rR','--no-print-directory','-j'+str(cell.jobs),'--jobserver-style=pipe','-O'+cell.mode]
    process=subprocess.Popen([str(root/'bin/make'),*args],cwd=root/'work',env=environment,stdin=slave,stdout=slave,
        stderr=slave if cell.merged else subprocess.PIPE,start_new_session=True,preexec_fn=lambda:harness.child_limits(5))
    streams={master:bytearray()}
    if process.stderr is not None:streams[process.stderr.fileno()]=bytearray()
    active=set(streams);limit=None;deadline=time.monotonic()+4
    try:
        while True:
            if time.monotonic()>deadline:
                limit='timeout';harness.kill_group(process.pid);break
            ready,_,_=select.select(list(active),[],[],.01)
            for descriptor in ready:
                try:chunk=os.read(descriptor,8192)
                except BlockingIOError:continue
                if not chunk:active.remove(descriptor)
                else:streams[descriptor].extend(chunk)
            if process.poll() is not None and not ready:break
        process.wait(timeout=1)
    finally:
        harness.kill_group(process.pid)
    err=bytes(streams[process.stderr.fileno()]) if process.stderr is not None else b''
    if process.stderr is not None:process.stderr.close()
    observed=harness.Observation(bytes(streams[master]),err,process.returncode,limit,
        harness.snapshot({n:root/n for n in ('work','home','tmp')}),False)
    errors=[]
    if (observed.stdout,observed.stderr)!=cell.expected(os.ttyname(slave)):errors.append('terminal metadata and diversion anchor')
    if observed.returncode!=0 or observed.limit:errors.append('terminal process completion')
    return observed,errors


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle',type=Path,default=oracle.PREFIX/'bin/make')
    parser.add_argument('--candidate',type=Path)
    parser.add_argument('--self-test',action='store_true')
    parser.add_argument('--repeat',type=int,default=3)
    parser.add_argument('--case',action='append',default=[])
    parser.add_argument('--artifacts',type=Path,default=ROOT/'artifacts/terminals')
    args=parser.parse_args()
    if bool(args.candidate)==args.self_test:parser.error('choose candidate or self-test')
    if not 1<=args.repeat<=100:parser.error('repeat must be 1-100')
    reference=args.oracle.resolve();profile=oracle.verify(reference)
    candidate=reference if args.self_test else args.candidate.resolve();identity=harness.executable_identity(candidate)
    cells=inventory()
    if set(args.case)-{c.id for c in cells}:parser.error('unknown case')
    args.artifacts.mkdir(parents=True,exist_ok=True)
    destination=Path(tempfile.mkdtemp(prefix='pty-',dir=args.artifacts.resolve()));rows=[]
    for cell in cells:
        if args.case and cell.id not in args.case:continue
        fixture=destination/('input-'+cell.id);fixture.mkdir()
        (fixture/'Makefile').write_bytes(cell.source);(fixture/'helper.py').write_bytes(HELPER)
        for repeat in range(args.repeat):
            root=destination/'run';root.mkdir(exist_ok=True)
            master,slave=pty.openpty();os.set_blocking(master,False)
            try:
                baseline,base_errors=execute(cell,root,fixture,reference,master,slave)
                observed,errors=execute(cell,root,fixture,candidate,master,slave)
            finally:os.close(master);os.close(slave)
            errors+=base_errors+harness.differences(baseline,observed)
            rows.append({'case':cell.id,'repeat':repeat,'errors':errors,'oracle':dataclasses.asdict(baseline),'candidate':dataclasses.asdict(observed)})
            print('FAIL' if errors else 'PASS',cell.id,repeat,errors,flush=True)
    report={'oracle_profile':profile,'candidate_identity':identity,'runner_sha256':harness.sha256_file(Path(__file__)),
            'results':rows,'limitations':['Inherited Linux PTYs without an interactive controlling-terminal job-control session. Raw CRLF and tty spellings are compared exactly using the same PTY for both engines.']}
    (destination/'report.json').write_text(json.dumps(report,default=lambda x:{'hex':x.hex()} if isinstance(x,bytes) else str(x),indent=2)+'\n')
    print('Report:',destination/'report.json')
    return int(any(row['errors'] for row in rows))

if __name__=='__main__':raise SystemExit(main())
