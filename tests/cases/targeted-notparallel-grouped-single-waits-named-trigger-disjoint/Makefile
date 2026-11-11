.SECONDEXPANSION:
.NOTPARALLEL: all.out
LATE := c d e
all.out peer.out &: ; @$(info goal $@ $+ | $|):
all.out: a b
peer.out: x
a b c d e x y z: ; @$(info leaf $@):
