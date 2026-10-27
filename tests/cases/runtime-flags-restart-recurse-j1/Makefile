$(info read=$(if $(MAKE_RESTARTS),second,first) flags=[$(filter -j% -O% -l%,$(MAKEFLAGS))])
MAKEFLAGS += -j3 -l0
$(info after=[$(filter -j% -O% -l%,$(MAKEFLAGS))])
include generated.mk
.PHONY: all
all:;@printf 'ready\n'
generated.mk:;@printf 'VALUE = ready\n' > $@
