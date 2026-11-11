.SECONDEXPANSION:
.NOTPARALLEL: all.out peer.out
LATE := c d e
all.out peer.out &: a | b c ; @$(info goal $@ $+ | $|):
a b c d e x y z: ; @$(info leaf $@):
