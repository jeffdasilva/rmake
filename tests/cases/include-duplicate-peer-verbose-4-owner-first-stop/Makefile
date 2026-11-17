$(info READ:$(MAKE_RESTARTS))
.DEFAULT_GOAL := all
.PHONY: all force
all: ; @echo ALL:$(MAKE_RESTARTS)
-include pair.peer.mk pair.owner.mk
ifeq ($(MAKE_RESTARTS),)
pair.owner.mk pair.peer.mk pair.peer.mk pair.peer.mk pair.peer.mk &:: force ; @$(file >>seen,group)false
endif
