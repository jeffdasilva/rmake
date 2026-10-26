import os
os.symlink('link','link')
os.utime('out',ns=(1600000010000000000,)*2,follow_symlinks=False)
os.utime('link',ns=(1600000000000000000,)*2,follow_symlinks=False)
