.SECONDEXPANSION:
.NOTPARALLEL: all.out
DYNAMIC := b c d e
LAST := g h i
WAIT := .WAIT
all.out: a
all.out: $$(DYNAMIC)
%.out: p1 p2 ; @$(info goal $+ | $|):
a b c d e f g h i p1 p2 x y: ; @$(info leaf $@):
