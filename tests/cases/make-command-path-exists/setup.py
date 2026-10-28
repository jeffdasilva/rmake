import os,pathlib
name=b'dir\xff'
os.mkdir(name);pathlib.Path(os.fsdecode(name),"inner.mk").write_bytes(b"child: ; @$(info directory=[$(notdir $(CURDIR))])true\n")
