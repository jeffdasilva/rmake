seed := $(shell touch -d @1600000002 out; touch -d @1600000004 first; touch -d @1600000000 second; touch -d @1600000000 peer.mid)
.SECONDARY:
.SECONDEXPANSION:
all: out
out: x.mid
	@echo out=$@; cp -p $< $@
x.mid peer.mid &: $$(info first=$$@)first
	@echo group=$@; cp -p $< x.mid; cp -p $< peer.mid
peer.mid: $$(info peer=$$@)second
