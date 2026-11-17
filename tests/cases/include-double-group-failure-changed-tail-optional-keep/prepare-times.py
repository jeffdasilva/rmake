import os
for name in ("pair.owner.mk", "pair.peer.mk", "third.mk"):
    os.utime(name, ns=(1680300000000000000,)*2)
    assert os.stat(name).st_mtime_ns == 1680300000000000000
