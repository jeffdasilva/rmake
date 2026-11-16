$(info READ:$(MAKE_RESTARTS))
.PHONY: all force
all: ; @printf "%s\n" "ALL:$(VALUE)"
-include payload.mk
include worker.mk
ifeq ($(MAKE_RESTARTS),)
force:
payload.mk: worker.mk ; @:
worker.mk:: force ; @exit 4
endif
