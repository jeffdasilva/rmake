$(info READ:$(MAKE_RESTARTS))
.PHONY: all force
all: ; @echo ALL:$(VALUE)
include worker.mk
include payload.mk
ifeq ($(MAKE_RESTARTS),)
worker.mk: force ; @rm -f payload.mk
endif
