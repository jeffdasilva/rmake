seed := $(shell touch -d @1600000002 input; touch -d @1600000000 out)
.DEFAULT_GOAL := all
.NOTINTERMEDIATE: V=unused
.INTERMEDIATE: mid
all: out
out: mid
	@echo out; cp -p $< $@
mid: input
	@echo mid; cp -p $< $@
