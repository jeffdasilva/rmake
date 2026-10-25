seed := $(shell touch -d @1600000002 out; touch -d @1600000000 first; touch -d @1600000004 second; touch -d @1600000000 peer.mid)
.SECONDARY:
.SECONDEXPANSION:
all: out
out: x.mid
	@echo out=$@; cp -p $< $@
x.mid peer.mid &:: $$(info first=$$@)first
	@echo first=$@; cp -p $< x.mid; cp -p $< peer.mid
x.mid peer.mid &:: $$(info second=$$@)second
	@echo second=$@; cp -p $< x.mid; cp -p $< peer.mid
