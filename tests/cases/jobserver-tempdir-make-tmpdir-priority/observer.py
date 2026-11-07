import os
from pathlib import Path
import stat
import sys

expected_dir, style = sys.argv[1:]
auth = next(word.split('=', 1)[1] for word in os.environ['MAKEFLAGS'].split()
            if word.startswith('--jobserver-auth='))
if style == 'fifo':
    assert auth.startswith('fifo:'), auth
    path = Path(auth[5:])
    assert path.parent == Path(expected_dir), path
    assert path.name == f'GMfifo{os.getppid()}', path
    assert stat.S_ISFIFO(path.stat().st_mode), path
    Path('owned-fifo-path').write_text(str(path))
else:
    assert all(descriptor.isdigit() for descriptor in auth.split(',')), auth
    assert len(auth.split(',')) == 2, auth
print('verified ' + style)
