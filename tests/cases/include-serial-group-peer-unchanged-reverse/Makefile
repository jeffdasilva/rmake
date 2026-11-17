$(info READ:$(MAKE_RESTARTS))
.PHONY: all force
all: ; @printf "%s\n" "ALL:$(MAKE_RESTARTS):$(VALUE)"
include peer.mk
include owner.mk
ifeq ($(MAKE_RESTARTS),)
owner.mk peer.mk &: force ; @printf "RUN\n"
endif
