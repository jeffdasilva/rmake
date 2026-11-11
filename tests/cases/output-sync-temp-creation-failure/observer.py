import os
from pathlib import Path
import re
import stat
import subprocess
import sys

relative = Path('relative')
shifted = Path('shifted')
relative.mkdir()
shifted.mkdir()
for mode, reader, suppressions in (('none', False, 0), ('line', False, 2),
                                  ('target', False, 2), ('recurse', False, 1),
                                  ('recurse', True, 1)):
    env = dict(os.environ, MAKE_TMPDIR='relative///', READER='yes' if reader else '')
    command = [sys.argv[1], '-C', 'shifted', '-f', '../worker.mk', '-j2',
               '--jobserver-style=pipe', f'-O{mode}']
    completed = subprocess.run(command, env=env, capture_output=True, timeout=2, umask=0o022)
    expected = b'reader\n' if reader else b''
    expected += f'executing:rR -j2 -O{mode}'.encode()
    expected += b' --jobserver-auth=3,4 --no-print-directory --jobserver-auth=-2,-2\n'
    assert completed.returncode == 0 and completed.stdout == expected, (mode, completed)
    if suppressions:
        suppression = b'make[1]: cannot open output-sync lock file, suppressing output-sync.\n'
        first = re.escape(b'make[1]: cannot create temporary file relative///Gm')
        first += b'[A-Za-z0-9]{6}' + re.escape(b': No such file or directory\n')
        assert re.fullmatch(first + re.escape(suppression) * suppressions, completed.stderr), (
            mode, completed.stderr
        )
    else:
        assert not completed.stderr, completed.stderr
    witness = shifted / 'witness'
    assert witness.read_bytes() == b'payload\n'
    assert stat.S_IMODE(witness.stat().st_mode) == (0o600 if suppressions else 0o644)
    witness.unlink()
    print('verified ' + mode + (' with reader' if reader else ''))
shifted.rmdir()
relative.rmdir()
