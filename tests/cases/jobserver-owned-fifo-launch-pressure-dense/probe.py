'import os
import re
import stat
import sys

path = re.search(r"--jobserver-auth=fifo:([^ ]+)", os.environ["MAKEFLAGS"]).group(1)
assert stat.S_ISFIFO(os.stat(path).st_mode)
for fd in range(3, 64):
    try:
        metadata = os.fstat(fd)
    except OSError:
        continue
    assert not stat.S_ISFIFO(metadata.st_mode)

if sys.argv[1] == "recursive":
    descriptor = os.open(path, os.O_RDWR | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        token = os.read(descriptor, 1)
        assert token == b"+"
        assert os.write(descriptor, token) == 1
    finally:
        os.close(descriptor)
print(sys.argv[1])
