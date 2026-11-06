import fcntl
import os
import stat
import sys

def live(fd, direction):
    try:
        metadata = os.fstat(fd)
        mode = fcntl.fcntl(fd, fcntl.F_GETFL) & os.O_ACCMODE
    except OSError:
        return False
    return (stat.S_ISFIFO(metadata.st_mode) and mode == direction
            and f'{metadata.st_dev}:{metadata.st_ino}' == os.environ['WIRE_IDENTITY'])

wire = live(50, os.O_RDONLY) and live(51, os.O_WRONLY)
flags = [flag for flag in os.environ['MAKEFLAGS'].split() if flag.startswith('--jobserver-auth=')]
print(sys.argv[1] + '=[' + ('open' if wire else 'closed') + '|' + ' '.join(flags) + ']')
