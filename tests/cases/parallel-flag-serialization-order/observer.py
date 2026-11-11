import os
from pathlib import Path
import re
import subprocess
import sys

if sys.argv[1] == 'show':
    for name in (b'MAKEFLAGS', b'MFLAGS'):
        sys.stdout.buffer.write(os.environb[name] + b'\n')
else:
    temporary = Path('scratch').resolve()
    temporary.mkdir()
    env = dict(os.environ, MAKE_TMPDIR=str(temporary), GNUMAKEFLAGS='')
    for transport in ('fifo', 'pipe'):
        for mode in ('none', 'target', 'recurse'):
            command = [sys.argv[1], '-f', 'worker.mk', '-j2', '--debug=j', '--debug=n',
                       f'--jobserver-style={transport}']
            if mode != 'none':
                command.extend((f'-O{mode}', '--warn-undefined-variables', '-s', '--no-silent',
                                '--shuffle=reverse'))
            with subprocess.Popen(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE) as child:
                try:
                    stdout, stderr = child.communicate(timeout=2)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.communicate()
                    raise
            assert child.returncode == 0 and not stderr, (transport, mode, child.returncode, stderr)
            owner, owner_mflags, peer, peer_mflags = stdout.splitlines()
            expected = b'rR -j2'
            if mode != 'none':
                expected += f' -O{mode}'.encode()
            expected += b' --debug=j --debug=n --jobserver-auth='
            if transport == 'fifo':
                expected += b'fifo:' + os.fsencode(temporary / f'GMfifo{child.pid}')
            else:
                expected += b'3,4'
            expected += b' --no-print-directory'
            if mode != 'none':
                expected += b' --warn-undefined-variables'
                if mode == 'target':
                    mutex, = (part for part in owner.split() if part.startswith(b'--sync-mutex='))
                    prefix = b'--sync-mutex=fnm:' + os.fsencode(temporary) + b'/Gm'
                    assert re.fullmatch(re.escape(prefix) + b'[A-Za-z0-9]{6}', mutex), mutex
                    expected += b' ' + mutex
                expected += b' --no-silent --shuffle=reverse'
            expected_peer = expected
            if transport == 'pipe':
                expected_peer += b' --jobserver-auth=-2,-2'
            assert (owner, owner_mflags, peer, peer_mflags) == (
                expected, b'-' + expected, expected_peer, b'-' + expected_peer
            ), (transport, mode, stdout)
            print(f'verified {transport} {mode}')
    temporary.rmdir()
