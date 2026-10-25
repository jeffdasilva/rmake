seed := $(shell touch -d @1600000002 out; touch -d @1600000000 input)
VPATH=$(info should-not-expand-vpath)
.NOTINTERMEDIATE: bad
.INTERMEDIATE: bad
all: out
out: mid
	@echo out=$@; cp -p $< $@
mid: input
	@echo mid=$@; cp -p $< $@
