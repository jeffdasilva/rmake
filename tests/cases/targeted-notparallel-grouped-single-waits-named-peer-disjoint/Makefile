.SECONDEXPANSION:
.NOTPARALLEL: peer.out
LATE := c d e
all.out peer.out &: ; @$(info goal $@ $+ | $|):
all.out: a
peer.out: x y
a b c d e x y z: ; @$(info leaf $@):
