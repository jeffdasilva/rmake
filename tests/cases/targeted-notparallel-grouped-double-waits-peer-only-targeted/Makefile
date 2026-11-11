.SECONDEXPANSION:
.NOTPARALLEL: peer.out
LATE := c d e
all.out peer.out &:: a b c ; @$(info head $@ $+ | $|):
all.out peer.out &:: x y z ; @$(info tail $@ $+ | $|):
a b c d e x y z: ; @$(info leaf $@):
