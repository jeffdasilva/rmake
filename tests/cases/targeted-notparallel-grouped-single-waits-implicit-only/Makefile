.SECONDEXPANSION:
.NOTPARALLEL: all.out
LATE := c d e
%.out %.peer &: a b c ; @$(info goal $@ $+ | $|):
a b c d e x y z: ; @$(info leaf $@):
