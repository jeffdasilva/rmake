$(info READ:$(MAKE_RESTARTS))
.PHONY: all force
all: ; @printf "%s\n" "ALL:$(VALUE)"
include worker.mk
include payload.mk
ifneq ($(wildcard payload.mk),)
force:
worker.mk payload.mk witness.mk &: force ; @rm -f payload.mk
endif
