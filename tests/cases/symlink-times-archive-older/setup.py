import os
os.utime('lib.a',ns=(1600000000000000000,)*2,follow_symlinks=False)
os.utime('input',ns=(1600000000000000000,)*2,follow_symlinks=False)
os.utime('one.o',ns=(1600000019000000000,)*2,follow_symlinks=False)
