$(info READ:$(MAKE_RESTARTS))
.PHONY: all force
all: ; @printf "%s\n" "ALL:$(PAYLOAD):$(WORKER)"
include payload.mk
include worker.mk
ifeq ($(MAKE_RESTARTS),)
force:
payload.mk worker.mk &: force ; @cp -p worker.mk stamp; printf 'WORKER = changed\n' > worker.mk; touch -r stamp worker.mk; rm -f stamp; exit 4
endif
