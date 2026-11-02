ifeq ($(MAKE_RESTARTS),)
$(shell python3 setup.py)
endif
include generated.mk
all:;@printf 'G=[%s] R=[%s]\n' '$(G)' '$(MAKE_RESTARTS)'
generated.mk: input
	@printf 'G=new\n' > generated.mk
