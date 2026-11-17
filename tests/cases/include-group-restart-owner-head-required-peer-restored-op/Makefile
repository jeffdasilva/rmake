ifeq ($(MAKE_RESTARTS),)
$(shell python3 clock.py init)
endif
$(info READ:$(MAKE_RESTARTS))
.DEFAULT_GOAL := all
.PHONY: all force
all: ; @echo ALL:$(MAKE_RESTARTS); cat seen; python3 clock.py show
include owner.mk peer.mk
ifeq ($(MAKE_RESTARTS),)
owner.mk:: force ; @$(file >>seen,owner-fail)false
owner.mk peer.mk third.mk &:: force ; @$(file >>seen,group:$@)python3 clock.py peer-restored
endif
