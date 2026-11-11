.SECONDEXPANSION:
LATE := b c d
TRAILING := a .WAIT
ORDER_TRAILING := a .WAIT | b c
LATE_ORDER := a | b .WAIT
EFFECTIVE := a .WAIT b c
all.out: a .WAIT
all.out: b c
all.out: ; @$(info goal $@ $+ | $|):
a b c d: ; @$(info leaf $@):
