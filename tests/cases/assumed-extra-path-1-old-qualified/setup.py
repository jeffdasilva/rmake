from pathlib import Path
import os
Path('inputs').mkdir()
for n,t in [('all',400),('inputs/dep',300)]:
 Path(n).write_text('initial\n');os.utime(n,ns=((1500000000+t)*10**9,)*2)
