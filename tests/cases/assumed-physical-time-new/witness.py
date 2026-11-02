from pathlib import Path
print('MTIME='+str(Path('dep').stat().st_mtime_ns))
