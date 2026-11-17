$(info READ:$(MAKE_RESTARTS))
.DEFAULT_GOAL := all
.PHONY: all force
all: ; @cat seen; echo ALL:$(MAKE_RESTARTS)
-include pair.owner.mk pair.peer.mk
ifeq ($(MAKE_RESTARTS),)
pair.owner.mk:: absent ; @$(file >>seen,owner-blocked):
pair.owner.mk pair.peer.mk &:: force ; @$(file >>seen,group):
endif
