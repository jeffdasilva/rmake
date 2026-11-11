.SECONDEXPANSION:
.NOTPARALLEL: all.out
LATE := c d e
all.out:: a b ; @$(info head $+ | $|):
all.out:: x $$(LATE) ; @$(info tail $+ | $|):
a b c d e x y z: ; @$(info leaf $@):
