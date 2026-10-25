seed := $(shell touch -d @1600000002 out)
.INTERMEDIATE: input mid
all: out
out: mid
	@echo out=$@; cp -p $< $@
mid: input
	@echo mid=$@; cp -p $< $@
