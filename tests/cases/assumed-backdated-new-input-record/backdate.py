from pathlib import Path
import os
print('DEP')
Path('dep').write_bytes(b'backdated\n')
os.utime('dep',ns=(1500000100000000000,1500000100000000000))
