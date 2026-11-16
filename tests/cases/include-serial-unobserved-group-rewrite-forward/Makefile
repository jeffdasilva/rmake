$(info READ:$(MAKE_RESTARTS))
.PHONY: all force
all: ; @printf "%s\n" "ALL:$(VALUE)"
ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1680300000 payload.mk)
endif
include worker.mk
include payload.mk
ifneq ($(VALUE),new)
force:
worker.mk payload.mk witness.mk &: force ; @printf 'VALUE = new\n' > payload.mk; touch -d @1680300020 payload.mk
endif
