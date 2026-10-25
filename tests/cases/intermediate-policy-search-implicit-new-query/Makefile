seed := $(shell touch -d @1600000000 out; touch -d @1600000002 input)
.SECONDARY:
.SECONDEXPANSION:
all: out
out: x.mid
	@echo out=$@; cp -p $< $@
%.mid: input
	@echo mid=$@; cp -p $< $@
