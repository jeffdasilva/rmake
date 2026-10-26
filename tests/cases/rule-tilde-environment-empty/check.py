import os,sys
from pathlib import Path
assert os.fsencode(sys.argv[1])==Path("expected-home").read_bytes()
print("environment-home-ok")
