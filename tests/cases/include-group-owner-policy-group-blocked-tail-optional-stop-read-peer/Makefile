$(info READ:$(MAKE_RESTARTS))
.DEFAULT_GOAL := all
.PHONY: all force
all: ; @cat seen; echo ALL:$(MAKE_RESTARTS)
-include pair.owner.mk pair.peer.mk
ifeq ($(MAKE_RESTARTS),)
pair.owner.mk:: force ; @$(file >>seen,owner):
pair.owner.mk pair.peer.mk &:: absent ; @$(file >>seen,group):
endif
