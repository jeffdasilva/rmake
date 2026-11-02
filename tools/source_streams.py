#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original bounded live-source read/close/interruption profiles, Linux/glibc."""
from __future__ import annotations
import argparse
import dataclasses
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import conformance as h
import oracle
ROOT=Path(__file__).resolve().parents[1]
SITES=('main','include','eval','call','deep','preload')
FAULTS=('normal','open-eintr','read-eio','read-eintr','close-eio','signal')
BODY=b'$(file >before.marker,yes)\n$(info BEFORE)\n$(file >after.marker,yes)\n$(info AFTER)\n'
@dataclasses.dataclass(frozen=True)
class Cell:
    site:str
    fault:str
    @property
    def id(self):return f'source-{self.site}-{self.fault}'
    @property
    def failed(self):return self.fault in ('read-eio','read-eintr','signal')
    @property
    def target(self):return 'Makefile' if self.site=='main' else 'source-probe.mk'
    @property
    def files(self):
        main=(BODY+b'.PHONY: all\nall:;\n') if self.site=='main' else (
            b'.PHONY: all\n$(info ROOT)\n'+
            {'include':b'include source-probe.mk\n','eval':b'$(eval include source-probe.mk)\n','call':b'$(call eval,include source-probe.mk)\n','deep':b'$(R0)\n','preload':b''}[self.site]+
            b'$(info TAIL)\nall:;\n')
        if self.site=='deep':
            definitions=b''.join(f'R{i}=$(eval $$(R{i+1}))\n'.encode() for i in range(512))+b'R512=$(eval include source-probe.mk)\n'
            main=definitions+main
        files={'Makefile':main,'anchor':b'Unmodified source profile anchor.\n'}
        if self.site!='main':files['source-probe.mk']=BODY
        return files
    @property
    def expected(self):
        stdout=b'BEFORE\n' if self.failed else b'BEFORE\nAFTER\n'
        if self.site in ('include','eval','call','deep'):stdout=b'ROOT\n'+stdout+(b'' if self.failed else b'TAIL\n')
        if self.site=='preload' and not self.failed:stdout+=b'ROOT\nTAIL\n'
        stderr=b''
        if self.fault in ('read-eio','read-eintr'):
            message=b'Input/output error' if self.fault=='read-eio' else b'Interrupted system call'
            stderr=b'make: *** '+self.target.encode()+b': '+message+b'.  Stop.\n'
        return {'exit_code':-15 if self.fault=='signal' else 2 if self.failed else 0,'stdout_hex':stdout.hex(),'stderr_hex':stderr.hex()}
    @property
    def args(self):return ('-rR','--no-print-directory','-s')
    def environment(self,library):
        env={'LD_PRELOAD':str(library),'RMAKE_SOURCE_FAULT':self.fault,'RMAKE_SOURCE_TARGET':self.target}
        if self.site=='preload':env['MAKEFILES']='source-probe.mk'
        return env

def inventory():return tuple(Cell(s,f) for s in SITES for f in FAULTS)
def validate(cell,obs):
    errors=[]
    fs=obs.filesystem
    if 'work/before.marker' not in fs:errors.append('prior source effect missing')
    if ('work/after.marker' in fs)==cell.failed:errors.append('later source effect ordering')
    import base64
    if base64.b64decode(fs.get('work/anchor',{}).get('content',''))!=cell.files['anchor']:errors.append('anchor changed')
    expected=b'' if cell.fault=='normal' else (cell.fault+'\n').encode()
    actual=base64.b64decode(fs.get('work/source.trace',{}).get('content',''))
    if actual!=expected:errors.append('unqualified source fault trace')
    return errors

def build(directory):
    p=directory/'source-streams.so'
    subprocess.run(['cc','-shared','-fPIC','-O2','-Wall','-Wextra','-Werror',str(ROOT/'tools/source_streams.c'),'-ldl','-o',str(p)],check=True,timeout=30)
    return p

def run(reference,candidate,artifacts,case=None):
    oracle.verify(reference)
    cells=[c for c in inventory() if case is None or c.id==case]
    if not cells:raise ValueError('unknown source profile')
    artifacts.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='rmake-source-profile-',dir=artifacts) as tmp:
        directory=Path(tmp);library=build(directory);records=[];fail=0
        (directory/'observations').mkdir()
        for cell in cells:
            path=directory/cell.id;path.mkdir()
            for name,data in cell.files.items():(path/name).write_bytes(data)
            model=h.Case(path,cell.id,'includes.live-source-io','required',cell.args,5,cell.environment(library),cell.expected)
            obs=[h.execute(model,directory/'observations',binary) for binary in (reference,reference,candidate)]
            errors=[h.expectation_errors(model,o)+validate(cell,o) for o in obs]
            repeat=h.differences(obs[0],obs[1]);diff=h.differences(obs[0],obs[2]);bounds=[bool(o.limit or o.leftover_processes) for o in obs]
            bad=any(errors) or repeat or diff or any(bounds);fail+=bool(bad)
            print(('FAIL' if bad else 'PASS'),cell.id,errors,repeat,diff,bounds,flush=True)
            records.append(dict(id=cell.id,site=cell.site,fault=cell.fault,inputs={n:d.hex() for n,d in cell.files.items()},args=cell.args,expected=cell.expected,errors=errors,repeat=repeat,differences=diff,bounds=bounds,observations=[dict(h.observation_json(o),stdout_hex=o.stdout.hex(),stderr_hex=o.stderr.hex()) for o in obs]))
        report={'classification':'Separately gated original Linux/glibc source I/O profiles; no extra ordinary-case or safe-exception credit.','identities':{str(b):hashlib.sha256(b.read_bytes()).hexdigest() for b in (reference,candidate)},'observer_sha256':hashlib.sha256((ROOT/'tools/source_streams.c').read_bytes()).hexdigest(),'records':records,'failures':fail}
        destination=artifacts/('report-'+candidate.name+'.json');destination.write_text(json.dumps(report,indent=2)+'\n');print('Source profiles:',len(cells),'failures:',fail,'report:',destination,flush=True)
        return int(fail!=0)

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--oracle',type=Path,required=True);p.add_argument('--candidate',type=Path);p.add_argument('--self-test',action='store_true');p.add_argument('--case');p.add_argument('--artifacts',type=Path,default=ROOT/'artifacts/source-streams');a=p.parse_args()
    if not a.self_test and a.candidate is None:p.error('--candidate or --self-test required')
    return run(a.oracle.resolve(),a.oracle.resolve() if a.self_test else a.candidate.resolve(),a.artifacts.resolve(),a.case)
if __name__=='__main__':raise SystemExit(main())
