import fcntl
import os
import stat

readers, writers = set(), set()
for fd in range(3, 65):
    try:
        metadata = os.fstat(fd)
        access = fcntl.fcntl(fd, fcntl.F_GETFL) & os.O_ACCMODE
    except OSError:
        continue
    if stat.S_ISFIFO(metadata.st_mode):
        identity = metadata.st_dev, metadata.st_ino
        if access == os.O_RDONLY:
            readers.add(identity)
        if access == os.O_WRONLY:
            writers.add(identity)
print('borrowed-pipe=' + ('open' if readers & writers else 'closed'))
