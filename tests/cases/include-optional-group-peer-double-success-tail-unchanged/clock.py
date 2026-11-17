# SPDX-License-Identifier: GPL-3.0-or-later
# Original rmake source-clock fixture.
import os, sys
from pathlib import Path
stamp = 1680300000000000000
names = ["owner.mk", "peer.mk", "third.mk"]
mode = sys.argv[1]
if mode == "init":
    for name in names:
        os.utime(name, ns=(stamp, stamp))
        assert Path(name).stat().st_mtime_ns == stamp
elif mode in ("set", "restore"):
    name = sys.argv[2]
    value = stamp + int(sys.argv[3])
    os.utime(name, ns=(value, value))
    assert Path(name).stat().st_mtime_ns == value
    if mode == "restore":
        os.utime(name, ns=(stamp, stamp))
        assert Path(name).stat().st_mtime_ns == stamp
elif mode == "remove":
    Path(sys.argv[2]).unlink()
state = "".join(f"{name}={Path(name).stat().st_mtime_ns if Path(name).exists() else 'missing'}\n" for name in names)
Path("clock-state").write_text(state)
if mode == "show":
    print(state, end="")
