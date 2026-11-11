import os
from pathlib import Path
import stat
import sys

mode = sys.argv[1]
record = Path('owned-fifo-path')
if mode == 'warnings':
    path = record.read_text()
    assert Path('chdir.err').read_text() == (
        f'make[2]: cannot open jobserver {path}: No such file or directory\n'
        "make[2]: warning: jobserver unavailable: using -j1.  Add '+' to parent make rule.\n"
    )
else:
    flags = os.environ['MAKEFLAGS'].split(' -- ', 1)[0].split()
    auth = next((word.split('=', 1)[1] for word in flags
                 if word.startswith('--jobserver-auth=')), '')
    if mode == 'unavailable':
        assert not auth and '-j1' in flags, flags
    else:
        assert auth.startswith('fifo:'), auth
        path = auth[5:]
        assert stat.S_ISFIFO(Path(path).stat().st_mode), path
        if mode == 'owner':
            assert path == f'{sys.argv[2]}GMfifo{os.getppid()}', path
            record.write_text(path)
        else:
            assert path == record.read_text(), path
print('verified ' + mode)
