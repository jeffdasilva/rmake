.SECONDEXPANSION:
.NOTPARALLEL: all.out
LATE := c d e
all.out:: a $$(LATE) ; @$(info head $+ | $|):
all.out:: x $$(LATE) y ; @$(info tail $+ | $|):
a b c d e x y z: ; @$(info leaf $@):
