from pathlib import Path
import os
for n,t in [('generated.mk',300),('input',500)]:
 Path(n).write_bytes(b'G=old\n'if n=='generated.mk'else b'input\n');os.utime(n,ns=((1500000000+t)*10**9,)*2)
