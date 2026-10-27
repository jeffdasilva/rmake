from pathlib import Path
import sys
import time
name = sys.argv[1]
if name == 'a':
    deadline = time.monotonic() + 3
    while not Path('expanded-c').exists():
        if time.monotonic() >= deadline:
            raise RuntimeError('recipe expansion waited behind load gate')
        time.sleep(.002)
with open('order', 'a') as out:
    out.write(name + '\n')
