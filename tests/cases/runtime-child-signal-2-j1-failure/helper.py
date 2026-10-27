import ctypes
import os
from pathlib import Path
import signal
import sys
assert ctypes.CDLL(None).prctl(4, 0, 0, 0, 0) == 0
number = int(sys.argv[1])
signal.signal(number, signal.SIG_DFL)
Path('out').write_bytes(b'changed\n')
os.kill(os.getpid(), number)
