$(info READ:$(MAKE_RESTARTS))
ifeq ($(MAKE_RESTARTS),)
$(shell touch -t 200001010000 worker.mk payload.mk)
endif
include worker.mk
-include payload.mk
.PHONY: all force
all: ; @echo ALL:$(VALUE)
ifeq ($(MAKE_RESTARTS),)
worker.mk payload.mk &: force ; @rm -f payload.mk
endif
