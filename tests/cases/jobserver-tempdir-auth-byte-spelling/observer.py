import os
from pathlib import Path
import subprocess
import sys

if sys.argv[1] == 'show':
    for name in (b'MAKEFLAGS', b'MFLAGS'):
        sys.stdout.buffer.write(os.environb[name] + b'\0')
else:
    directory = b'dir-\n-\r-\x0b-\x0c-\t- -\\-\xff-rest'
    escaped = b'dir-\n-\r-\x0b-\x0c-\\\t-\\ -\\\\-\xff-rest'
    path = Path(os.fsdecode(directory))
    path.mkdir()
    env = {**os.environb, b'MAKE_TMPDIR': directory}
    command = [sys.argv[1], '-f', 'worker.mk', '-j2']
    with subprocess.Popen(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE) as child:
        try:
            stdout, stderr = child.communicate(timeout=2)
        except subprocess.TimeoutExpired:
            child.kill()
            child.communicate()
            raise
    flags = (b'rR -j2 --jobserver-auth=fifo:' + escaped + b'/GMfifo'
             + str(child.pid).encode() + b' --no-print-directory\0')
    assert (child.returncode, stdout, stderr) == (0, (flags + b'-' + flags) * 2, b''), (
        child.returncode, stdout, stderr
    )
    path.rmdir()
    print('verified FIFO auth bytes')
