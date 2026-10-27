from pathlib import Path
import time
Path('peer.started').touch()
end=time.monotonic()+3
while not Path('failed').exists():
 if time.monotonic()>end:raise RuntimeError('bounded completion barrier')
 time.sleep(.002)
Path('peer.done').touch()
