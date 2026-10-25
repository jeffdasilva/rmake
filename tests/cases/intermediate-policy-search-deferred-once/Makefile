seed := $(shell touch -d @1600000002 out; touch -d @1600000000 input; touch -d @1600000004 extra)
.SECONDARY:
.SECONDEXPANSION:
all: out
out: x.mid extra
	@echo out; cp -p $< $@
x.mid: $$(info expanded=$$@)input
	@echo mid; cp -p $< $@
