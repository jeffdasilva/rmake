.SECONDEXPANSION:
.NOTPARALLEL: all.out peer.out
LATE := c d e
all.out peer.out &:: a b ; @$(info head $@ $+ | $|):
all.out peer.out &:: x $$(LATE) ; @$(info tail $@ $+ | $|):
a b c d e x y z: ; @$(info leaf $@):
