seed := $(shell touch -d @1600000000 out; touch -d @1600000002 input)
.INTERMEDIATE: mid
all: out
out: mid
	@echo out=$@; cp -p $< $@
mid: input
	@echo mid=$@; cp -p $< $@
