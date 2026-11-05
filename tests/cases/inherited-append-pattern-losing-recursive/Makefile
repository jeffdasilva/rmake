X = outer
%.task: override private X += $(file >>seen,retained)kept
%.task: X += $(file >>seen,loser)lost
.PHONY: one.task child
one.task: child
one.task child: ; $(info $@=[$(X)/$(origin X)/$(flavor X)])@:
