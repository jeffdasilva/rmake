import os
os.utime('out',ns=(1600000010000000000,)*2,follow_symlinks=False)
os.utime('input',ns=(1600000000000000000,)*2,follow_symlinks=False)
os.utime('link',ns=(1600000020000000000,)*2,follow_symlinks=False)
os.utime('config.mk',ns=(1600000000000000000,)*2,follow_symlinks=False)
os.utime('real.mk',ns=(1600000000000000000,)*2,follow_symlinks=False)
