import os,pathlib,sys
cwd=pathlib.Path.cwd();work=pathlib.Path(os.environ['TMPDIR']).parent/'work';temporary=pathlib.Path(os.environ['TMPDIR'])
new_source=pathlib.Path('next.mk').read_bytes();mode,name=sys.argv[1:]
if mode=='origfile':
 work.rename(temporary/'saved-work');work.write_bytes(b'not a directory\n');destination=None
else:
 cwd.rename(temporary/'saved')
 if mode=='retarget':
  destination=work/'other';(work/'link').unlink();(work/'link').symlink_to('other')
 elif mode=='broken':
  (work/'link').unlink();(work/'link').symlink_to('missing');destination=None
 else:destination=cwd
if destination is not None:
 destination.mkdir(parents=True,exist_ok=True);(destination/name).write_bytes(new_source)
 (destination/'new.in').write_bytes(b'new\n');(destination/'inc').mkdir();(destination/'inc/config.mk').write_bytes(b'MARKER=new\n')
