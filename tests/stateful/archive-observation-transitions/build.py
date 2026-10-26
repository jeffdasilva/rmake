from pathlib import Path
import os,sys
Path('out').write_bytes(b'original output\n')
os.utime('out',ns=(int(Path('next-time').read_text()),)*2)
print('built=['+'|'.join(sys.argv[1:])+']')
