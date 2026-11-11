import os
from pathlib import Path
import re
import stat
import subprocess
import sys

relative = Path('relative')
shifted = Path('shifted')
working_temp = shifted / relative
relative.mkdir()
shifted.mkdir()
error = (re.escape(b'make[1]: cannot create temporary file relative///Gm')
         + b'[A-Za-z0-9]{6}' + re.escape(b': No such file or directory\n'))
suppression = re.escape(b'make[1]: cannot open output-sync lock file, suppressing output-sync.\n')
for mode, jobs, situation in (('line', 2, 'parallel'), ('target', 2, 'parallel'),
                              ('line', 1, 'serial'), ('target', 1, 'serial'),
                              ('line', 2, 'working'), ('target', 2, 'working'),
                              ('target', 2, 'mode off')):
    working = situation == 'working'
    if working:
        working_temp.mkdir()
    env = dict(os.environ, MAKE_TMPDIR='relative///',
               READER_OFF='yes' if situation == 'mode off' else '')
    command = [sys.argv[1], '-C', 'shifted', '-f', '../worker.mk', f'-j{jobs}',
               '--jobserver-style=pipe', f'-O{mode}']
    result = subprocess.run(command, env=env, stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, timeout=2, umask=0o022)
    expected = b'' if working else error + suppression
    expected += b'reader\n'
    if situation == 'parallel':
        expected += error + suppression * 2
    expected += b'recipe\n'
    assert result.returncode == 0 and re.fullmatch(expected, result.stdout), (
        mode, jobs, situation, result
    )
    witness = shifted / 'witness'
    assert witness.read_bytes() == b'payload\n'
    assert stat.S_IMODE(witness.stat().st_mode) == (0o644 if working else 0o600)
    witness.unlink()
    if working:
        working_temp.rmdir()
    print(f'verified {mode} {situation}')
shifted.rmdir()
relative.rmdir()
