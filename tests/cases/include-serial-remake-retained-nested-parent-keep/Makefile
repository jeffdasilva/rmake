$(info READ:$(MAKE_RESTARTS))
.PHONY: all force
all: ; @printf "%s\n" "ALL:$(VALUE)"
-include payload.mk
include worker.mk
ifeq ($(MAKE_RESTARTS),)
force:
payload.mk: middle ; @:
middle: worker.mk ; @:
worker.mk: force ; @printf 'VALUE = new\n' > payload.mk; exit 4
endif
