$(info READ:$(MAKE_RESTARTS))
.PHONY: all force
all: ; @printf "%s\n" "ALL:$(VALUE)"
-include payload.mk
-include worker.mk
ifeq ($(MAKE_RESTARTS),)
force:
worker.mk payload.mk &: force ; @exit 4
endif
