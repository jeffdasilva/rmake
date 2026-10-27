import os
from pathlib import Path
import stat
import sys
import time

def wait(predicate):
    deadline = time.monotonic() + 3
    while not predicate():
        if time.monotonic() >= deadline:
            raise RuntimeError('bounded stdin handoff did not complete')
        time.sleep(.002)

mode = sys.argv[1]
if mode == 'a':
    Path('a.pid').write_text(str(os.getpid()))
    wait(lambda: Path('b.ready').exists())
elif mode == 'b':
    wait(lambda: Path('a.pid').exists())
    assert stat.S_ISFIFO(os.fstat(0).st_mode), 'unused stdin must be a pipe'
    assert os.read(0, 1) == b'', 'unused stdin must have no writer'
    print('blocked=pipe/eof', flush=True)
    pid = int(Path('a.pid').read_text())
    Path('b.ready').write_text('ready\n')
    wait(lambda: not Path('/proc', str(pid)).exists())
    Path('a.pid').unlink()
else:
    assert stat.S_ISFIFO(os.fstat(0).st_mode), 'handoff lost original pipe'
    value = bytearray()
    while (byte := os.read(0, 1)) and byte != b'\n':
        value.extend(byte)
    print(mode + '=' + value.decode('ascii'), flush=True)
