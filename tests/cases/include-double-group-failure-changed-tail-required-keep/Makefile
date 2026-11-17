ifeq ($(MAKE_RESTARTS),)
$(shell python3 prepare-times.py)
endif
$(info READ:$(MAKE_RESTARTS))
.DEFAULT_GOAL := all
.PHONY: all force
all: ; @python3 check-times.py && cat seen && echo ALL:$(MAKE_RESTARTS)
include pair.owner.mk pair.peer.mk
ifeq ($(MAKE_RESTARTS),)
pair.owner.mk pair.peer.mk &:: force ; @$(file >>seen,group)printf "# changed\n" > pair.peer.mk; touch -m -d @1680300010 pair.peer.mk
pair.peer.mk:: force ; @$(file >>seen,fail)false
endif
