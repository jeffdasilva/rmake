.SECONDEXPANSION:
.NOTPARALLEL: all.out
LATE := c d e
all.out:: a | b c ; @$(info head $+ | $|):
all.out:: x y ; @$(info tail $+ | $|):
a b c d e x y z: ; @$(info leaf $@):
