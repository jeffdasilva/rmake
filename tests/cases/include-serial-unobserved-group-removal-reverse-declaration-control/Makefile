$(info READ:$(MAKE_RESTARTS))
.PHONY: all force
all: ; @printf "%s\n" "ALL:$(VALUE)"
include worker.mk
include payload.mk
ifeq ($(MAKE_RESTARTS),)
force:
worker.mk witness.mk payload.mk &: force ; @rm -f payload.mk
endif
