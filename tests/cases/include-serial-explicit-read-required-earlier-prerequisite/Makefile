ifeq ($(MAKE_RESTARTS),)
$(shell touch -t 200001010000 worker.mk seed other.mk payload.mk)
endif
$(info READ:$(MAKE_RESTARTS))
.PHONY: all force
all: ; @echo ALL:$(VALUE)
include worker.mk
include payload.mk
ifeq ($(MAKE_RESTARTS),)
worker.mk: force ; @rm -f payload.mk
endif
payload.mk: worker.mk ; @cp seed $@
