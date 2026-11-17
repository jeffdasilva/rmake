$(info READ:$(MAKE_RESTARTS))
.DEFAULT_GOAL := all
.PHONY: all force
all: ; @echo ALL:$(MAKE_RESTARTS)
include dir/pair.owner.mk dir/pair.peer.mk
ifeq ($(MAKE_RESTARTS),)
%.owner.mk %.peer.mk &: force ; @printf "RUN\n"
endif
