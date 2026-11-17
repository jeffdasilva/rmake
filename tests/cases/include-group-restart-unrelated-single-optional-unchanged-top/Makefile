ifeq ($(MAKE_RESTARTS),)
$(shell python3 clock.py init)
endif
$(info READ:$(MAKE_RESTARTS))
.DEFAULT_GOAL := all
.PHONY: all force
all: ; @echo ALL:$(MAKE_RESTARTS); cat seen; python3 clock.py show
-include third.mk owner.mk peer.mk
ifeq ($(MAKE_RESTARTS),)
owner.mk peer.mk &:: force ; @$(file >>seen,group:$@)python3 clock.py unchanged
third.mk: force ; @$(file >>seen,third-fail)false
endif
