$(info READ:$(MAKE_RESTARTS))
.PHONY: all force
all: ; @printf "%s\n" "ALL:$(VALUE):$(WORKER)"
include worker.mk
include payload.mk
ifeq ($(MAKE_RESTARTS),)
force:
worker.mk payload.mk &: force ; @printf 'VALUE = new\n' > payload.mk; exit 4
endif
