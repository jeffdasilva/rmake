import os
from pathlib import Path
import re
import subprocess
import sys

if sys.argv[1] == 'record':
    Path(sys.argv[2]).write_text(f'{os.getpid()} {os.getppid()}')
else:
    temporary = Path('scratch').resolve()
    temporary.mkdir()
    env = dict(os.environ, MAKE_TMPDIR=str(temporary))
    for style in ('fifo', 'pipe'):
        for mode in ('none', 'line', 'target', 'recurse'):
            command = [sys.argv[1], '-f', 'worker.mk', '-j2', '--debug=j',
                       f'--jobserver-style={style}']
            if mode != 'none':
                command.append(f'-O{mode}')
            with subprocess.Popen(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE) as child:
                try:
                    stdout, stderr = child.communicate(timeout=2)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.communicate()
                    raise
            pids = []
            for name in ('one', 'two', 'three'):
                record = Path(name)
                pid, parent = map(int, record.read_text().split())
                assert parent == child.pid and pid not in pids, (pid, parent, child.pid)
                pids.append(pid)
                record.unlink()
            assert child.returncode == 0 and not stderr, (style, mode, child.returncode, stderr)
            identity = re.search(rb'(?m)^Putting child (0x[0-9a-f]+) ', stdout)[1]
            assert int(identity, 16) >= 4096 and int(identity, 16) % 8 == 0, identity
            auth = (b'fifo:' + os.fsencode(temporary / f'GMfifo{child.pid}')
                    if style == 'fifo' else b'3,4')
            expected = [b'Using jobserver controller ' + auth,
                        b"Need a job token; we don't have children",
                        b'Putting child %s (all) PID %d on the chain.' % (identity, pids[0])]
            for pid in pids:
                expected.append(b'Live child %s (all) PID %d ' % (identity, pid))
                expected.append(b'Reaping winning child %s PID %d ' % (identity, pid))
            expected.append(b'Removing child %s PID %d from chain.' % (identity, pids[-1]))
            expected.append(b'Acquired all 1 jobserver tokens.')
            assert stdout == b'\n'.join(expected) + b'\n', (style, mode, stdout, expected)
            print(f'verified {style} {mode}')
    temporary.rmdir()
