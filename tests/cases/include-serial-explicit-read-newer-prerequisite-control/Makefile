ifeq ($(MAKE_RESTARTS),)
$(shell touch -t 200001010000 worker.mk seed other.mk payload.mk; touch -t 200101010000 seed)
endif
$(info READ:$(MAKE_RESTARTS))
.PHONY: all force
all: ; @echo ALL:$(VALUE)
include worker.mk
include payload.mk
ifeq ($(MAKE_RESTARTS),)
worker.mk: force ; @mv -f other.mk payload.mk
endif
payload.mk: seed ; @cp seed $@
