import os
from pathlib import Path
import re
import subprocess
import sys

if sys.argv[1] == 'show':
    paths = []
    for name in (b'MAKEFLAGS', b'MFLAGS'):
        prefix = b'--sync-mutex=fnm:'
        path, = (word.removeprefix(prefix) for word in os.environb[name].split()
                 if word.startswith(prefix))
        assert Path(os.fsdecode(path)).is_file(), path
        paths.append(path)
    assert paths[0] == paths[1], paths
    sys.stdout.buffer.write(paths[0] + b'\n')
else:
    fixture = str(Path.cwd())
    Path('relative').mkdir()
    Path('elsewhere').mkdir()
    for mode, directory, status in (('relative', './relative///', 2),
                                    ('absolute', fixture + '/relative///', 0)):
        env = dict(os.environ, MAKE_TMPDIR=directory, FIXTURE=fixture)
        command = [sys.argv[1], '-f', 'worker.mk', '-j2', '--jobserver-style=pipe', '-Otarget']
        with subprocess.Popen(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE) as child:
            try:
                stdout, stderr = child.communicate(timeout=2)
            except subprocess.TimeoutExpired:
                child.kill()
                child.communicate()
                raise
        path = stdout.splitlines()[0]
        assert re.fullmatch(re.escape(os.fsencode(directory)) + b'Gm[A-Za-z0-9]{6}', path), path
        expected_stderr = (b'make[2]: *** cannot open output sync mutex ' + path
                           + b': No such file or directory.  Stop.\n'
                           + b'make[1]: *** [worker.mk:5: owner] Error 2\n') if status else b''
        assert (child.returncode, stdout, stderr) == (
            status, (path + b'\n') * (2 if status else 3), expected_stderr
        ), (mode, child.returncode, stdout, stderr)
        assert not Path(os.fsdecode(path)).exists(), path
        print('verified ' + mode)
    Path('elsewhere').rmdir()
    Path('relative').rmdir()
