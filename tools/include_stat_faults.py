#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare original include-directory interruption models on Linux/glibc."""
from __future__ import annotations
import argparse
import dataclasses
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import conformance as harness
import oracle

ROOT=Path(__file__).resolve().parents[1]
PHASES=('startup','reader','recipe')
MODES=('normal','eintr-1','eintr-3','signal')
TRACE=re.compile(r'^(eintr|real) (stat|stat64|statx) ([1-9][0-9]*)$')

@dataclasses.dataclass(frozen=True)
class Cell:
    phase: str
    mode: str
    repeated: bool = False

    @property
    def id(self):return f'include-stat-{self.phase}-{self.mode}'+('-repeated' if self.repeated else '')
    @property
    def interrupts(self):return 3 if self.mode=='eintr-3' else int(self.mode!='normal')
    @property
    def operands(self):return ('-I./inc','-I./inc') if self.repeated else ('-Iinc',)
    @property
    def arguments(self):return ('-rR','--no-print-directory','-I-',*(self.operands if self.phase=='startup' else ()))
    @property
    def source(self):
        return self.single_source.replace(b'-Iinc', b'-I./inc -I./inc') if self.repeated else self.single_source
    @property
    def single_source(self):
        if self.mode=='signal':
            return {'startup':b'$(file >forbidden)\nall:;@:\n',
                    'reader':b'MAKEFLAGS += -Iinc\n$(file >forbidden)\nall:;@:\n',
                    'recipe':b'all:;@$(eval MAKEFLAGS+=-Iinc)$(file >forbidden):\n'}[self.phase]
        return {'startup':b'include input.mk\n$(info VALUE=[$(VALUE)])\nall:;@:\n',
                'reader':b'MAKEFLAGS += -Iinc\ninclude input.mk\n$(info VALUE=[$(VALUE)])\nall:;@:\n',
                'recipe':b'all:;@$(eval MAKEFLAGS+=-Iinc)$(eval include input.mk)$(info VALUE=[$(VALUE)]):\n'}[self.phase]
    @property
    def expected(self):
        return {'exit_code':-2 if self.mode=='signal' else 0,
                'stdout_hex':('' if self.mode=='signal' else b'VALUE=[ok]\n'.hex()),'stderr_hex':''}


def inventory():return tuple(Cell(phase,mode,repeated) for repeated in (False,True) for phase in PHASES for mode in MODES)


def trace_errors(cell,trace):
    lines=trace.splitlines();parsed=[TRACE.fullmatch(line) for line in lines]
    if not lines or any(match is None for match in parsed):return ['metadata fault phase was not qualified']
    kinds=[match[1] for match in parsed]
    wanted=['eintr'] if cell.mode=='signal' else ['eintr']*cell.interrupts+['real']*(2 if cell.repeated else 1)
    errors=[]
    if kinds!=wanted:errors.append('metadata retry or cancellation sequence')
    if [int(match[3]) for match in parsed]!=list(range(1,len(parsed)+1)):errors.append('metadata trace cursor')
    return errors


def anchors(cell,observed,trace):
    case=harness.Case(Path('.'),cell.id,'includes.stat-interruptions','required',cell.arguments,5,{},cell.expected,'separate')
    errors=harness.expectation_errors(case,observed)+trace_errors(cell,trace)
    if observed.limit or observed.leftover_processes:errors.append('process containment')
    if 'work/forbidden' in observed.filesystem:errors.append('effect after cancellation')
    return errors


def compile_helpers(directory):
    compiler=shutil.which('cc',path='/usr/bin:/bin')
    if compiler is None:raise harness.HarnessError('cc is required for Linux/glibc include-stat fault probes')
    library=directory/'include-stat.so';client=directory/'include-stat-abi'
    sources=[ROOT/'tools/fixtures/include_stat_fault.c',ROOT/'tools/fixtures/include_stat_abi.c']
    for source,output,extra in ((sources[0],library,['-shared','-fPIC']),(sources[1],client,[])):
        command=[compiler,'-O2','-Wall','-Wextra','-Werror',*extra,str(source),'-o',str(output),'-ldl']
        result=subprocess.run(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=30)
        if result.returncode:raise harness.HarnessError('include-stat helper compilation failed: '+result.stderr.decode(errors='replace'))
    return library,client,{'compiler':compiler,'sources':{str(p.relative_to(ROOT)):harness.sha256_file(p) for p in sources},'library_sha256':harness.sha256_file(library),'client_sha256':harness.sha256_file(client)}


def qualify_abi(client,library,directory):
    environment={'PATH':'/usr/bin:/bin','LC_ALL':'C','TZ':'UTC'}
    outputs=[]
    for extra in ({},{'LD_PRELOAD':str(library)}):
        p=subprocess.run([str(client)],cwd=directory,env=environment|extra,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=5)
        outputs.append({'returncode':p.returncode,'stdout_hex':p.stdout.hex(),'stderr_hex':p.stderr.hex()})
    if outputs[0]!=outputs[1] or outputs[0]['returncode']!=0 or outputs[0]['stderr_hex']:
        (directory/'abi-qualification-failure.json').write_text(json.dumps(outputs,indent=2)+'\n')
        raise harness.HarnessError('include-stat observer failed null-path ABI qualification')
    return outputs


def observe(cell,fixture,workspace,binary,library):
    trace=workspace/'metadata-observer.trace';trace.unlink(missing_ok=True)
    environment={'LD_PRELOAD':str(library),'RMAKE_INCLUDE_STAT_INTERRUPTS':str(cell.interrupts),'RMAKE_INCLUDE_STAT_TRACE':str(trace)}
    if cell.mode=='signal':environment['RMAKE_INCLUDE_STAT_SIGNAL']='2'
    case=harness.Case(fixture,cell.id,'includes.stat-interruptions','required',cell.arguments,5,environment,cell.expected,'separate')
    observation=harness.execute(case,workspace,binary)
    return observation,trace.read_text() if trace.exists() else ''


def campaign(cells,reference,candidate,destination,repeat):
    library,client,identity=compile_helpers(destination)
    abi=qualify_abi(client,library,destination)
    workspace=destination/'run';workspace.mkdir();records=[]
    for cell in cells:
        fixture=destination/('input-'+cell.id);fixture.mkdir();(fixture/'inc').mkdir()
        (fixture/'Makefile').write_bytes(cell.source);(fixture/'inc/input.mk').write_bytes(b'VALUE=ok\n')
        for iteration in range(repeat):
            before,trace1=observe(cell,fixture,workspace,reference,library)
            again,trace2=observe(cell,fixture,workspace,reference,library)
            errors=anchors(cell,before,trace1)+anchors(cell,again,trace2)+harness.differences(before,again)
            if errors:
                failure={'case':cell.id,'errors':errors,'traces':[trace1,trace2],
                         'observations':[dict(harness.observation_json(o),stdout_hex=o.stdout.hex(),stderr_hex=o.stderr.hex()) for o in (before,again)]}
                (destination/('oracle-failure-'+cell.id+'.json')).write_text(json.dumps(failure,indent=2)+'\n')
                raise harness.HarnessError(f'oracle include-stat model/observer disagreement: {cell.id}: {errors}')
            after,trace3=observe(cell,fixture,workspace,candidate,library)
            errors=anchors(cell,after,trace3)+harness.differences(before,after)
            records.append({'case':cell.id,'repeat':iteration,'source_sha256':hashlib.sha256(cell.source).hexdigest(),'expected':cell.expected,'arguments':cell.arguments,'errors':errors,'traces':[trace1,trace2,trace3],
                            'observations':[dict(harness.observation_json(o),stdout_hex=o.stdout.hex(),stderr_hex=o.stderr.hex()) for o in (before,again,after)]})
            print('FAIL' if errors else 'PASS',cell.id,iteration,errors,flush=True)
    return {'observer':identity,'abi_qualification':abi,'records':records,
            'credit':'Separate Linux/glibc fault profiles; no extra curated-case or safety-exception credit.'}


def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle',type=Path,default=oracle.PREFIX/'bin/make')
    parser.add_argument('--candidate',type=Path)
    parser.add_argument('--self-test',action='store_true')
    parser.add_argument('--repeat',type=int,default=3)
    parser.add_argument('--case',action='append',default=[])
    parser.add_argument('--artifacts',type=Path,default=ROOT/'artifacts/include-stat')
    args=parser.parse_args(argv)
    if bool(args.candidate)==args.self_test:parser.error('choose candidate or self-test')
    if not 1<=args.repeat<=100:parser.error('repeat must be 1-100')
    cells=inventory()
    if set(args.case)-{c.id for c in cells}:parser.error('unknown case')
    if args.case:cells=tuple(c for c in cells if c.id in args.case)
    reference=args.oracle.resolve();profile=oracle.verify(reference)
    candidate=reference if args.self_test else args.candidate.resolve()
    identities={'oracle':harness.executable_identity(reference,oracle=True),'candidate':harness.executable_identity(candidate)}
    args.artifacts.mkdir(parents=True,exist_ok=True)
    destination=Path(tempfile.mkdtemp(prefix='include-stat-',dir=args.artifacts.resolve()))
    report=campaign(cells,reference,candidate,destination,args.repeat)
    report.update(identities=identities,oracle_profile=profile,runner_sha256=harness.sha256_file(Path(__file__)))
    (destination/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    print('Report:',destination/'report.json')
    return int(any(r['errors'] for r in report['records']))

if __name__=='__main__':
    try:raise SystemExit(main())
    except (OSError,subprocess.SubprocessError,oracle.OracleError,harness.HarnessError) as error:
        raise SystemExit('include-stat: '+str(error))
