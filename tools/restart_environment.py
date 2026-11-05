# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original finite raw process-vector GNU2 differential controls."""
from pathlib import Path
import argparse,base64,hashlib,json,os,re,shutil,tempfile
ROOT=Path(__file__).resolve().parents[1]
import conformance as h
import oracle

PLAN=ROOT/'tests/restart-environment/models.json'

def file_record(value):
    return {'type':'file','mode':0o644,'size':len(value),'sha256':hashlib.sha256(value).hexdigest(),
            'content':base64.b64encode(value).decode('ascii')}

def run(row,sources,root,binaries,compiler):
    inputs=root/row['id'];inputs.mkdir()
    files={'entry.c':sources['constructor'].encode(),'launch.py':sources['launcher'].encode(),
           'Makefile':row.get('makefile',sources['makefile']).encode(),
           'extra-entries':('\n'.join(row['raw'])+'\n').encode(),'seen':b'anchor\n'}
    modules=[('entry.c','entry.so')]
    if 'extra_module'in row:
        extra=row['extra_module'];files[extra['source']]=bytes.fromhex(extra['bytes_hex']);modules.append((extra['source'],extra['output']))
    for name,value in files.items():(inputs/name).write_bytes(value)
    compilations=[]
    for source,output in modules:
        argv=[compiler,'-std=c11','-Wall','-Wextra','-Werror','-O2','-shared','-fPIC','-o',str(inputs/output),str(inputs/source)]
        out,err,status,limit,leftovers=h.capture(argv,inputs,{'PATH':'/usr/bin:/bin','LC_ALL':'C','TZ':'UTC'},15)
        if status or limit or leftovers:raise h.HarnessError('original module compilation failed: '+repr((status,out,err,limit,leftovers)))
        compilations.append({'argv':argv,'source_sha256':h.sha256_file(inputs/source),
                             'output_sha256':h.sha256_file(inputs/output),'stdout_hex':out.hex(),'stderr_hex':err.hex()})
    h.validate_fixture(inputs)
    case=h.Case(inputs,row['id'],'restart.raw-vector','required',(),6.0,{},{});runtime=root/'runtime';runtime.mkdir(exist_ok=True)
    seen=bytes.fromhex(row['seen']);observations=[]
    for binary in binaries:
        h.reset_workspace(case,runtime,binary)
        roots={name:runtime/name for name in ('work','home','tmp','bin')}
        expected=h.snapshot(roots)
        expected['work/seen']=file_record(seen);expected['work/generated.mk']=file_record(b'READY:=yes\n')
        environment=h.clean_environment(runtime,{'LD_PRELOAD':str(runtime/'work/entry.so'),'MAKELEVEL':'2'})
        out,err,status,limit,leftovers=h.capture(['/usr/bin/python3','launch.py',str(runtime/'bin/make')],runtime/'work',environment,case.timeout)
        actual=h.snapshot(roots)
        errors=[name for name,wrong in [('stdout',bool(out)),('stderr',bool(err)),('status',status!=0),
                    ('bounds',bool(limit or leftovers)),('complete filesystem and ordered witness',actual!=expected)]if wrong]
        observations.append({'binary':str(binary),'stdout_hex':out.hex(),'stderr_hex':err.hex(),'status':status,
                             'limit':limit,'leftover_processes':leftovers,'filesystem':actual,
                             'expected_filesystem':expected,'errors':errors,'passed':not errors})
    repeated={k:v for k,v in observations[0].items()if k!='binary'}=={k:v for k,v in observations[1].items()if k!='binary'}
    return {'id':row['id'],'expected_seen_hex':seen.hex(),'inputs_hex':{n:v.hex()for n,v in files.items()},
            'compilations':compilations,'observations':observations,'gnu_repeated_exact':repeated,
            'gnu2_qualified':repeated and all(o['passed']for o in observations[:2]),
            'passed':repeated and all(o['passed']for o in observations)}

def models():
    data=json.loads(PLAN.read_bytes())
    if data['schema_version']!=1:raise h.HarnessError('unsupported raw environment model schema')
    names=set()
    for row in data['models']:
        name=row['id']
        if not re.fullmatch(r'[a-z0-9]+(?:-[a-z0-9]+)*',name)or name in names:
            raise h.HarnessError('invalid or duplicate raw environment case ID')
        names.add(name)
        entries=[bytes.fromhex(value)for value in row['raw']]
        if len(entries)>32 or any(b'\0'in value or len(value)>4096 for value in entries):
            raise h.HarnessError('raw environment fixture exceeds its declared byte/entry bounds')
        bytes.fromhex(row['seen'])
        if 'extra_module'in row:
            extra=row['extra_module']
            for key,extension in [('source','.c'),('output','.so')]:
                name=extra[key]
                if Path(name).name!=name or not name.endswith(extension)or name in ('entry.c','entry.so'):
                    raise h.HarnessError('raw environment module path must be a distinct fixture basename')
            bytes.fromhex(extra['bytes_hex'])
    if not names:raise h.HarnessError('empty raw environment model inventory')
    return data

def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle',type=Path,default=ROOT/'.tools/oracle/bin/make')
    parser.add_argument('--candidate',type=Path)
    parser.add_argument('--oracle-only',action='store_true')
    parser.add_argument('--report',type=Path,default=ROOT/'artifacts/restart-environment/campaign.json')
    parser.add_argument('--case',action='append',default=[])
    parser.add_argument('--cc',default='cc')
    args=parser.parse_args(argv)
    if args.oracle_only==(args.candidate is not None):parser.error('supply either --candidate or --oracle-only')
    report={'passed':False}
    try:
        reference=args.oracle.resolve();oracle.verify(reference);data=models()
        selected=[row for row in data['models']if not args.case or row['id']in args.case]
        if not selected or set(args.case)-{row['id']for row in selected}:
            raise h.HarnessError('unknown or empty raw environment case selection')
        compiler=shutil.which(args.cc)
        if compiler is None:raise h.HarnessError('raw environment gate requires a C compiler')
        identities={'oracle':h.executable_identity(reference,oracle=True)}
        binaries=[reference,reference]
        if args.candidate is not None:
            candidate=args.candidate.resolve();identities['candidate']=h.executable_identity(candidate);binaries.append(candidate)
        previous_umask=os.umask(0o022)
        try:
            with tempfile.TemporaryDirectory(prefix='rmake-raw-env-')as directory:
                records=[run(row,data['sources'],Path(directory),binaries,compiler)for row in selected]
        finally:
            os.umask(previous_umask)
        passed=all(row['passed']for row in records)
        report={'classification':'Original strict raw-vector reexec profiles; no ordinary-ledger, milestone, exception or performance credit',
                'passed':passed,'oracle_only':args.oracle_only,'identities':identities,
                'functional_cpu_affinity':sorted(os.sched_getaffinity(0)),
                'source_hashes':{'driver':h.sha256_file(Path(__file__)),'models':h.sha256_file(PLAN),
                                 'conformance':h.sha256_file(ROOT/'tools/conformance.py')},
                'totals':{'cases':len(records),'gnu2':sum(row['gnu2_qualified']for row in records),
                          'pass':sum(row['passed']for row in records),'fail':sum(not row['passed']for row in records)},
                'records':records}
        status=0 if passed else 1
    except(h.HarnessError,oracle.OracleError,OSError,ValueError,TypeError,KeyError)as error:
        report.update(error=str(error),passed=False);status=2
        print('ERROR '+str(error),flush=True)
    args.report.parent.mkdir(parents=True,exist_ok=True)
    args.report.write_text(json.dumps(report,indent=2)+'\n')
    print('Raw restart environment gate:',report.get('totals',{'error':True}),flush=True)
    return status
if __name__=='__main__':raise SystemExit(main())
