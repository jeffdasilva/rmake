$(info READ:$(MAKE_RESTARTS))
.PHONY: all force
all: ; @printf "%s\n" "ALL:$(MAKE_RESTARTS):$(VALUE)"
include owner.mk
include peer.mk
ifeq ($(MAKE_RESTARTS),)
owner.mk peer.mk &:  ; @printf "RUN\n"
endif
