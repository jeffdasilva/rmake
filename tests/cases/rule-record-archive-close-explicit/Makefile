$(shell python3 setup.py old.o)
.DEFAULT_GOAL := out
out: lib.a(*.o)
ifeq ($(shell python3 setup.py late.o),)
endif
	@printf '[%s|%s|%s]\n' '$@' '$^' '$|'
