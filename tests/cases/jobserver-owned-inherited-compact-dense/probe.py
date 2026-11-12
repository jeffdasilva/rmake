'import os
import re
import stat
import sys

match = re.search(r"--jobserver-auth=([0-9]+),([0-9]+)", os.environ.get("MAKEFLAGS", ""))
pipes = []
for fd in range(3, 64):
    try:
        metadata = os.fstat(fd)
    except OSError:
        continue
    if stat.S_ISFIFO(metadata.st_mode):
        pipes.append(fd)

if sys.argv[1] == "recursive":
    assert match
    reader, writer = map(int, match.groups())
    assert sorted((reader, writer)) == pipes
    assert os.fstat(reader).st_ino == os.fstat(writer).st_ino
else:
    assert not pipes
print(sys.argv[1])
