from pathlib import Path
import os
p=Path('seen')
old=p.read_bytes() if p.exists() else b''
p.write_bytes(old+b'E\n')
Path('generated.mk').write_bytes(b'# generated\n')
n=1000000000000000000+len(old)+2
os.utime('generated.mk',ns=(n,n))
