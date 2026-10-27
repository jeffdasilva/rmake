import os,re,stat
flags=os.environ.get('MAKEFLAGS','')
auth=re.findall(r'--jobserver-auth=([^ ]+)',flags)
jobs=re.findall(r'(?:^| )-j([^ ]*)',flags)
invalid=bool(auth and auth[-1]=='-2,-2')
pool='none'
positive=next((a for a in auth if ',' in a and not a.startswith('-')),None)
if positive:
 pool='closed'
 try:
  a,b=map(int,positive.split(','));left=os.fstat(a);right=os.fstat(b)
 except OSError:pass
 else:
  assert stat.S_ISFIFO(left.st_mode) and (left.st_dev,left.st_ino)==(right.st_dev,right.st_ino),'wrong forwarded pipe'
  pool='live'
elif auth and auth[0].startswith('fifo:'):
 assert stat.S_ISFIFO(os.stat(auth[0][5:]).st_mode),'missing shared FIFO'
 pool='fifo'
if invalid and ' -- ' in flags:assert flags.index('--jobserver-auth=-2,-2')<flags.index(' -- '),'marker after command assignments'
print('jobs='+(','.join(jobs) or 'unset')+' pool='+pool+' disabled='+str(int(invalid))+' X='+os.environ.get('X','unset'),flush=True)
