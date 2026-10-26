import os
for n,s in [('archive.a',0),('out',40)]:
 os.utime(n,ns=((1600000000+s)*10**9,)*2)
