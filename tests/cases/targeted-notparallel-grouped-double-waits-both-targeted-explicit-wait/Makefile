.SECONDEXPANSION:
.NOTPARALLEL: all.out peer.out
LATE := c d e
all.out peer.out &:: a .WAIT b c ; @$(info head $@ $+ | $|):
all.out peer.out &:: x y .WAIT z ; @$(info tail $@ $+ | $|):
a b c d e x y z: ; @$(info leaf $@):
