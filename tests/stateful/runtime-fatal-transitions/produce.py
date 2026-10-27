from pathlib import Path
import os
Path('target').write_bytes(b'original target\n')
os.utime('target',ns=(int(Path('next-time').read_text()),)*2)
Path('trigger').write_bytes(b'armed\n')
os.utime('trigger',ns=(1600000000000000000,)*2)
