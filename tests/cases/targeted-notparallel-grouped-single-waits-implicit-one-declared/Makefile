.SECONDEXPANSION:
.NOTPARALLEL: all.out
LATE := c d e
all.out: x
%.out %.peer &: a b c ; @$(info goal $@ $+ | $|):
a b c d e x y z: ; @$(info leaf $@):
