import os
import sys

make, auth = sys.argv[1:]
read, write = os.pipe()
metadata = os.fstat(read)
for existing, advertised in ((read, 50), (write, 51)):
    os.dup2(existing, advertised, inheritable=True)
    os.close(existing)
environment = {**os.environ, 'MAKEFLAGS': '-j --jobserver-auth=' + auth,
               'MAKELEVEL': '0', 'WIRE_IDENTITY': f'{metadata.st_dev}:{metadata.st_ino}'}
os.execvpe(make, [make, '-rR', '--no-print-directory', '-f', 'child.mk'], environment)
