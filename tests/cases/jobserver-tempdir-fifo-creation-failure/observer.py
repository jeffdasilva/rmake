import os
from pathlib import Path
import stat
import subprocess
import sys


def verify_pipe():
    flags = os.environ['MAKEFLAGS'].split(' -- ', 1)[0].split()
    auth, = [word.split('=', 1)[1] for word in flags if word.startswith('--jobserver-auth=')]
    reader, writer = map(int, auth.split(','))
    assert stat.S_ISFIFO(os.fstat(reader).st_mode)
    assert os.fstat(reader).st_ino == os.fstat(writer).st_ino
    os.set_blocking(reader, False)
    token = os.read(reader, 1)
    assert token == b'+'
    assert os.write(writer, token) == 1
    print('verified pipe')


def create_collision(kind, make, *options):
    path = Path('shifted/relative') / f'GMfifo{os.getpid()}'
    if kind == 'file':
        path.write_bytes(b'preserve')
    elif kind == 'fifo':
        os.mkfifo(path)
    else:
        path.symlink_to('absent')
    os.execv(make, [make, *options])


def verify_collision(path, kind):
    if kind == 'file':
        assert stat.S_ISREG(path.lstat().st_mode) and path.read_bytes() == b'preserve'
    elif kind == 'fifo':
        assert stat.S_ISFIFO(path.lstat().st_mode)
    else:
        assert path.is_symlink() and os.readlink(path) == 'absent'
    path.unlink()


def verify_creation_failure(make):
    relative = Path('relative')
    shifted = Path('shifted')
    relative.mkdir()
    shifted.mkdir()
    for name, options, inherited, status, warns, collision in (
        ('default', [], '', 0, True, None),
        ('explicit fifo', ['--jobserver-style=fifo'], '', 2, True, None),
        ('inherited fifo', [], ' --jobserver-style=fifo', 2, True, None),
        ('file fifo', ['--jobserver-style=pipe', 'FROM_FILE=yes'], '', 2, True, None),
        ('explicit pipe', ['--jobserver-style=pipe'], '', 0, False, None),
        ('file collision', [], '', 0, True, 'file'),
        ('fifo collision', ['--jobserver-style=fifo'], '', 2, True, 'fifo'),
        ('symlink collision', [], '', 0, True, 'symlink'),
    ):
        env = dict(os.environ, MAKE_TMPDIR='relative///', MAKEFLAGS=os.environ['MAKEFLAGS'] + inherited)
        command = [make, '-C', 'shifted', '-f', '../worker.mk', '-j2', *options]
        if collision:
            (shifted / 'relative').mkdir()
            command = [sys.executable, __file__, 'create-collision', collision, *command]
        with subprocess.Popen(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE) as child:
            try:
                stdout, stderr = child.communicate(timeout=2)
            except subprocess.TimeoutExpired:
                child.kill()
                child.communicate()
                raise
        expected = b''
        filename = f'GMfifo{child.pid}'
        if warns:
            error = 'File exists' if collision else 'No such file or directory'
            expected += f'make[1]: jobserver mkfifo: relative////{filename}: {error}\n'.encode()
        if status:
            expected += b"make[1]: *** unknown jobserver auth style 'fifo'.  Stop.\n"
        assert (child.returncode, stdout, stderr) == (
            status, b'' if status else b'verified pipe\n', expected
        ), (name, child.returncode, stdout, stderr)
        if collision:
            verify_collision(shifted / 'relative' / filename, collision)
            (shifted / 'relative').rmdir()
        print('verified ' + name)
    shifted.rmdir()
    relative.rmdir()


if sys.argv[1] == 'verify-pipe':
    verify_pipe()
elif sys.argv[1] == 'create-collision':
    create_collision(*sys.argv[2:])
else:
    verify_creation_failure(sys.argv[1])
