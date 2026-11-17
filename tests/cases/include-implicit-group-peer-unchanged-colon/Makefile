$(info READ:$(MAKE_RESTARTS))
.DEFAULT_GOAL := all
.PHONY: all force
all: ; @echo ALL:$(MAKE_RESTARTS)
include pair.owner.mk pair.peer.mk
ifeq ($(MAKE_RESTARTS),)
%.owner.mk %.peer.mk &: force ; @:
endif
