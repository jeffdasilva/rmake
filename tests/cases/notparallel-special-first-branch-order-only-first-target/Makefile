.SECONDEXPANSION:
NAMED := all.out
.NOTPARALLEL:: | all.out
all.out: a b c ; @$(info goal $@ $+ | $|):
other.out: x y z ; @$(info goal $@ $+ | $|):
a b c x y z: ; @$(info leaf $@):
