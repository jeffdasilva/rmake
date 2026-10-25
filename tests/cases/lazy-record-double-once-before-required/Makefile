seed := $(shell touch -d @1600000002 out; touch -d @1600000000 first; touch -d @1600000000 second; touch -d @1600000000 peer.mid; touch -d @1600000004 extra)
.SECONDARY:
.SECONDEXPANSION:
all: out
out: x.mid extra
	@echo out=$@; cp -p $< $@
x.mid:: $$(info first=$$@)first
	@echo first=$@; cp -p $< $@
x.mid:: $$(info second=$$@)second
	@echo second=$@; cp -p $< $@
