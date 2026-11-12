import os
import resource
import sys
limit=int(sys.argv[1])
targets=list(map(int,sys.argv[2].split(',')))
source=os.open('/dev/null',os.O_RDONLY)
for target in targets:
    if target==source:
        os.set_inheritable(source,True)
    else:
        os.dup2(source,target,inheritable=True)
if source not in targets:
    os.close(source)
resource.setrlimit(resource.RLIMIT_NOFILE,(limit,limit))
os.execv(sys.argv[3],sys.argv[3:])
