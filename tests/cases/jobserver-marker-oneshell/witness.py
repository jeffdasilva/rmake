import fcntl
import os
import stat
import sys

auth = [word.split('=', 1)[1] for word in os.environ['MAKEFLAGS'].split()
        if word.startswith('--jobserver-auth=')]

def endpoint(fd, access):
    try:
        metadata = os.fstat(fd)
        mode = fcntl.fcntl(fd, fcntl.F_GETFL) & os.O_ACCMODE
    except OSError:
        return None
    if stat.S_ISFIFO(metadata.st_mode) and mode == access:
        return metadata.st_dev, metadata.st_ino

reader, writer = map(int, auth[0].split(','))
identity = endpoint(reader, os.O_RDONLY)
wire = identity is not None and identity == endpoint(writer, os.O_WRONLY)
print(f"{sys.argv[1]}: {'open' if wire else 'closed'} "
      f"{'disabled' if auth[-1] == '-2,-2' else 'enabled'}")
