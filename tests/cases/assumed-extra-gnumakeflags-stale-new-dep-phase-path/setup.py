from pathlib import Path
import os
Path('all').write_bytes(b'all-initial\n')
os.utime('all',ns=(1500000400000000000,1500000400000000000))
Path('dep').write_bytes(b'dep-initial\n')
os.utime('dep',ns=(1500000300000000000,1500000300000000000))
Path('pre').write_bytes(b'pre-initial\n')
os.utime('pre',ns=(1500000500000000000,1500000500000000000))
