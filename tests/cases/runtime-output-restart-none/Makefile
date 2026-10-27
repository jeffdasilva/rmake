$(info pass-$(if $(MAKE_RESTARTS),second,first))
include generated.mk
.PHONY: all
all:;@printf '%s\n' '$(VALUE)'
generated.mk:;@printf 'VALUE = ready\n' > $@
