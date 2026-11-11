.SECONDEXPANSION:
.NOTPARALLEL: peer.out
LATE := c d e
all.out peer.out &: a $$(LATE) ; @$(info goal $@ $+ | $|):
a b c d e x y z: ; @$(info leaf $@):
