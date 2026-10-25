seed := $(shell touch -d @1600000002 out; touch -d @1600000000 input)
.SECONDARY:
.NOTINTERMEDIATE:
.INTERMEDIATE: named
.NOTINTERMEDIATE: named
all: out
out: mid
	@echo out=$@; cp -p $< $@
mid: input
	@echo mid=$@; cp -p $< $@
