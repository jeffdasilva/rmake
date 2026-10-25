seed := $(shell touch -d @1600000002 out)
.SECONDARY:
.SECONDEXPANSION:
all: out
out: x.mid
	@echo out; cp -p $< $@
VPATH=$(shell printf "input\n" >first; touch -d @1600000004 first)
x.mid peer.mid &: $$(info owner=$$@)first
	@echo group; cp -p $< x.mid; cp -p $< peer.mid
