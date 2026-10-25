seed := $(shell touch -d @1600000000 input; touch -d @1600000002 out)
.DEFAULT_GOAL := all
unused: .SECONDARY
all: out
out: mid
	@echo out; cp -p $< $@
mid: input
	@echo mid; cp -p $< $@
