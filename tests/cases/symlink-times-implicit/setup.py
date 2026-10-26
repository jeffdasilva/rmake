import os
os.utime('one.in',ns=(1600000020000000000,)*2,follow_symlinks=False)
os.utime('one.out',ns=(1600000010000000000,)*2,follow_symlinks=False)
os.utime('input',ns=(1600000000000000000,)*2,follow_symlinks=False)
