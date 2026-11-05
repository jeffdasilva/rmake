X = $(file >>seen,base)$(eval MARK := first)global
%.task: X += $(file >>seen,suffix=$(MARK))tail
$(file >>seen,reader)
one.task: ; $(info value=[$(X)] after=[$(MARK)])@:
