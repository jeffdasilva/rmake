from pathlib import Path
from time import monotonic, sleep

release = Path('released')
deadline = monotonic() + 2
while not release.exists() and monotonic() < deadline:
    sleep(0.002)
print('parallel' if release.exists() else 'serial')
