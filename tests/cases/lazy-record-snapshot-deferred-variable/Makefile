seed := $(shell touch -d @1600000002 out)
.SECONDARY:
.SECONDEXPANSION:
all: out
out: x.mid
	@echo out; cp -p $< $@
NAME=first
x.mid peer.mid &: $$(NAME)
	@echo group; cp -p $< x.mid; cp -p $< peer.mid
