import os
import sys
import time
for descriptor in os.listdir('/proc/self/fd'):
 if int(descriptor)>2:
  try:os.fstat(int(descriptor))
  except OSError:continue
  raise RuntimeError('unexpected inherited capture descriptor')
if sys.argv[1]=='closed':
 os.close(1)
 time.sleep(.01)
 raise SystemExit(7)
if sys.argv[1]=='large':os.write(1,b'x '*32768)
else:os.write(1,b'alpha\nbeta\n')
