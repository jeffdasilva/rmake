import os
p=os.path.realpath('Makefile')
seen=False
for f in os.listdir('/proc/self/fd'):
 try:
  seen |= os.readlink('/proc/self/fd/'+f)==p
 except OSError:
  pass
print('LEAK' if seen else 'closed')
