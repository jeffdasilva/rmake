# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original bounded pipe launcher and separately sampled owned-temp identity."""
import json
import os
from pathlib import Path
import signal
import stat
import sys
import time

source=Path(sys.argv[1]).read_bytes()
mode=sys.argv[2]
identity_output=Path(sys.argv[3])
binary=sys.argv[4]
args=sys.argv[5:]
read,write=os.pipe()
assert len(source)<8192
if mode in('signal','identity'):
    if mode=='identity':os.write(write,source)
    pid=os.fork()
    if pid==0:
        os.close(write);os.dup2(read,0,inheritable=True);os.close(read)
        os.execve(binary,[binary,*args],os.environ)
    os.close(read)
    if mode=='identity':os.close(write)
    reaped=False
    def require_live():
        global reaped
        if os.waitpid(pid,os.WNOHANG)[0]:
            reaped=True
            raise RuntimeError('child exited before identity observation completed')
    try:
        if mode=='signal':os.write(write,b'# deliberately open stdin\n')
        directory=Path(os.environ['TMPDIR'])
        deadline=time.monotonic()+3
        while True:
            require_live()
            choices=list(directory.glob('Gm*'))
            if len(choices)>1:raise RuntimeError('more than one owned-temp candidate')
            if choices:
                path=choices[0]
                descriptor=os.open(path,os.O_RDONLY|os.O_NOFOLLOW)
                with os.fdopen(descriptor,'rb')as file:
                    before=os.fstat(file.fileno())
                    if mode=='signal'or before.st_size>=len(source):
                        content=file.read(8193);after=os.fstat(file.fileno())
                        if (before.st_dev,before.st_ino,before.st_mode,before.st_size,before.st_mtime_ns)!=(after.st_dev,after.st_ino,after.st_mode,after.st_size,after.st_mtime_ns):raise RuntimeError('source changed during independent observation')
                        if not stat.S_ISREG(before.st_mode):raise RuntimeError('owned source is not regular')
                        require_live()
                        capture={'path_hex':os.fsencode(path).hex(),'device':before.st_dev,'inode':before.st_ino,
                                 'mode':stat.S_IMODE(before.st_mode),'size':before.st_size,'content_hex':content.hex(),
                                 'captured_before_exit':True}
                        identity_output.write_text(json.dumps(capture,indent=2)+'\n')
                        break
            if time.monotonic()>deadline:raise RuntimeError('owned source was never available')
            time.sleep(.001)
        if mode=='signal':
            with open('seen','a')as file:file.write('CONTROLLER=source-created\n')
            os.kill(pid,signal.SIGTERM)
        else:Path('source-observed').write_bytes(b'ready\n')
        waited,status=os.waitpid(pid,0)
        reaped=True
        assert waited==pid
        if mode=='signal':
            with open('seen','a')as file:file.write('CONTROLLER=child-finished\n')
    except BaseException:
        if not reaped:
            try:os.kill(pid,signal.SIGKILL)
            except ProcessLookupError:pass
            try:os.waitpid(pid,0)
            except ChildProcessError:pass
        raise
    finally:
        if mode=='signal':os.close(write)
    if os.WIFSIGNALED(status):
        number=os.WTERMSIG(status);signal.signal(number,signal.SIG_DFL);os.kill(os.getpid(),number)
    sys.exit(os.WEXITSTATUS(status))
os.write(write,source);os.close(write);os.dup2(read,0,inheritable=True);os.close(read)
if mode=='merged':os.dup2(1,2)
if mode=='cosmetic':
    descriptor=os.open('/dev/null',os.O_WRONLY);os.dup2(descriptor,1);os.close(descriptor)
os.execve(binary,[binary,*args],os.environ)
