$(info READ:$(MAKE_RESTARTS))
.PHONY: all force
all: ; @printf "%s\n" "ALL:$(WORKER)"
ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1680300000 worker.mk)
endif
-include payload.mk
include worker.mk
ifneq ($(WORKER),changed)
force:
payload.mk worker.mk &: force ; @printf 'WORKER = changed\n' > worker.mk; touch -d @1680300020 worker.mk
endif
