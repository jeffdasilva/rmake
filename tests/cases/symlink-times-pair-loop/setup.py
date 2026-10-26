import os
if not os.path.lexists('link'): os.symlink('hop', 'link')
if not os.path.lexists('hop'): os.symlink('link', 'hop')
os.utime('out',ns=(1600000010000000000,)*2,follow_symlinks=False)
os.utime('link',ns=(1600000000000000000,)*2,follow_symlinks=False)
