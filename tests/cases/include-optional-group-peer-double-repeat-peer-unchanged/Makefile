ifeq ($(MAKE_RESTARTS),)
$(shell python3 clock.py init)
endif
$(info READ:$(MAKE_RESTARTS))
.DEFAULT_GOAL := all
.PHONY: all force
all: ; @$(file >>seen,goal)echo ALL:$(MAKE_RESTARTS); cat seen; python3 clock.py show
-include owner.mk
include peer.mk
include peer.mk
ifeq ($(MAKE_RESTARTS),)
owner.mk peer.mk &:: force ; @$(file >>seen,group:$@)python3 clock.py unchanged; false
third.mk: force ; @$(file >>seen,third):
endif
