$(info READ:$(MAKE_RESTARTS))
.PHONY: all force
all: ; @printf "%s\n" "ALL:$(PAYLOAD):$(WORKER)"
include payload.mk
include worker.mk
ifeq ($(MAKE_RESTARTS),)
force:
payload.mk worker.mk &: force ; @rm -f worker.mk; exit 4
endif
