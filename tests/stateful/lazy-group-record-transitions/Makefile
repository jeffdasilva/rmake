.SECONDARY:
.SECONDEXPANSION:
all: out
out: x.mid
	@echo out changed=$?; cp -p $< $@
x.mid peer.mid &: $$(info owner=$$@)first
	@echo group=$@; cp -p $< x.mid; cp -p $< peer.mid
peer.mid: $$(info peer=$$@)second
