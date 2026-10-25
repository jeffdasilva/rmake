seed := $(shell touch -d @1600000002 out; touch -d @1600000000 input)
.SECONDARY:
.SECONDEXPANSION:
all: out
out: x.mid
	@echo out=$@; cp -p $< $@
x.mid: $$(error deferred-failure)input
	@echo should-not-run
