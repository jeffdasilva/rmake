from pathlib import Path
import sys
assert sys.argv[1:]==['-c','-o','demo.o','demo.c'],sys.argv
Path('demo.o').write_bytes(b'original object\n')
print('compile')
