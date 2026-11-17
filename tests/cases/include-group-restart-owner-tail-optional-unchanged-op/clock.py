# SPDX-License-Identifier: GPL-3.0-or-later
# Original rmake nanosecond clock fixture.
import os, sys
from pathlib import Path
stamp = 1680300000000000000
names = ["owner.mk", "peer.mk", "third.mk"]
mode = sys.argv[1]
if mode == "init":
    for name in names:
        os.utime(name, ns=(stamp, stamp))
        assert Path(name).stat().st_mtime_ns == stamp
elif mode in ("owner-newer", "peer-newer", "peer-restored"):
    name = "owner.mk" if mode == "owner-newer" else "peer.mk"
    os.utime(name, ns=(stamp + 1, stamp + 1))
    assert Path(name).stat().st_mtime_ns == stamp + 1
    if mode == "peer-restored":
        os.utime(name, ns=(stamp, stamp))
        assert Path(name).stat().st_mtime_ns == stamp
state = "".join(f"{name}={Path(name).stat().st_mtime_ns}\n" for name in names)
Path("clock-state").write_text(state)
if mode == "show":
    print(state, end="")
