X = $(file >>seen,base)$(eval %.task: X := changed)outer
%.task: X += $(file >>seen,suffix)tail
.PHONY: all one.task two.task
all: one.task two.task
one.task two.task: ; $(info $@=[$(X)/$(flavor X)])@:
