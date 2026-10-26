import os
os.utime('item',ns=(1600000020000000000,)*2,follow_symlinks=False)
os.utime('src/item',ns=(1600000000000000000,)*2,follow_symlinks=False)
