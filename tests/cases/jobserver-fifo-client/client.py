import os,sys,stat
words=os.environ['MAKEFLAGS'].split()
auth=next(x.split('=',1)[1] for x in words if x.startswith('--jobserver-auth='))
if auth.startswith('fifo:'):
 r=w=os.open(auth[5:],os.O_RDWR|os.O_NONBLOCK)
else:
 a,b=map(int,auth.split(','))
 try:
  r=os.open('/proc/self/fd/'+str(a),os.O_RDONLY|os.O_NONBLOCK)
 except OSError:
  assert sys.argv[1]=='closed'
  print('closed')
  sys.exit(0)
 w=b
assert sys.argv[1]!='closed'
assert stat.S_ISFIFO(os.fstat(r).st_mode)
tokens=b''
while True:
 try:
  token=os.read(r,1)
  assert token
  tokens+=token
 except BlockingIOError: break
assert len(tokens)==2,tokens
os.write(w,tokens)
print('tokens:'+str(len(tokens)))
