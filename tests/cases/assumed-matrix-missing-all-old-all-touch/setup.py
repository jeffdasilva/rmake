from pathlib import Path
import os
Path('dep').write_bytes(b'dep-initial\n')
os.utime('dep',ns=(1500000300000000000,1500000300000000000))
Path('pre').write_bytes(b'pre-initial\n')
os.utime('pre',ns=(1500000200000000000,1500000200000000000))
