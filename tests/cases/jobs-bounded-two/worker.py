import fcntl, pathlib, sys, time
with open('lock', 'a') as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    n = int(pathlib.Path('active').read_text()) + 1
    pathlib.Path('active').write_text(str(n))
    pathlib.Path('maximum').write_text(str(max(n, int(pathlib.Path('maximum').read_text()))))
for _ in range(300):
    try:
        if int(pathlib.Path('maximum').read_text() or '0') >= 2: break
    except ValueError: pass
    time.sleep(.01)
else: sys.exit(9)
time.sleep(.02)
with open('lock', 'a') as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    pathlib.Path('active').write_text(str(int(pathlib.Path('active').read_text()) - 1))
pathlib.Path(sys.argv[1]).write_text('complete\n')
