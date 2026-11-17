$(info READ:$(MAKE_RESTARTS))
.DEFAULT_GOAL := all
.PHONY: all force
all: ; @cat seen; echo ALL:$(MAKE_RESTARTS)
-include pair.owner.mk pair.peer.mk
ifeq ($(MAKE_RESTARTS),)
.DELETE_ON_ERROR:
.PRECIOUS: pair.owner.mk
pair.owner.mk pair.peer.mk &:: force ; @$(file >>seen,group):
pair.owner.mk:: force ; @$(file >>seen,owner-fail)printf "# changed\n" > pair.owner.mk; false
endif
