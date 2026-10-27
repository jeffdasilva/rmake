from pathlib import Path
import os,re,select,sys,time

def await_file(name):
 for _ in range(500):
  if Path(name).exists(): return
  time.sleep(.005)
 raise SystemExit(8)

if sys.argv[1]=='b':
 Path('started').write_text('b')
 await_file('trigger')
 await_file('fatal-ready')
 Path('finished').write_text('b')
else:
 await_file('started')
 Path('trigger').write_text('a')
