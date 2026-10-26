import os
os.utime('out',ns=(1600000010000000000,)*2,follow_symlinks=False)
os.utime('other/input',ns=(1600000000000000000,)*2,follow_symlinks=False)
os.utime('input',ns=(1600000030000000000,)*2,follow_symlinks=False)
os.utime('link',ns=(1600000000000000000,)*2,follow_symlinks=False)
os.utime('middle',ns=(1600000000000000000,)*2,follow_symlinks=False)
os.utime('other/real/hop',ns=(1600000000000000000,)*2,follow_symlinks=False)
