seed := $(shell touch -d @1600000002 out; touch -d @1600000000 input)
.SECONDARY:
.SECONDEXPANSION:
all: out
out: x.mid
	@echo out=$@; cp -p $< $@
%.mid: $$(info search=$$@)input
	@echo mid=$@; cp -p $< $@
