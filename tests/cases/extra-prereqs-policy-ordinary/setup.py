from pathlib import Path
import os
for name,seconds in [('input',0),('hidden.src',1),('out',10)]:
 Path(name).write_bytes(b'original\n');os.utime(name,ns=((1600000000+seconds)*10**9,)*2)
