seed := $(shell touch -d @1600000002 out; touch -d @1600000000 input)
GPATH=$(error should-not-expand-gpath)
.NOTINTERMEDIATE: bad
.SECONDARY: bad
all: out
out: mid
	@echo out=$@; cp -p $< $@
mid: input
	@echo mid=$@; cp -p $< $@
