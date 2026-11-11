.SECONDEXPANSION:
.NOTPARALLEL: all.out last.out
LATE := c d e
all.out peer.out last.out &: a $$(LATE) ; @$(info goal $@ $+ | $|):
a b c d e x y z: ; @$(info leaf $@):
