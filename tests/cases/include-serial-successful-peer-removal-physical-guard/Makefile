$(info READ:$(MAKE_RESTARTS))
.PHONY: all force
all: ; @printf "%s\n" "ALL:$(PAYLOAD):$(WORKER)"
include payload.mk
include $(wildcard worker.mk)
ifneq ($(wildcard worker.mk),)
force:
payload.mk worker.mk &: force ; @rm -f worker.mk
endif
