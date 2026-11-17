$(info READ:$(MAKE_RESTARTS))
.DEFAULT_GOAL := all
.PHONY: all force
all: ; @echo ALL:$(MAKE_RESTARTS)
include pair.owner.mk
ifeq ($(MAKE_RESTARTS),)
%.owner.mk %.peer.mk &: force ; @printf "RUN\n"
endif
