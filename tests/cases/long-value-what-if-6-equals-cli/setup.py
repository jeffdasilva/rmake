from pathlib import Path
import os
for n,rank in [('all',400),('dep',300),('pre',500)]:
 Path(n).write_text(n+'-initial\n');os.utime(n,ns=((1500000000+rank)*10**9,)*2)
