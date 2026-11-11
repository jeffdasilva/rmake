.SECONDEXPANSION:
.NOTPARALLEL: all.out
LATE := c d e
all.out: .EXTRA_PREREQS = x y
all.out:: a ; @$(info head $+ | $|):
all.out:: b c ; @$(info tail $+ | $|):
a b c d e x y z: ; @$(info leaf $@):
