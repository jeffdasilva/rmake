import pathlib, shutil, sys, time
role, target, value, mode = sys.argv[1:]
def wait(name):
    deadline = time.monotonic() + 3
    while not pathlib.Path(name).exists():
        if time.monotonic() > deadline: raise SystemExit("barrier timed out")
        time.sleep(.002)
if role == "1":
    print("first=" + target + ":" + value, flush=True)
    pathlib.Path("first-started").write_bytes(b"ready\n")
    wait("second-started")
else:
    wait("first-started")
    print("second=" + target + ":" + value, flush=True)
    pathlib.Path("second-started").write_bytes(b"ready\n")
    if mode != "failed": wait("first-after")
shutil.copyfile("input", target)
if role == "1" and mode in ("failed", "ignored"): raise SystemExit(7)
