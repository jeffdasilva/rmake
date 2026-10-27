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
 match=re.search(r'--jobserver-auth=(\d+),(\d+)',os.environ.get('MAKEFLAGS',''))
 if not match: raise SystemExit(9)
 read_fd,write_fd=map(int,match.groups())
 if not select.select([read_fd],[],[],3)[0]: raise SystemExit(10)
 token=os.read(read_fd,1)
 if not token: raise SystemExit(11)
 os.write(write_fd,token)
 Path('finished').write_text('b')
else:
 await_file('started')
 Path('trigger').write_text('a')
